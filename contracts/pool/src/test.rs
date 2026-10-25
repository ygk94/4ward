#![cfg(test)]
//! Tests de départ, un bloc par personne : retirer les #[ignore] au fur et à mesure.
//! Scénario commun : 1 000 000 MXN, spot 0.055 USD/MXN, plafond 10 %, oracle = mock-oracle.

// ---- Vault ----
// Tests dans vault.rs (mod tests) : parts LP, dépôts / retraits, reserve / release / book_*.

// ---- Pricing ----

#[test]
#[ignore = "TODO pricing: r_MXN 7 %, r_USD 4 %, 90 j → F ≈ 0.05460 avant spread"]
fn pricing_forward_parity() {}

#[test]
#[ignore = "TODO pricing: le spread est contre l'utilisateur dans les deux sens"]
fn pricing_spread_direction() {}

#[test]
#[ignore = "TODO pricing: plafond sous le minimum de la devise refusé"]
fn pricing_min_cap() {}

// ---- Paiement ----

use crate::hedge::{self, HedgeError};
use crate::vault::VaultError;
use crate::{Hedge, Pool, PoolClient, Side};
use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Event, Symbol};

/// 1 000 000 MXN (7 décimales).
const NOTIONAL: i128 = 10_000_000_000_000;
/// Spot de départ : 0.055 USD/MXN (14 décimales).
const SPOT_0: i128 = 5_500_000_000_000;
/// Taux bloqué : 0.05487 USD/MXN → coût figé 54 870 USDC.
const LOCKED: i128 = 5_487_000_000_000;
/// Marge du plafond 10 % : 5 487 USDC (7 décimales).
const MARGIN: i128 = 54_870_000_000;

/// Environnement de test commun : USDC (SAC), mock-oracle (MXN à 0.055), pool, et les acteurs.
struct Setup<'a> {
    env: Env,
    lucia: Address,
    supplier: Address,
    lp: Address,
    usdc: TokenClient<'a>,
    oracle: MockOracleClient<'a>,
    pool: PoolClient<'a>,
}

fn setup<'a>() -> Setup<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let lucia = Address::generate(&env);
    let supplier = Address::generate(&env);
    let lp = Address::generate(&env);

    let usdc_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let usdc_admin = StellarAssetClient::new(&env, &usdc_id);
    usdc_admin.mint(&lp, &10_000_000_000_000); // 1 000 000 USDC
    usdc_admin.mint(&lucia, &2_000_000_000_000); // 200 000 USDC

    let oracle_id = env.register(MockOracle, (&admin,));
    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&OracleAsset::Other(Symbol::new(&env, "MXN")), &SPOT_0);

    let pool_id = env.register(Pool, (&admin, &usdc_id, &oracle_id, 50u32));
    let pool = PoolClient::new(&env, &pool_id);

    Setup {
        usdc: TokenClient::new(&env, &usdc_id),
        env,
        lucia,
        supplier,
        lp,
        oracle,
        pool,
    }
}

/// Fixe le cours MXN du mock-oracle (horodaté au ledger courant).
fn set_spot(s: &Setup, price: i128) {
    s.oracle.set_price(&OracleAsset::Other(Symbol::new(&s.env, "MXN")), &price);
}

#[test]
fn fixture_is_ready() {
    let s = setup();
    let mxn = OracleAsset::Other(Symbol::new(&s.env, "MXN"));
    assert_eq!(s.oracle.lastprice(&mxn).unwrap().price, SPOT_0);
    assert_eq!(s.usdc.balance(&s.lucia), 2_000_000_000_000);
    assert_eq!(s.usdc.balance(&s.lp), 10_000_000_000_000);
    assert_eq!(s.usdc.balance(&s.supplier), 0);
    assert_eq!(s.pool.capacity(), 0);

    set_spot(&s, 6_160_000_000_000);
    assert_eq!(s.oracle.lastprice(&mxn).unwrap().price, 6_160_000_000_000);
}

