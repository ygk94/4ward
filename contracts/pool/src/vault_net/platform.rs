// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Filet de tests du vault, angle plateforme Soroban : autorisations, TTL, événements,
//! échecs de transfert USDC, budget de ressources, codes d'erreur.
extern crate std;
use super::*;
use crate::{Pool, PoolClient, Role, Side};
use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _, Ledger as _, MockAuth, MockAuthInvoke,
};
use soroban_sdk::{symbol_short, token, Event as _, IntoVal, Symbol};

const USDC: i128 = 10_000_000;
const MXN_SPOT: i128 = 5_500_000_000_000; // 0,055 USD / MXN, 14 décimales
const MXN_1M: i128 = 10_000_000_000_000; // 1 000 000 MXN, 7 décimales
const SECS_PER_LEDGER: u64 = 5;

struct Net {
    env: Env,
    admin: Address,
    pool: Address,
    usdc: Address,
    oracle: Address,
}

/// Pool complet (SAC USDC, mock-oracle MXN à 0,055, taux et plafond minimum MXN réglés).
/// `mock_all_auths` pendant la mise en place ; les tests d'autorisation le coupent ensuite.
fn net() -> Net {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| {
        l.timestamp = 1_000_000;
        l.sequence_number = 1_000;
    });
    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    // Émetteur « révocable », comme l'USDC de Circle : set_authorized(false) gèle un solde.
    sac.issuer().set_flag(soroban_sdk::testutils::IssuerFlags::RevocableFlag);
    let usdc = sac.address();
    let oracle = env.register(MockOracle, (&admin,));
    MockOracleClient::new(&env, &oracle).set_price(&OracleAsset::Other(symbol_short!("MXN")), &MXN_SPOT);
    let pool = env.register(Pool, (&admin, &usdc, &oracle, &50u32));
    PoolClient::new(&env, &pool).set_senior_deposits_open(&true); // fermés par défaut (B5)
    let c = PoolClient::new(&env, &pool);
    c.set_rate(&symbol_short!("USD"), &400);
    c.set_rate(&symbol_short!("MXN"), &700);
    c.set_min_cap(&symbol_short!("MXN"), &500);
    Net { env, admin, pool, usdc, oracle }
}

/// Photo de la compta du vault + soldes USDC, pour vérifier qu'un échec ne laisse aucune trace.
#[derive(Debug, PartialEq, Eq)]
struct Books {
    total_deposits: i128,
    reserved: i128,
    total_shares: i128,
    senior_total_shares: i128,
    lp_shares: i128,
    lp_senior_shares: i128,
    usdc_pool: i128,
    usdc_lp: i128,
}

impl Net {
    fn client(&self) -> PoolClient<'_> {
        PoolClient::new(&self.env, &self.pool)
    }
    fn vault<R>(&self, f: impl FnOnce(&Env) -> R) -> R {
        self.env.as_contract(&self.pool, || f(&self.env))
    }
    fn usdc_balance(&self, who: &Address) -> i128 {
        token::Client::new(&self.env, &self.usdc).balance(who)
    }
    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.usdc).mint(to, &amount);
    }
    fn sac(&self) -> token::StellarAssetClient<'_> {
        token::StellarAssetClient::new(&self.env, &self.usdc)
    }
    fn allowed_lp(&self) -> Address {
        let lp = Address::generate(&self.env);
        self.client().set_allowed(&Role::Lp, &lp, &true);
        lp
    }
    fn new_lp(&self, amount: i128) -> Address {
        let lp = self.allowed_lp();
        self.mint(&lp, amount);
        self.client().deposit_lp(&lp, &amount);
        lp
    }
    fn new_senior(&self, amount: i128) -> Address {
        let lp = self.allowed_lp();
        self.mint(&lp, amount);
        self.client().deposit_senior(&lp, &amount);
        lp
    }
    /// Avance horloge et numéro de ledger ensemble (5 s par ledger), comme sur le réseau.
    fn advance_secs(&self, secs: u64) {
        self.env.ledger().with_mut(|l| {
            l.timestamp += secs;
            l.sequence_number += (secs / SECS_PER_LEDGER) as u32;
        });
    }
    fn set_mxn(&self, price: i128) {
        MockOracleClient::new(&self.env, &self.oracle).set_price(&OracleAsset::Other(symbol_short!("MXN")), &price);
    }
    /// Vraie couverture via `open_hedge` (pas `reserve` appelé à la main). Retourne (id, PME).
    fn open_hedge(&self, notional: i128, duration: u64) -> (u64, Address) {
        let sme = Address::generate(&self.env);
        self.mint(&sme, notional); // largement assez pour la marge
        let id = self.client().open_hedge(
            &sme,
            &Address::generate(&self.env),
            &symbol_short!("MXN"),
            &Side::BuyCurrency,
            &notional,
            &duration,
            &1000,
        );
        (id, sme)
    }
    fn books(&self, lp: &Address) -> Books {
        self.vault(|env| Books {
            total_deposits: total_deposits(env),
            reserved: reserved(env),
            total_shares: total_shares(env),
            senior_total_shares: senior_total_shares(env),
            lp_shares: env.storage().persistent().get(&VaultKey::Shares(lp.clone())).unwrap_or(0),
            lp_senior_shares: env.storage().persistent().get(&VaultKey::SeniorShares(lp.clone())).unwrap_or(0),
            usdc_pool: 0,
            usdc_lp: 0,
        })
        .with_usdc(self.usdc_balance(&self.pool), self.usdc_balance(lp))
    }
    fn pool_events(&self) -> soroban_sdk::testutils::ContractEvents {
        self.env.events().all().filter_by_contract(&self.pool)
    }
    /// Coupe `mock_all_auths` : plus aucune signature n'est simulée.
    fn require_real_auths(&self) {
        self.env.mock_auths(&[]);
    }
}

