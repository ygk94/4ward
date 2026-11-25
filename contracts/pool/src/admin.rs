// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Admin : transfert d'admin en deux temps, garde et pause, mise à jour du code, contrôle de l'oracle.
//!
//! La pause bloque seulement ce qui fait entrer de l'argent ou du risque (`deposit_lp`, `deposit_senior`,
//! `open_hedge`). Retraits, règlement, clôture anticipée et lectures restent ouverts : on ne bloque jamais
//! l'argent des gens.
use crate::pricing::OracleClient;
use crate::{DataKey, PRICE_DECIMALS};
use soroban_sdk::{contracterror, contractevent, contracttype, panic_with_error, Address, BytesN, ContractExecutable, Env,
};

/// Erreurs de l'admin. Codes 400+ (Vault 100+, Paiement 200+, Pricing 300+).
/// Ne jamais réutiliser un numéro : ajouter à la suite.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AdminError {
    /// Dépôt ou ouverture de couverture pendant la pause.
    Paused = 400,
    /// Acceptation ou annulation sans proposition en cours.
    NoPendingAdmin = 401,
    /// `pause` demandée par une adresse qui n'est ni la garde ni l'admin.
    NotGuardian = 402,
    /// L'oracle ne publie pas ses prix avec 14 décimales.
    InvalidOracleDecimals = 403,
    /// La garde doit être une clé distincte de l'admin.
    GuardianIsAdmin = 404,
    /// Fonction pas encore branchée. N'est plus renvoyé depuis que la liste blanche `Role::Sme` est branchée
    /// (B9, côté couvertures) ; numéro gardé pour ne jamais le réutiliser.
    NotImplemented = 405,
}