#[test]
fn pnl_currency_up_user_gains() {
    // +8 % : 0.0594 → 59 400 USDC au comptant, le pool doit 4 530 USDC
    assert_eq!(hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, 5_940_000_000_000, MARGIN), 45_300_000_000);
}

#[test]
fn pnl_currency_down_user_loses() {
    // −8 % : 0.0506 → 50 600 USDC au comptant, Lucía doit 4 270 USDC sur sa marge
    assert_eq!(hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, 5_060_000_000_000, MARGIN), -42_700_000_000);
}

#[test]
fn pnl_is_capped_at_margin() {
    // +15 % : écart de 8 380 USDC, borné à la marge de 5 487 USDC
    assert_eq!(hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, 6_325_000_000_000, MARGIN), MARGIN);
    // −15 % : borné à −marge
    assert_eq!(hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, 4_675_000_000_000, MARGIN), -MARGIN);
}

#[test]
fn pnl_sell_side_is_reversed() {
    // L'exportateur gagne quand la devise baisse
    assert_eq!(hedge::pnl(Side::SellCurrency, NOTIONAL, LOCKED, 5_060_000_000_000, MARGIN), 42_700_000_000);
    assert_eq!(hedge::pnl(Side::SellCurrency, NOTIONAL, LOCKED, 5_940_000_000_000, MARGIN), -45_300_000_000);
}

#[test]
fn pnl_rounds_in_favour_of_pool() {
    // 1 unité de devise (1e-7), écart de 1e-14 USD : P&L réel = 1e-21 USDC
    // le pool ne paie rien quand il doit une fraction...
    assert_eq!(hedge::pnl(Side::BuyCurrency, 1, LOCKED, LOCKED + 1, MARGIN), 0);
    // ...et encaisse l'unité entière quand c'est l'utilisateur qui doit une fraction
    assert_eq!(hedge::pnl(Side::BuyCurrency, 1, LOCKED, LOCKED - 1, MARGIN), -1);
}

#[test]
fn pnl_unchanged_spot_is_zero() {
    assert_eq!(hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, LOCKED, MARGIN), 0);
}

/// Capital des LPs dans le pool : 100 000 USDC.
const LP_CAPITAL: i128 = 1_000_000_000_000;
/// Solde USDC de Lucía avant sa marge : 200 000 USDC.
const LUCIA_START: i128 = 2_000_000_000_000;
/// Échéance de la couverture, en secondes après l'ouverture.
const MATURITY_IN: u64 = 60;

/// Le LP dépose son capital dans le pool (vrai `deposit_lp` du Vault).
fn fund_pool(s: &Setup) {
    s.pool.deposit_lp(&s.lp, &LP_CAPITAL);
}

/// Pool financé puis couverture de la démo ouverte par Lucía dans le sens `side`. Retourne son id.
fn open_funded(s: &Setup, side: Side) -> u64 {
    fund_pool(s);
    s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &side, &NOTIONAL, &MATURITY_IN, &1000)
}

/// Code d'erreur de contrat tel que le reçoit le client (`try_*` → `Err(Ok(..))`).
fn contract_err(code: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(code)
}

/// Avance le ledger à l'échéance et fixe le cours MXN du jour.
fn at_maturity(s: &Setup, price: i128) {
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + MATURITY_IN);
    set_spot(s, price);
}

/// (capital des LPs, capital réservé) du pool, lus par les points d'entrée publics du Vault.
fn pool_books(s: &Setup) -> (i128, i128) {
    let total = s.pool.total_assets();
    (total, total - s.pool.capacity())
}