impl Books {
    fn with_usdc(mut self, pool: i128, lp: i128) -> Self {
        self.usdc_pool = pool;
        self.usdc_lp = lp;
        self
    }
}

fn func(env: &Env, contract: &Address, name: &str, args: soroban_sdk::Vec<Val>) -> AuthorizedFunction {
    AuthorizedFunction::Contract((contract.clone(), Symbol::new(env, name), args))
}

type TryErr = Option<Result<soroban_sdk::Error, soroban_sdk::InvokeError>>;

/// Signature manquante ou ne couvrant pas l'appel : erreur hôte, pas un code du contrat.
fn auth_refused() -> TryErr {
    use soroban_sdk::xdr::{ScErrorCode, ScErrorType};
    Some(Ok(soroban_sdk::Error::from_type_and_code(ScErrorType::Context, ScErrorCode::InvalidAction)))
}

/// Erreur du SAC USDC remontée telle quelle par le pool (10 solde insuffisant, 11 solde gelé).
fn sac_err(code: u32) -> TryErr {
    Some(Ok(soroban_sdk::Error::from_contract_error(code)))
}

fn leaf(f: AuthorizedFunction) -> AuthorizedInvocation {
    AuthorizedInvocation { function: f, sub_invocations: std::vec![] }
}

// ======================= 1. Autorisations =======================

/// deposit_lp : le LP signe l'appel ET le transfert USDC qu'il déclenche, personne d'autre.
#[test]
fn auth_deposit_lp_tree_is_lp_only() {
    let s = net();
    let lp = s.allowed_lp();
    s.mint(&lp, 1_000 * USDC);
    s.client().deposit_lp(&lp, &(1_000 * USDC));
    let e = &s.env;
    assert_eq!(
        e.auths(),
        std::vec![(
            lp.clone(),
            AuthorizedInvocation {
                function: func(e, &s.pool, "deposit_lp", (lp.clone(), 1_000 * USDC).into_val(e)),
                sub_invocations: std::vec![leaf(func(
                    e,
                    &s.usdc,
                    "transfer",
                    (lp.clone(), s.pool.clone(), 1_000 * USDC).into_val(e)
                ))],
            }
        )]
    );
}

/// withdraw : seul le LP signe ; le transfert sortant est autorisé par le contrat lui-même.
#[test]
fn auth_withdraw_tree_is_lp_only() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    s.client().withdraw(&lp, &(400 * USDC));
    let e = &s.env;
    assert_eq!(
        e.auths(),
        std::vec![(lp.clone(), leaf(func(e, &s.pool, "withdraw", (lp.clone(), 400 * USDC).into_val(e))))]
    );
}

#[test]
fn auth_senior_trees_are_lp_only() {
    let s = net();
    s.new_lp(200_000 * USDC);
    let lp = s.allowed_lp();
    s.mint(&lp, 800_000 * USDC);
    s.client().deposit_senior(&lp, &(800_000 * USDC));
    let e = &s.env;
    assert_eq!(
        e.auths(),
        std::vec![(
            lp.clone(),
            AuthorizedInvocation {
                function: func(e, &s.pool, "deposit_senior", (lp.clone(), 800_000 * USDC).into_val(e)),
                sub_invocations: std::vec![leaf(func(
                    e,
                    &s.usdc,
                    "transfer",
                    (lp.clone(), s.pool.clone(), 800_000 * USDC).into_val(e)
                ))],
            }
        )]
    );
    s.client().withdraw_senior(&lp, &(100_000 * USDC));
    assert_eq!(
        e.auths(),
        std::vec![(lp.clone(), leaf(func(e, &s.pool, "withdraw_senior", (lp.clone(), 100_000 * USDC).into_val(e))))]
    );
}