#[contracttype]
enum AdminKey {
    PendingAdmin,
    Guardian,
    Paused,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposed {
    #[topic]
    pub current: Address,
    #[topic]
    pub proposed: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferred {
    #[topic]
    pub previous: Address,
    #[topic]
    pub new_admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposalCancelled {
    #[topic]
    pub proposed: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardianSet {
    #[topic]
    pub guardian: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paused {
    #[topic]
    pub by: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unpaused {
    #[topic]
    pub by: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upgraded {
    pub new_wasm_hash: BytesN<32>,
}

// ---- contrôles appelés par lib.rs ----

/// Refuse un oracle dont les prix n'ont pas 14 décimales (toute la formule de prix en dépend).
pub(crate) fn check_oracle_decimals(env: &Env, oracle: &Address) {
    if OracleClient::new(env, oracle).decimals() != PRICE_DECIMALS {
        panic_with_error!(env, AdminError::InvalidOracleDecimals);
    }
}

pub(crate) fn require_not_paused(env: &Env) {
    if paused(env) {
        panic_with_error!(env, AdminError::Paused);
    }
}

/// La garde ne peut pas devenir admin (sinon une seule clé ferait les deux rôles).
fn require_not_guardian(env: &Env, who: &Address) {
    if guardian(env).as_ref() == Some(who) {
        panic_with_error!(env, AdminError::GuardianIsAdmin);
    }
}

// ---- transfert d'admin ----

/// Refuse `GuardianIsAdmin` (404) si `new_admin` est la garde.
pub(crate) fn propose_admin(env: &Env, new_admin: &Address) {
    let current = crate::admin(env);
    current.require_auth();
    require_not_guardian(env, new_admin);
    env.storage().instance().set(&AdminKey::PendingAdmin, new_admin);
    AdminProposed { current, proposed: new_admin.clone() }.publish(env);
}

/// Signée par l'admin proposé : il reçoit le rôle, l'ancien admin perd ses droits.
/// Refuse `GuardianIsAdmin` (404) si le proposé est devenu la garde depuis la proposition.
pub(crate) fn accept_admin(env: &Env) {
    let new_admin = pending_admin(env).unwrap_or_else(|| panic_with_error!(env, AdminError::NoPendingAdmin));
    new_admin.require_auth();
    require_not_guardian(env, &new_admin);
    let previous = crate::admin(env);
    let s = env.storage().instance();
    s.set(&DataKey::Admin, &new_admin);
    s.remove(&AdminKey::PendingAdmin);
    AdminTransferred { previous, new_admin }.publish(env);
}

pub(crate) fn cancel_admin_proposal(env: &Env) {
    crate::admin(env).require_auth();
    let proposed = pending_admin(env).unwrap_or_else(|| panic_with_error!(env, AdminError::NoPendingAdmin));
    env.storage().instance().remove(&AdminKey::PendingAdmin);
    AdminProposalCancelled { proposed }.publish(env);
}

pub(crate) fn pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&AdminKey::PendingAdmin)
}

// ---- garde et pause ----

pub(crate) fn set_guardian(env: &Env, guardian: &Address) {
    let admin = crate::admin(env);
    admin.require_auth();
    if *guardian == admin {
        panic_with_error!(env, AdminError::GuardianIsAdmin);
    }
    env.storage().instance().set(&AdminKey::Guardian, guardian);
    GuardianSet { guardian: guardian.clone() }.publish(env);
}

pub(crate) fn guardian(env: &Env) -> Option<Address> {
    env.storage().instance().get(&AdminKey::Guardian)
}

/// Garde ou admin, signature de `caller`.
pub(crate) fn pause(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != crate::admin(env) && guardian(env).as_ref() != Some(caller) {
        panic_with_error!(env, AdminError::NotGuardian);
    }
    env.storage().instance().set(&AdminKey::Paused, &true);
    Paused { by: caller.clone() }.publish(env);
}

/// Admin seul.
pub(crate) fn unpause(env: &Env) {
    let admin = crate::admin(env);
    admin.require_auth();
    env.storage().instance().set(&AdminKey::Paused, &false);
    Unpaused { by: admin }.publish(env);
}

pub(crate) fn paused(env: &Env) -> bool {
    env.storage().instance().get(&AdminKey::Paused).unwrap_or(false)
}

// ---- mise à jour du code ----

/// Remplace le WASM du contrat ; adresse et données inchangées. Le hash doit déjà être téléversé.
pub(crate) fn upgrade(env: &Env, new_wasm_hash: &BytesN<32>) {
    crate::admin(env).require_auth();
    env.deployer().update_current_contract(ContractExecutable::Wasm(new_wasm_hash.clone()));
    Upgraded { new_wasm_hash: new_wasm_hash.clone() }.publish(env);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pool, PoolClient, Role, Side};
    use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
    use soroban_sdk::testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke};
    use soroban_sdk::token::StellarAssetClient;
    use soroban_sdk::{contract, contractimpl, Event as _, IntoVal, Symbol};

    #[contract]
    struct EightDecimalsOracle;

    #[contractimpl]
    impl EightDecimalsOracle {
        pub fn decimals(_env: Env) -> u32 {
            8
        }
    }

    struct Setup {
        env: Env,
        admin: Address,
        pool: Address,
        usdc: Address,
    }

    impl Setup {
        fn client(&self) -> PoolClient<'_> {
            PoolClient::new(&self.env, &self.pool)
        }
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let oracle = env.register(MockOracle, (&admin,));
        MockOracleClient::new(&env, &oracle)
            .set_price(&OracleAsset::Other(Symbol::new(&env, "MXN")), &5_500_000_000_000);
        let pool = env.register(Pool, (&admin, &usdc, &oracle, 50u32));
        Setup { env, admin, pool, usdc }
    }

    fn assert_admin_error<T, E: core::fmt::Debug>(
        r: Result<T, Result<soroban_sdk::Error, E>>,
        code: AdminError,
    ) {
        match r {
            Err(Ok(e)) => assert_eq!(e, soroban_sdk::Error::from_contract_error(code as u32)),
            _ => panic!("erreur {:?} attendue", code),
        }
    }

    // ---- décimales de l'oracle ----

    #[test]
    #[should_panic(expected = "Error(Contract, #403)")]
    fn constructor_rejects_oracle_without_14_decimals() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let oracle = env.register(EightDecimalsOracle, ());
        env.register(Pool, (&admin, &usdc, &oracle, 50u32));
    }

    #[test]
    fn constructor_accepts_14_decimals_oracle() {
        let s = setup();
        assert_eq!(s.client().get_admin(), s.admin);
    }

    // ---- transfert d'admin ----

    #[test]
    fn two_step_transfer_needs_new_admin_signature() {
        let s = setup();
        let c = s.client();
        let new_admin = Address::generate(&s.env);

        c.propose_admin(&new_admin);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [AdminProposed { current: s.admin.clone(), proposed: new_admin.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert_eq!(c.pending_admin(), Some(new_admin.clone()));
        // proposé n'est pas encore admin
        assert_eq!(c.get_admin(), s.admin);

        c.accept_admin();
        assert_eq!(s.env.auths()[0].0, new_admin);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [AdminTransferred { previous: s.admin.clone(), new_admin: new_admin.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert_eq!(c.get_admin(), new_admin);
        assert_eq!(c.pending_admin(), None);
    }

    #[test]
    fn accept_fails_without_new_admin_signature() {
        let s = setup();
        let c = s.client();
        let new_admin = Address::generate(&s.env);
        c.propose_admin(&new_admin);

        // seul l'ancien admin signe : refusé
        s.env.mock_auths(&[MockAuth {
            address: &s.admin,
            invoke: &MockAuthInvoke {
                contract: &s.pool,
                fn_name: "accept_admin",
                args: ().into_val(&s.env),
                sub_invokes: &[],
            },
        }]);
        assert!(c.try_accept_admin().is_err());
        assert_eq!(c.get_admin(), s.admin);
    }

    #[test]
    fn old_admin_loses_rights_after_transfer() {
        let s = setup();
        let c = s.client();
        let new_admin = Address::generate(&s.env);
        c.propose_admin(&new_admin);
        c.accept_admin();

        // l'ancien admin seul signe set_spread : refusé
        s.env.mock_auths(&[MockAuth {
            address: &s.admin,
            invoke: &MockAuthInvoke {
                contract: &s.pool,
                fn_name: "set_spread",
                args: (60u32,).into_val(&s.env),
                sub_invokes: &[],
            },
        }]);
        assert!(c.try_set_spread(&60).is_err());

        s.env.mock_all_auths();
        c.set_spread(&60);
        assert_eq!(s.env.auths()[0].0, new_admin);
    }

    #[test]
    fn cancel_proposal() {
        let s = setup();
        let c = s.client();
        let new_admin = Address::generate(&s.env);
        assert_admin_error(c.try_cancel_admin_proposal(), AdminError::NoPendingAdmin);
        assert_admin_error(c.try_accept_admin(), AdminError::NoPendingAdmin);

        c.propose_admin(&new_admin);
        c.cancel_admin_proposal();
        assert_eq!(s.env.auths()[0].0, s.admin);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [AdminProposalCancelled { proposed: new_admin.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert_eq!(c.pending_admin(), None);
        assert_admin_error(c.try_accept_admin(), AdminError::NoPendingAdmin);
        assert_eq!(c.get_admin(), s.admin);
    }

    #[test]
    fn guardian_cannot_be_proposed_as_admin() {
        let s = setup();
        let c = s.client();
        let guardian = Address::generate(&s.env);
        c.set_guardian(&guardian);
        assert_admin_error(c.try_propose_admin(&guardian), AdminError::GuardianIsAdmin);
        assert_eq!(c.pending_admin(), None);
    }

    #[test]
    fn proposed_admin_made_guardian_cannot_accept() {
        let s = setup();
        let c = s.client();
        let x = Address::generate(&s.env);
        c.propose_admin(&x);
        c.set_guardian(&x);
        assert_admin_error(c.try_accept_admin(), AdminError::GuardianIsAdmin);
        assert_eq!(c.get_admin(), s.admin);
    }

    // ---- garde et pause ----

    #[test]
    fn guardian_must_differ_from_admin() {
        let s = setup();
        assert_admin_error(s.client().try_set_guardian(&s.admin), AdminError::GuardianIsAdmin);
    }

    #[test]
    fn guardian_pauses_only_admin_unpauses() {
        let s = setup();
        let c = s.client();
        let guardian = Address::generate(&s.env);
        let stranger = Address::generate(&s.env);

        c.set_guardian(&guardian);
        assert_eq!(s.env.auths()[0].0, s.admin);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [GuardianSet { guardian: guardian.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert_eq!(c.guardian(), Some(guardian.clone()));
        assert!(!c.paused());

        assert_admin_error(c.try_pause(&stranger), AdminError::NotGuardian);

        c.pause(&guardian);
        assert_eq!(s.env.auths()[0].0, guardian);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [Paused { by: guardian.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert!(c.paused());

        // la garde ne peut ni reprendre ni changer un réglage
        s.env.mock_auths(&[MockAuth {
            address: &guardian,
            invoke: &MockAuthInvoke {
                contract: &s.pool,
                fn_name: "unpause",
                args: ().into_val(&s.env),
                sub_invokes: &[],
            },
        }]);
        assert!(c.try_unpause().is_err());
        s.env.mock_auths(&[MockAuth {
            address: &guardian,
            invoke: &MockAuthInvoke {
                contract: &s.pool,
                fn_name: "set_spread",
                args: (60u32,).into_val(&s.env),
                sub_invokes: &[],
            },
        }]);
        assert!(c.try_set_spread(&60).is_err());

        s.env.mock_all_auths();
        c.unpause();
        assert_eq!(s.env.auths()[0].0, s.admin);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [Unpaused { by: s.admin.clone() }.to_xdr(&s.env, &s.pool)]
        );
        assert!(!c.paused());
    }

    #[test]
    fn admin_can_pause_without_guardian() {
        let s = setup();
        s.client().pause(&s.admin);
        assert!(s.client().paused());
    }

    #[test]
    fn pause_blocks_deposits_and_opening_not_exits() {
        let s = setup();
        let c = s.client();
        let lp = Address::generate(&s.env);
        c.set_allowed(&Role::Lp, &lp, &true);
        let owner = Address::generate(&s.env);
        c.set_allowed(&Role::Sme, &owner, &true);
        let usdc_admin = StellarAssetClient::new(&s.env, &s.usdc);
        usdc_admin.mint(&lp, &2_000_000_000_000);
        usdc_admin.mint(&owner, &1_000_000_000_000);
        let mxn = Symbol::new(&s.env, "MXN");
        c.set_rate(&mxn, &700);
        c.set_rate(&Symbol::new(&s.env, "USD"), &400);
        c.set_min_cap(&mxn, &500);

        let shares = c.deposit_lp(&lp, &1_000_000_000_000);
        let senior = c.deposit_senior(&lp, &100_000_000_000);
        let id = c.open_hedge(&owner, &owner, &mxn, &Side::BuyCurrency, &10_000_000_000_000, &(90 * 86_400), &1000, &None);

        c.pause(&s.admin);
        assert_admin_error(c.try_deposit_lp(&lp, &1_000_000_000), AdminError::Paused);
        assert_admin_error(c.try_deposit_senior(&lp, &1_000_000_000), AdminError::Paused);
        assert_admin_error(
            c.try_open_hedge(&owner, &owner, &mxn, &Side::BuyCurrency, &10_000_000_000_000, &(90 * 86_400), &1000, &None),
            AdminError::Paused,
        );

        // sorties et lectures toujours possibles
        c.close_early(&id);
        assert!(c.withdraw(&lp, &(shares / 2)) > 0);
        assert!(c.withdraw_senior(&lp, &senior) > 0);
        c.capacity();
        c.quote(&mxn, &Side::BuyCurrency, &10_000_000_000_000, &(90 * 86_400), &1000);

        c.unpause();
        c.deposit_lp(&lp, &1_000_000_000);
    }

    // ---- événements des réglages de pricing ----

    #[test]
    fn pricing_settings_emit_events() {
        use crate::pricing::{MinCapSet, RateSet, SpreadSet};
        let s = setup();
        let c = s.client();
        let mxn = Symbol::new(&s.env, "MXN");
        c.set_rate(&mxn, &700);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [RateSet { currency: mxn.clone(), rate_bps: 700 }.to_xdr(&s.env, &s.pool)]
        );
        c.set_min_cap(&mxn, &500);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [MinCapSet { currency: mxn.clone(), cap_bps: 500 }.to_xdr(&s.env, &s.pool)]
        );
        c.set_spread(&60);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [SpreadSet { spread_bps: 60 }.to_xdr(&s.env, &s.pool)]
        );
    }

    // ---- mise à jour du code ----

    #[test]
    fn upgrade_requires_admin() {
        let s = setup();
        let stranger = Address::generate(&s.env);
        let hash = BytesN::from_array(&s.env, &[7u8; 32]);
        s.env.mock_auths(&[MockAuth {
            address: &stranger,
            invoke: &MockAuthInvoke {
                contract: &s.pool,
                fn_name: "upgrade",
                args: (hash.clone(),).into_val(&s.env),
                sub_invokes: &[],
            },
        }]);
        assert!(s.client().try_upgrade(&hash).is_err());
    }
}