/// Vérifie l'état après `settle` : versement à Lucía, P&L du pool, capital libéré, couverture close.
fn assert_settled(s: &Setup, id: u64, paid: i128, payout: i128, pool_pnl: i128) {
    assert_eq!(paid, payout);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - MARGIN + payout);
    assert_eq!(pool_books(s), (LP_CAPITAL + pool_pnl, 0));
    // Plus aucune marge en dépôt : tout l'USDC du contrat appartient aux LPs
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + pool_pnl);
    assert!(s.pool.get_hedge(&id).unwrap().settled);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_currency_up_pool_pays() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000); // +8 %

    // marge 5 487 + P&L 4 530 = 10 017 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 100_170_000_000, -45_300_000_000);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_currency_down_user_pays() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_060_000_000_000); // −8 %

    // marge 5 487 − perte 4 270 = 1 217 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 12_170_000_000, 42_700_000_000);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_beyond_cap_is_capped() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 6_325_000_000_000); // +15 %

    // écart 8 380 borné à la marge : 2 × 5 487 = 10 974 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 2 * MARGIN, -MARGIN);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_total_loss_pays_nothing() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 4_675_000_000_000); // −15 %

    // perte bornée à la marge : rien à rendre, toute la marge va au pool
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 0, MARGIN);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_sell_side() {
    let s = setup();
    let id = open_funded(&s, Side::SellCurrency);
    at_maturity(&s, 5_060_000_000_000); // −8 %, l'exportateur est protégé

    // marge 5 487 + P&L 4 270 = 9 757 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 97_570_000_000, -42_700_000_000);
}

#[test]
#[ignore = "attend Pricing"]
fn settle_guards() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);

    // avant l'échéance
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + MATURITY_IN - 1);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(HedgeError::NotMatured as u32))));

    // id inconnu
    assert_eq!(s.pool.try_settle(&42), Err(Ok(contract_err(HedgeError::UnknownHedge as u32))));

    // à l'échéance pile : OK, puis refusé une deuxième fois
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 1);
    set_spot(&s, SPOT_0);
    s.pool.settle(&id);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - MARGIN + MARGIN + hedge::pnl(Side::BuyCurrency, NOTIONAL, LOCKED, SPOT_0, MARGIN));
}

/// Lucía ouvre la couverture de la démo : 1 000 000 MXN, plafond 10 %, échéance `MATURITY_IN`.
fn open_demo(s: &Setup) -> u64 {
    s.pool.open_hedge(
        &s.lucia,
        &s.supplier,
        &Symbol::new(&s.env, "MXN"),
        &Side::BuyCurrency,
        &NOTIONAL,
        &MATURITY_IN,
        &1000,
    )
}

#[test]
#[ignore = "attend Pricing"]
fn open_locks_rate_and_takes_margin() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();

    let id = open_demo(&s);

    assert_eq!(id, 0);
    // Lucía a signé l'ouverture
    assert_eq!(s.env.auths()[0].0, s.lucia);
    // marge de 5 487 USDC passée de Lucía au contrat, et réservée dans le pool
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - MARGIN);
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + MARGIN);
    assert_eq!(pool_books(&s), (LP_CAPITAL, MARGIN));
    assert_eq!(s.pool.capacity(), LP_CAPITAL - MARGIN);

    let h = s.pool.get_hedge(&id).unwrap();
    assert_eq!(
        h,
        Hedge {
            owner: s.lucia.clone(),
            supplier: s.supplier.clone(),
            currency: Symbol::new(&s.env, "MXN"),
            side: Side::BuyCurrency,
            notional: NOTIONAL,
            locked_rate: LOCKED,
            cap_bps: 1000,
            margin: MARGIN,
            maturity: now + MATURITY_IN,
            settled: false,
        }
    );
}

#[test]
#[ignore = "attend Pricing"]
fn open_ids_increment() {
    let s = setup();
    fund_pool(&s);
    assert_eq!(open_demo(&s), 0);
    assert_eq!(open_demo(&s), 1);
    assert_eq!(pool_books(&s), (LP_CAPITAL, 2 * MARGIN));
}

#[test]
#[ignore = "attend Pricing"]
fn open_margin_follows_cap() {
    let s = setup();
    fund_pool(&s);
    // plafond 20 % : marge 10 974 USDC
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &2000);
    assert_eq!(s.pool.get_hedge(&id).unwrap().margin, 2 * MARGIN);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - 2 * MARGIN);
}