/// Un tiers qui signe « withdraw(victime, …) » ne retire rien : l'auth exigée est celle de la victime.
#[test]
fn auth_third_party_cannot_withdraw_someone_elses_shares() {
    let s = net();
    let victim = s.new_lp(1_000 * USDC);
    let senior_victim = s.new_senior(200 * USDC);
    let thief = Address::generate(&s.env);
    let before = s.books(&victim);
    let before_senior = s.books(&senior_victim);
    s.require_real_auths();

    s.env.mock_auths(&[MockAuth {
        address: &thief,
        invoke: &MockAuthInvoke {
            contract: &s.pool,
            fn_name: "withdraw",
            args: (victim.clone(), 1_000 * USDC).into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(s.client().try_withdraw(&victim, &(1_000 * USDC)).err(), auth_refused());

    s.env.mock_auths(&[MockAuth {
        address: &thief,
        invoke: &MockAuthInvoke {
            contract: &s.pool,
            fn_name: "withdraw_senior",
            args: (senior_victim.clone(), 200 * USDC).into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(s.client().try_withdraw_senior(&senior_victim, &(200 * USDC)).err(), auth_refused());

    // Et avec ses propres signatures, le voleur n'a pas de parts : 106 / 112.
    s.env.mock_all_auths();
    assert_eq!(
        s.client().try_withdraw(&thief, &(1_000 * USDC)).err(),
        Some(Ok(VaultError::InsufficientShares.into()))
    );
    assert_eq!(
        s.client().try_withdraw_senior(&thief, &1).err(),
        Some(Ok(VaultError::InsufficientSeniorShares.into()))
    );
    assert_eq!(s.books(&victim), before);
    assert_eq!(s.books(&senior_victim), before_senior);
    assert_eq!(s.usdc_balance(&thief), 0);
}

/// Une signature vaut pour des arguments précis : signer un retrait de 100 parts n'autorise pas 1 000.
#[test]
fn auth_signature_is_bound_to_the_amount() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    s.require_real_auths();
    s.env.mock_auths(&[MockAuth {
        address: &lp,
        invoke: &MockAuthInvoke {
            contract: &s.pool,
            fn_name: "withdraw",
            args: (lp.clone(), 100 * USDC).into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(s.client().try_withdraw(&lp, &(1_000 * USDC)).err(), auth_refused());
    assert_eq!(s.client().lp_shares(&lp), 1_000 * USDC);
    // La signature exacte passe, une seule fois (nonce consommé).
    assert_eq!(s.client().withdraw(&lp, &(100 * USDC)), 100 * USDC);
    assert_eq!(s.client().try_withdraw(&lp, &(100 * USDC)).err(), auth_refused());
}

/// Un dépôt signé sans le sous-appel `transfer` échoue : le portefeuille doit montrer les deux.
#[test]
fn auth_deposit_without_signed_transfer_fails() {
    let s = net();
    let lp = s.allowed_lp();
    s.mint(&lp, 1_000 * USDC);
    let before = s.books(&lp);
    s.require_real_auths();
    s.env.mock_auths(&[MockAuth {
        address: &lp,
        invoke: &MockAuthInvoke {
            contract: &s.pool,
            fn_name: "deposit_lp",
            args: (lp.clone(), 1_000 * USDC).into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(s.client().try_deposit_lp(&lp, &(1_000 * USDC)).err(), auth_refused());
    assert_eq!(s.books(&lp), before);
}

/// Réglages du vault : la signature d'un LP (ou de quiconque sauf l'admin) ne suffit pas.
#[test]
fn auth_vault_settings_refuse_a_lp_signature() {
    let s = net();
    let lp = s.allowed_lp();
    let other = Address::generate(&s.env);
    s.require_real_auths();
    let e = &s.env;
    let signed_by_lp = |name: &'static str, args: soroban_sdk::Vec<Val>| {
        e.mock_auths(&[MockAuth {
            address: &lp,
            invoke: &MockAuthInvoke { contract: &s.pool, fn_name: name, args, sub_invokes: &[] },
        }]);
    };
    let c = s.client();
    signed_by_lp("set_currency_limit", (symbol_short!("MXN"), 0u32).into_val(e));
    assert_eq!(c.try_set_currency_limit(&symbol_short!("MXN"), &0).err(), auth_refused());
    signed_by_lp("set_allowed", (Role::Lp, other.clone(), true).into_val(e));
    assert_eq!(c.try_set_allowed(&Role::Lp, &other, &true).err(), auth_refused());
    signed_by_lp("set_allowlist_enabled", (Role::Lp, false).into_val(e));
    assert_eq!(c.try_set_allowlist_enabled(&Role::Lp, &false).err(), auth_refused());
    signed_by_lp("set_notional_cap", (Some(0i128),).into_val(e));
    assert_eq!(c.try_set_notional_cap(&Some(0)).err(), auth_refused());

    assert_eq!(c.currency_limit(&symbol_short!("MXN")), None);
    assert!(!c.is_allowed(&Role::Lp, &other));
    assert!(c.pilot_limits().lp_allowlist_on);
    assert_eq!(c.pilot_limits().notional_cap, None);

    // Signée par l'admin avec exactement ces arguments : acceptée.
    e.mock_auths(&[MockAuth {
        address: &s.admin,
        invoke: &MockAuthInvoke {
            contract: &s.pool,
            fn_name: "set_notional_cap",
            args: (Some(5i128),).into_val(e),
            sub_invokes: &[],
        },
    }]);
    c.set_notional_cap(&Some(5));
    assert_eq!(c.pilot_limits().notional_cap, Some(5));
}

/// Les lectures n'exigent aucune signature.
#[test]
fn auth_reads_need_no_signature() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    s.require_real_auths();
    let c = s.client();
    c.lp_balance(&lp);
    c.lp_shares(&lp);
    c.share_price();
    c.exit_price();
    c.capacity();
    c.total_assets();
    c.senior_balance(&lp);
    c.tranche_values();
    c.exposure(&symbol_short!("MXN"));
    c.currency_limit(&symbol_short!("MXN"));
    c.pilot_limits();
    c.total_notional();
    c.is_allowed(&Role::Lp, &lp);
    assert!(s.env.auths().is_empty());
}

// ======================= 2. Durée de vie du stockage =======================

const LEDGERS_PER_DAY: u64 = 17_280;
const DAY: u64 = LEDGERS_PER_DAY * SECS_PER_LEDGER;

fn persistent_ttl(s: &Net, key: &VaultKey) -> u32 {
    s.vault(|env| env.storage().persistent().get_ttl(key))
}
fn instance_ttl(s: &Net) -> u32 {
    s.vault(|env| env.storage().instance().get_ttl())
}
fn has(s: &Net, key: &VaultKey) -> bool {
    s.vault(|env| env.storage().persistent().has(key))
}

/// Retraits : prolongent l'instance et les parts restantes, suppriment la clé à zéro.
#[test]
fn ttl_withdrawals_extend_or_remove_entries() {
    let s = net();
    let lp = s.new_lp(200_000 * USDC);
    let senior = s.new_senior(100_000 * USDC);
    s.advance_secs(100 * DAY);
    s.client().withdraw(&lp, &(1_000 * USDC));
    assert_eq!(instance_ttl(&s), TTL_EXTEND_TO);
    assert_eq!(persistent_ttl(&s, &VaultKey::Shares(lp.clone())), TTL_EXTEND_TO);
    s.client().withdraw_senior(&senior, &(1_000 * USDC));
    assert_eq!(persistent_ttl(&s, &VaultKey::SeniorShares(senior.clone())), TTL_EXTEND_TO);

    let senior_rest = s.vault(|env| senior_shares(env, &senior));
    s.client().withdraw_senior(&senior, &senior_rest);
    let rest = s.client().lp_shares(&lp);
    s.client().withdraw(&lp, &rest);
    assert!(!has(&s, &VaultKey::Shares(lp)));
    assert!(!has(&s, &VaultKey::SeniorShares(senior)));
}

/// La lecture de la liste blanche par un dépôt prolonge l'entrée (LP autorisé il y a 100 jours).
#[test]
fn ttl_allowlist_entry_extended_by_deposit() {
    let s = net();
    let lp = s.allowed_lp();
    s.advance_secs(100 * DAY);
    s.mint(&lp, USDC);
    s.client().deposit_lp(&lp, &USDC);
    assert_eq!(persistent_ttl(&s, &VaultKey::LpAllowed(lp)), TTL_EXTEND_TO);
}

/// Le règlement passe par close_position : exposition supprimée à zéro, instance prolongée.
#[test]
fn ttl_settle_extends_instance_and_clears_exposure() {
    let s = net();
    s.new_lp(100_000 * USDC);
    let (id, _) = s.open_hedge(MXN_1M, 100 * DAY);
    assert_eq!(persistent_ttl(&s, &VaultKey::Exposure(symbol_short!("MXN"))), TTL_EXTEND_TO);
    // 100 jours : le TTL de l'instance (120 j) est passé sous le seuil de 30 j.
    s.advance_secs(100 * DAY);
    s.set_mxn(MXN_SPOT);
    s.client().settle(&id);
    assert!(!has(&s, &VaultKey::Exposure(symbol_short!("MXN"))));
    assert_eq!(instance_ttl(&s), TTL_EXTEND_TO);
}

/// Un LP passif 120 jours + 1 ledger : son entrée de parts est archivée. Le test env (comme le
/// réseau depuis le protocole 23) la restaure automatiquement si elle est dans l'empreinte :
/// rien n'est perdu, mais la transaction paie une lecture disque et la restauration.
#[test]
fn ttl_archived_lp_shares_are_restored_not_lost() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    s.advance_secs(120 * DAY);
    assert_eq!(persistent_ttl(&s, &VaultKey::Shares(lp.clone())), 0); // dernier ledger vivant
    s.advance_secs(SECS_PER_LEDGER);
    assert_eq!(s.client().withdraw(&lp, &(1_000 * USDC)), 1_000 * USDC);
    let r = s.env.cost_estimate().resources();
    std::println!("retrait après archivage : {r:?}");
    assert!(r.disk_read_entries >= 1, "l'entrée de parts a dû être restaurée");
}

/// Entrées touchées par le règlement d'une couverture : (live_until de l'exposition, de la couverture).
fn live_until_after_open(s: &Net, id: u64) -> (u32, u32) {
    let seq = s.env.ledger().sequence();
    (
        seq + persistent_ttl(s, &VaultKey::Exposure(symbol_short!("MXN"))),
        seq + s.vault(|env| env.storage().persistent().get_ttl(&crate::DataKey::Hedge(id))),
    )
}

/// Couverture de 120 jours sans autre activité sur MXN : à l'échéance + délai de fixing (1 h),
/// l'exposition (vault) et la couverture (hedge) ont dépassé leur TTL de 120 jours. Le règlement
/// passe quand même (restauration automatique), mais plus cher, et le keeper doit le prévoir.
/// ROUGE hors liste : à corriger en alignant le TTL sur l'échéance + marge (B9 côté couverture,
/// et `open_position` côté exposition), ou en bornant la durée à moins de 120 j − délai.
#[test]
#[ignore = "TTL hors liste : exposition et couverture archivées avant le règlement d'une couverture de 120 jours"]
fn ttl_120_day_hedge_entries_live_until_settlement_deadline() {
    let s = net();
    s.new_lp(100_000 * USDC);
    let (id, _) = s.open_hedge(MXN_1M, 120 * 86_400);
    let (exposure_until, hedge_until) = live_until_after_open(&s, id);
    s.advance_secs(120 * 86_400 + crate::pricing::FIXING_GRACE_SECS);
    let now = s.env.ledger().sequence();
    assert!(exposure_until >= now, "exposition archivée avant la fin du délai de fixing");
    assert!(hedge_until >= now, "couverture archivée avant la fin du délai de fixing");
}

/// VERT : malgré l'archivage, le règlement aboutit et restaure des entrées (lecture disque).
#[test]
fn ttl_120_day_hedge_still_settles_with_restore() {
    let s = net();
    s.new_lp(100_000 * USDC);
    let (id, _) = s.open_hedge(MXN_1M, 120 * 86_400);
    s.advance_secs(120 * 86_400);
    s.set_mxn(MXN_SPOT);
    s.advance_secs(crate::pricing::FIXING_GRACE_SECS);
    s.client().settle(&id);
    let r = s.env.cost_estimate().resources();
    std::println!("settle après 120 j + 1 h : {r:?}");
    assert!(r.disk_read_entries >= 2);
    assert_eq!(s.client().capacity(), s.client().total_assets());
}

// ======================= 3. Événements =======================

/// Bout en bout : open_hedge publie PositionReserved puis HedgeOpened ; settle publie
/// PositionReleased puis HedgeSettled. Rien d'autre côté pool.
#[test]
fn events_open_and_settle_through_entry_points() {
    let s = net();
    s.new_lp(100_000 * USDC);
    let (id, sme) = s.open_hedge(MXN_1M, 60);
    let opened = s.pool_events(); // avant tout autre appel : events() = dernière invocation
    let h = s.client().get_hedge(&id).unwrap();
    assert_eq!(
        opened,
        [
            PositionReserved { currency: symbol_short!("MXN"), side: Side::BuyCurrency, margin: h.margin }
                .to_xdr(&s.env, &s.pool),
            crate::hedge::HedgeOpened {
                hedge_id: id,
                owner: sme.clone(),
                supplier: h.supplier.clone(),
                currency: symbol_short!("MXN"),
                side: Side::BuyCurrency,
                notional: MXN_1M,
                locked_rate: h.locked_rate,
                cap_bps: 1000,
                margin: h.margin,
                maturity: h.maturity,
            }
            .to_xdr(&s.env, &s.pool),
        ]
    );
    s.advance_secs(60);
    s.set_mxn(MXN_SPOT);
    let payout = s.client().settle(&id);
    let pnl = payout - h.margin;
    assert_eq!(
        s.pool_events(),
        [
            PositionReleased { currency: symbol_short!("MXN"), side: Side::BuyCurrency, margin: h.margin }
                .to_xdr(&s.env, &s.pool),
            crate::hedge::HedgeSettled { hedge_id: id, owner: sme, spot: MXN_SPOT, pnl, payout }
                .to_xdr(&s.env, &s.pool),
        ]
    );
}

/// Un appel refusé ne publie rien (les événements sont annulés avec l'état).
#[test]
fn events_none_on_refused_calls() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    assert_eq!(s.client().try_withdraw(&lp, &(1_001 * USDC)).err(), Some(Ok(VaultError::InsufficientShares.into())));
    assert_eq!(s.pool_events().events().len(), 0);
    let stranger = Address::generate(&s.env);
    s.mint(&stranger, USDC);
    assert_eq!(s.client().try_deposit_lp(&stranger, &USDC).err(), Some(Ok(VaultError::LpNotAllowed.into())));
    assert_eq!(s.pool_events().events().len(), 0);
}

/// Les intérêts senior capitalisés par withdraw_senior ne publient aucun événement propre, et
/// SeniorWithdrew ne porte pas le prix servi : un indexeur doit le recalculer (usdc / parts).
/// VERT aujourd'hui (constat) ; à remplacer par le test FUTUR du plan §8 (prix dans SeniorWithdrew).
#[test]
fn events_senior_interest_has_no_event_today() {
    let s = net();
    s.new_lp(200_000 * USDC);
    let senior = s.new_senior(800_000 * USDC);
    s.advance_secs(31_536_000);
    let paid = s.client().withdraw_senior(&senior, &(100_000 * USDC));
    assert_eq!(paid, 104_500 * USDC);
    assert_eq!(
        s.pool_events(),
        [SeniorWithdrew { lp: senior, shares: 100_000 * USDC, usdc: paid }.to_xdr(&s.env, &s.pool)]
    );
}

// ======================= 4. Échecs de transfert USDC =======================

/// Dépôt d'un LP dont l'émetteur a gelé le compte USDC : refus, compta et soldes intacts.
#[test]
fn transfer_deposit_from_frozen_lp_leaves_books_intact() {
    let s = net();
    s.new_lp(1_000 * USDC);
    let lp = s.allowed_lp();
    s.mint(&lp, 500 * USDC);
    s.sac().set_authorized(&lp, &false);
    let before = s.books(&lp);
    let r = s.client().try_deposit_lp(&lp, &(500 * USDC));
    assert_eq!(r.err(), sac_err(11));
    let r = s.client().try_deposit_senior(&lp, &(100 * USDC));
    assert_eq!(r.err(), sac_err(11));
    assert_eq!(s.books(&lp), before);
    assert_eq!(s.pool_events().events().len(), 0);
}

/// Dépôt au-delà du solde : refus (erreur du SAC), compta intacte.
#[test]
fn transfer_deposit_above_balance_leaves_books_intact() {
    let s = net();
    let lp = s.allowed_lp();
    s.mint(&lp, 100 * USDC);
    let before = s.books(&lp);
    assert_eq!(s.client().try_deposit_lp(&lp, &(100 * USDC + 1)).err(), sac_err(10));
    assert_eq!(s.books(&lp), before);
}

/// Retraits vers un compte gelé : l'état est écrit AVANT le transfert, mais l'échec du transfert
/// annule toute l'invocation. Parts intactes ; une fois dégelé, le LP récupère tout.
#[test]
fn transfer_withdrawals_to_frozen_lp_revert_state() {
    let s = net();
    let lp = s.new_lp(200_000 * USDC);
    let senior = s.new_senior(100_000 * USDC);
    s.sac().set_authorized(&lp, &false);
    s.sac().set_authorized(&senior, &false);
    let before = s.books(&lp);
    let before_senior = s.books(&senior);

    let r = s.client().try_withdraw(&lp, &(1_000 * USDC));
    assert_eq!(r.err(), sac_err(11));
    assert_eq!(s.client().try_withdraw_senior(&senior, &(1_000 * USDC)).err(), sac_err(11));
    assert_eq!(s.books(&lp), before);
    assert_eq!(s.books(&senior), before_senior);
    assert_eq!(s.pool_events().events().len(), 0);

    s.sac().set_authorized(&lp, &true);
    s.sac().set_authorized(&senior, &true);
    assert_eq!(s.client().withdraw_senior(&senior, &(100_000 * USDC)), 100_000 * USDC);
    assert_eq!(s.client().withdraw(&lp, &(200_000 * USDC)), 200_000 * USDC);
}

/// L'émetteur gèle le contrat lui-même : aucun retrait ni dépôt possible, mais rien ne bouge.
#[test]
fn transfer_frozen_pool_blocks_flows_without_corrupting_books() {
    let s = net();
    let lp = s.new_lp(1_000 * USDC);
    s.mint(&lp, 1_000 * USDC);
    s.sac().set_authorized(&s.pool, &false);
    let before = s.books(&lp);
    assert_eq!(s.client().try_withdraw(&lp, &(1_000 * USDC)).err(), sac_err(11));
    assert_eq!(s.client().try_deposit_lp(&lp, &(1_000 * USDC)).err(), sac_err(11));
    assert_eq!(s.books(&lp), before);
}

/// Règlement vers une PME gelée : `settle` échoue en bloc, la couverture reste ouverte et sa marge
/// reste réservée (capital des LPs immobilisé). VERT aujourd'hui ; B4 (solde réclamable) le
/// transforme : `settle` doit réussir et créditer `claimable(owner)` (test FUTUR).
#[test]
fn transfer_settle_to_frozen_owner_reverts_and_keeps_reserve() {
    let s = net();
    let lp = s.new_lp(100_000 * USDC);
    let (id, sme) = s.open_hedge(MXN_1M, 60);
    s.advance_secs(60);
    s.set_mxn(MXN_SPOT);
    s.sac().set_authorized(&sme, &false);
    let before = s.books(&lp);
    let r = s.client().try_settle(&id);
    assert_eq!(r.err(), sac_err(11));
    assert_eq!(s.books(&lp), before);
    assert!(!s.client().get_hedge(&id).unwrap().settled);
    assert!(s.client().capacity() < s.client().total_assets());
}

// ======================= 5. Budget de ressources =======================

/// Mesure de la dernière invocation (contrat natif : ne compte pas l'exécution WASM).
fn last_cost(s: &Net) -> (i64, i64, u32, u32) {
    let r = s.env.cost_estimate().resources();
    (r.instructions, r.mem_bytes, r.memory_read_entries + r.disk_read_entries, r.write_entries)
}

/// Plafonds par opération, ~2× la mesure du 24/11 (contrat natif). À relever sciemment si un
/// correctif ajoute du travail, jamais en silence.
#[test]
fn budget_each_operation_under_ceiling() {
    let s = net();
    let c = s.client();
    let lp = s.allowed_lp();
    s.mint(&lp, 300_000 * USDC);
    c.deposit_lp(&lp, &(300_000 * USDC));
    let deposit = last_cost(&s);
    let senior = s.new_senior(100_000 * USDC);
    let deposit_senior = last_cost(&s);
    let (id, _) = s.open_hedge(MXN_1M, 60);
    let open = last_cost(&s);
    c.withdraw(&lp, &(1_000 * USDC));
    let withdraw = last_cost(&s);
    s.advance_secs(86_400);
    c.withdraw_senior(&senior, &(1_000 * USDC));
    let withdraw_senior = last_cost(&s);
    s.set_mxn(MXN_SPOT);
    let _ = id;
    let (id2, _) = s.open_hedge(MXN_1M, 60);
    s.advance_secs(60);
    s.set_mxn(MXN_SPOT);
    c.settle(&id2);
    let settle = last_cost(&s);
    std::println!(
        "deposit {deposit:?}\ndeposit_senior {deposit_senior:?}\nopen {open:?}\nwithdraw {withdraw:?}\nwithdraw_senior {withdraw_senior:?}\nsettle {settle:?}"
    );
    for (name, (cpu, mem, reads, writes), max_cpu) in [
        ("deposit_lp", deposit, 600_000),
        ("deposit_senior", deposit_senior, 600_000),
        ("withdraw", withdraw, 600_000),
        ("withdraw_senior", withdraw_senior, 600_000),
        ("open_hedge", open, 1_200_000),
        ("settle", settle, 1_200_000),
    ] {
        assert!(cpu <= max_cpu, "{name} : {cpu} instructions > {max_cpu}");
        assert!(mem <= 300_000, "{name} : {mem} octets");
        assert!(reads <= 12 && writes <= 12, "{name} : {reads} lectures, {writes} écritures");
    }
}

/// Coût constant : un dépôt, un retrait et un règlement lisent et écrivent le même nombre
/// d'entrées avec 1 ou 40 couvertures ouvertes. Échouera si un correctif boucle sur les
/// couvertures (B3 : borner par le maximum de couvertures ouvertes de B9 et mesurer à ce maximum).
/// Les instructions, elles, montent d'environ 50 % dans le test env (coût hôte du stockage qui
/// grossit, pas du code du vault) : seule une borne lâche (×2) est donc posée dessus.
#[test]
fn budget_constant_in_number_of_open_hedges() {
    fn measure(n: u32) -> [(i64, u32); 3] {
        let s = net();
        let lp = s.new_lp(1_000_000 * USDC);
        s.mint(&lp, USDC);
        let mut first = 0;
        for i in 0..n {
            let (id, _) = s.open_hedge(MXN_1M / 10, 60);
            if i == 0 {
                first = id;
            }
        }
        let c = s.client();
        c.deposit_lp(&lp, &USDC);
        let d = last_cost(&s);
        c.withdraw(&lp, &USDC);
        let w = last_cost(&s);
        s.advance_secs(60);
        s.set_mxn(MXN_SPOT);
        c.settle(&first);
        let st = last_cost(&s);
        [(d.0, d.2 + d.3), (w.0, w.2 + w.3), (st.0, st.2 + st.3)]
    }
    let one = measure(1);
    let many = measure(40);
    std::println!("1 couverture {one:?}\n40 couvertures {many:?}");
    for (a, b) in one.iter().zip(many.iter()) {
        assert_eq!(a.1, b.1, "nombre d'entrées lues/écrites");
        assert!(b.0 <= 2 * a.0, "instructions : {} contre {}", a.0, b.0);
    }
}

// ======================= 6. Codes d'erreur =======================

/// (nom, code) de chaque cas d'une enum d'erreur, lus dans la spec XDR générée par #[contracterror] :
/// un cas ajouté est vu sans modifier ce test.
fn error_cases(spec: &[u8]) -> std::vec::Vec<(std::string::String, u32)> {
    use soroban_sdk::xdr::{Limits, ReadXdr, ScSpecEntry};
    match ScSpecEntry::from_xdr(spec, Limits::none()).unwrap() {
        ScSpecEntry::UdtErrorEnumV0(e) => {
            e.cases.iter().map(|c| (c.name.to_utf8_string_lossy(), c.value)).collect()
        }
        other => panic!("pas une enum d'erreur : {other:?}"),
    }
}

#[test]
fn error_codes_unique_and_in_module_ranges() {
    let modules = [
        ("VaultError", error_cases(&VaultError::spec_xdr()), 100..=199u32),
        ("HedgeError", error_cases(&crate::hedge::HedgeError::spec_xdr()), 200..=299),
        ("PricingError", error_cases(&crate::pricing::PricingError::spec_xdr()), 300..=399),
        ("AdminError", error_cases(&crate::admin::AdminError::spec_xdr()), 400..=499),
    ];
    let mut seen = std::collections::BTreeMap::new();
    for (module, cases, range) in &modules {
        assert!(!cases.is_empty());
        for (name, code) in cases {
            assert!(range.contains(code), "{module}::{name} = {code} hors de {range:?}");
            // Jamais sous 100 : les erreurs du SAC USDC (0 à 13) remontent elles aussi en Error(Contract, #n).
            assert!(*code >= 100);
            if let Some(prev) = seen.insert(*code, std::format!("{module}::{name}")) {
                panic!("code {code} utilisé deux fois : {prev} et {module}::{name}");
            }
        }
    }
    // 113 reste réservé (voir l'en-tête de VaultError) tant que son lot n'est pas écrit.
    assert!(!seen.contains_key(&113), "113 attribué : mettre à jour la réservation");
}

// ======================= 7. Bout en bout du défaut 2 (B5) =======================

/// Défaut 2 par les vrais points d'entrée : pool 200 000 junior / 800 000 senior, une couverture
/// d'un an dont la marge (≈ 188 000) occupe presque tout le junior. Un an plus tard, les intérêts
/// senior (36 000) font passer le junior sous Reserved ; le senior retire tout (836 000),
/// T = 164 000 < R, et le règlement perdant à pleine marge échoue en 103. Après B5 : le retrait
/// est limité au payable (T − R), le règlement passe.
fn thin_junior_after_one_year() -> (Net, u64, Address) {
    let s = net();
    s.new_lp(200_000 * USDC);
    let senior = s.new_senior(800_000 * USDC);
    let (id, _) = s.open_hedge(35 * MXN_1M, 31_536_000);
    let margin = s.client().get_hedge(&id).unwrap().margin;
    assert!(margin > 164_000 * USDC && margin < 200_000 * USDC, "marge {margin}");
    s.advance_secs(31_536_000);
    s.set_mxn(MXN_SPOT * 2); // peso +100 % : perte du pool plafonnée à la marge
    (s, id, senior)
}

/// ROUGE (B5) : après un retrait senior total, un règlement perdant ne doit jamais échouer.
#[test]
fn e2e_defect2_settle_after_full_senior_exit_never_fails() {
    let (s, id, senior) = thin_junior_after_one_year();
    let shares = s.vault(|env| senior_shares(env, &senior));
    let _ = s.client().try_withdraw_senior(&senior, &shares);
    let reserved = s.vault(reserved);
    assert!(s.client().total_assets() >= reserved, "T < R après le retrait senior");
    assert!(s.client().try_settle(&id).is_ok());
}