#[test]
#[ignore = "attend Pricing"]
fn open_refusals() {
    let s = setup();
    let mxn = Symbol::new(&s.env, "MXN");
    let buy = Side::BuyCurrency;

    // pool vide : capacité insuffisante (erreur du Vault)
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &MATURITY_IN, &1000),
        Err(Ok(contract_err(VaultError::InsufficientCapacity as u32)))
    );

    fund_pool(&s);
    // plafond sous le minimum de la devise (erreur de Pricing, code à venir)
    assert!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &MATURITY_IN, &400).is_err());
    // notionnel nul ou négatif
    let invalid_notional = Err(Ok(contract_err(HedgeError::InvalidNotional as u32)));
    assert_eq!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &0, &MATURITY_IN, &1000), invalid_notional);
    assert_eq!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &-NOTIONAL, &MATURITY_IN, &1000), invalid_notional);
    // durée nulle
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &0, &1000),
        Err(Ok(contract_err(HedgeError::InvalidDuration as u32)))
    );

    // rien n'a bougé
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START);
    assert_eq!(pool_books(&s), (LP_CAPITAL, 0));

    // et une demande valide passe : les refus venaient bien des contrôles
    assert_eq!(open_demo(&s), 0);
}

#[test]
#[ignore = "attend Pricing"]
fn open_then_settle_full_flow() {
    let s = setup();
    fund_pool(&s);
    let id = open_demo(&s);

    at_maturity(&s, 5_940_000_000_000); // +8 %
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 100_170_000_000, -45_300_000_000);
}

#[test]
#[ignore = "attend Pricing"]
fn open_emits_event() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();
    let id = open_demo(&s);

    let expected = hedge::HedgeOpened {
        hedge_id: id,
        owner: s.lucia.clone(),
        supplier: s.supplier.clone(),
        currency: Symbol::new(&s.env, "MXN"),
        side: Side::BuyCurrency,
        notional: NOTIONAL,
        locked_rate: LOCKED,
        cap_bps: 1000,
        margin: MARGIN,
        maturity: now + MATURITY_IN,
    };
    assert_eq!(
        s.env.events().all().filter_by_contract(&s.pool.address),
        [expected.to_xdr(&s.env, &s.pool.address)]
    );
}

#[test]
#[ignore = "attend Pricing"]
fn settle_emits_event() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000); // +8 %
    s.pool.settle(&id);

    let expected = hedge::HedgeSettled {
        hedge_id: id,
        owner: s.lucia.clone(),
        spot: 5_940_000_000_000,
        pnl: 45_300_000_000,
        payout: 100_170_000_000,
    };
    assert_eq!(
        s.env.events().all().filter_by_contract(&s.pool.address),
        [expected.to_xdr(&s.env, &s.pool.address)]
    );
}

#[test]
#[ignore = "attend Pricing"]
fn open_and_settle_track_exposure() {
    let s = setup();
    let mxn = Symbol::new(&s.env, "MXN");
    let id = open_funded(&s, Side::BuyCurrency);

    // ouverture : notionnel 54 870 USDC au taux bloqué et marge côté acheteur
    let e = s.pool.exposure(&mxn);
    assert_eq!((e.buy_notional, e.buy_margin, e.sell_notional, e.sell_margin), (548_700_000_000, MARGIN, 0, 0));
    assert_eq!(e.net_notional, 548_700_000_000);

    // règlement : l'exposition retombe à zéro
    at_maturity(&s, SPOT_0);
    s.pool.settle(&id);
    let e = s.pool.exposure(&mxn);
    assert_eq!((e.buy_notional, e.buy_margin, e.net_notional), (0, 0, 0));
}

#[test]
#[ignore = "attend Pricing"]
fn open_refused_over_currency_limit() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");
    // limite MXN de l'admin : 5 % du capital = 5 000 USDC de marges, sous la marge de 5 487
    s.pool.set_currency_limit(&mxn, &500);
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000),
        Err(Ok(contract_err(VaultError::CurrencyLimitExceeded as u32)))
    );
}
