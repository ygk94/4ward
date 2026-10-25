#![cfg(test)]
//! Tests de départ, un bloc par personne : retirer les #[ignore] au fur et à mesure.
//! Scénario commun : 1 000 000 MXN, spot 0.055 USD/MXN, plafond 10 %, oracle = mock-oracle.

// ---- Vault ----
// Tests dans vault.rs (mod tests) : parts LP, dépôts / retraits, reserve / release / book_*.

// ---- Pricing ----

mod pricing_tests {
    //! Oracle de test minimal, même interface de lecture SEP-40 que Reflector FX et que `contracts/mock-oracle`
    //! (`lastprice(Asset::Other(sym))`, `decimals()` = 14). Réutilise les types `Asset` / `PriceData` du client
    //! déclaré dans `pricing.rs`, ce qui vérifie au passage que l'encodage coïncide.
    use crate::pricing::{self, Asset, PriceData, PricingError, MAX_CAP_BPS, MAX_PRICE_AGE_SECS, SECS_PER_YEAR};
    use crate::{Pool, PoolClient, Quote, Side, BPS, PRICE_DECIMALS};
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

    #[contracttype]
    enum OracleKey {
        Price(Asset),
    }

    #[contract]
    pub struct TestOracle;

    #[contractimpl]
    impl TestOracle {
        pub fn set_price(env: Env, asset: Asset, price: i128) {
            let data = PriceData { price, timestamp: env.ledger().timestamp() };
            env.storage().persistent().set(&OracleKey::Price(asset), &data);
        }
        pub fn lastprice(env: Env, asset: Asset) -> Option<PriceData> {
            env.storage().persistent().get(&OracleKey::Price(asset))
        }
        pub fn decimals(_env: Env) -> u32 {
            PRICE_DECIMALS
        }
    }

    /// Spot de la démo : 1 MXN = 0.055 USD, 14 décimales.
    pub const MXN_SPOT: i128 = 5_500_000_000_000;
    /// Horodatage de départ (ledger à 0 par défaut : on part d'une date réaliste pour pouvoir reculer aussi).
    pub const T0: u64 = 1_800_000_000;

    pub struct Setup {
        pub env: Env,
        pub pool: Address,
        pub oracle: Address,
    }

    /// Env + oracle de test + pool branché dessus (spread 50 bps). Aucun prix ni taux posé : chaque test
    /// pose ce dont il a besoin.
    pub fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(T0);
        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let oracle = env.register(TestOracle, ());
        let pool = env.register(Pool, (&admin, &usdc, &oracle, 50u32));
        Setup { env, pool, oracle }
    }

    /// Pose un prix sur l'oracle de test, horodaté au ledger courant.
    pub fn set_price(s: &Setup, currency: &Symbol, price: i128) {
        TestOracleClient::new(&s.env, &s.oracle).set_price(&Asset::Other(currency.clone()), &price);
    }

    /// Code d'erreur de contrat tel que le reçoit le client (`try_*` → `Err(Ok(..))`).
    fn err(e: PricingError) -> soroban_sdk::Error {
        soroban_sdk::Error::from_contract_error(e as u32)
    }

    pub fn mxn(env: &Env) -> Symbol {
        Symbol::new(env, "MXN")
    }

    /// `spot` est `pub(crate)` et n'a pas encore de point d'entrée public qui le traverse (`quote`
    /// arrive en 1.3) : on l'exécute dans le contexte du contrat pool.
    fn spot(s: &Setup, currency: &Symbol) -> i128 {
        s.env.as_contract(&s.pool, || pricing::spot(&s.env, currency))
    }

    /// Pose les taux USD et devise via les points d'entrée publics (auth mockée par `setup`).
    fn set_rates(s: &Setup, usd_bps: u32, dev_currency: &Symbol, dev_bps: u32) {
        let pool = PoolClient::new(&s.env, &s.pool);
        pool.set_rate(&Symbol::new(&s.env, "USD"), &usd_bps);
        pool.set_rate(dev_currency, &dev_bps);
    }

    fn set_spread(s: &Setup, spread_bps: u32) {
        PoolClient::new(&s.env, &s.pool).set_spread(&spread_bps);
    }

    /// Même principe que `spot` : `forward_rate` n'a pas encore de point d'entrée public.
    fn forward(s: &Setup, currency: &Symbol, side: Side, duration: u64) -> i128 {
        s.env.as_contract(&s.pool, || pricing::forward_rate(&s.env, currency, side, duration))
    }

    /// 90 jours.
    const D90: u64 = 7_776_000;
    /// F = S × 318_470_400_000 / 320_803_200_000 (r_USD 400 bps, r_MXN 700 bps, 90 j), floor.
    /// Valeurs calculées indépendamment (Python, entiers exacts).
    const MXN_F90: i128 = 5_460_005_386_479;
    const MXN_F90_BUY_50: i128 = 5_487_305_413_412;
    const MXN_F90_SELL_50: i128 = 5_432_705_359_546;

    /// Spot MXN, r_USD 4 %, r_MXN 7 %.
    fn setup_mxn() -> (Setup, Symbol) {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        set_rates(&s, 400, &mxn, 700);
        (s, mxn)
    }

    fn set_min_cap(s: &Setup, currency: &Symbol, cap_bps: u32) {
        PoolClient::new(&s.env, &s.pool).set_min_cap(currency, &cap_bps);
    }

    /// `setup_mxn` + spread 50 bps + MXN ouvert avec un plafond minimum de 5 %.
    fn setup_mxn_full() -> (Setup, Symbol) {
        let (s, mxn) = setup_mxn();
        set_spread(&s, 50);
        set_min_cap(&s, &mxn, 500);
        (s, mxn)
    }

    /// 1 000 000 MXN, 7 décimales.
    const NOTIONAL_1M: i128 = 10_000_000_000_000;

    #[test]
    fn pricing_spot_reads_oracle() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #300)")]
    fn pricing_spot_no_price_panics() {
        let s = setup();
        let brl = Symbol::new(&s.env, "BRL");
        spot(&s, &brl);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #301)")]
    fn pricing_spot_non_positive_price_panics() {
        let s = setup();
        let mxn = mxn(&s.env);
        // Le mock-oracle réel refuse un prix ≤ 0 ; l'oracle de test le laisse passer pour tester la garde côté pool.
        set_price(&s, &mxn, 0);
        spot(&s, &mxn);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #302)")]
    fn pricing_stale_price_panics() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        s.env.ledger().set_timestamp(T0 + MAX_PRICE_AGE_SECS + 1);
        spot(&s, &mxn);
    }

    #[test]
    fn pricing_price_within_max_age_is_accepted() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        // 600 s : deux périodes Reflector manquées, encore acceptable.
        s.env.ledger().set_timestamp(T0 + 600);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
        // Exactement à la limite : accepté.
        s.env.ledger().set_timestamp(T0 + MAX_PRICE_AGE_SECS);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
        // Oracle « en avance » sur le ledger : pas périmé.
        s.env.ledger().set_timestamp(T0 - 10);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
    }

    #[test]
    fn pricing_forward_parity() {
        let (s, mxn) = setup_mxn();
        set_spread(&s, 0);
        // Sans spread, les deux côtés coïncident.
        assert_eq!(forward(&s, &mxn, Side::BuyCurrency, D90), MXN_F90);
        assert_eq!(forward(&s, &mxn, Side::SellCurrency, D90), MXN_F90);
    }

    #[test]
    fn pricing_spread_direction() {
        let (s, mxn) = setup_mxn();
        set_spread(&s, 50);
        let buy = forward(&s, &mxn, Side::BuyCurrency, D90);
        let sell = forward(&s, &mxn, Side::SellCurrency, D90);
        assert_eq!(buy, MXN_F90_BUY_50); // ceil
        assert_eq!(sell, MXN_F90_SELL_50); // floor
        assert!(sell < MXN_F90 && MXN_F90 < buy);
    }

    #[test]
    fn pricing_forward_zero_duration() {
        let (s, mxn) = setup_mxn();
        set_spread(&s, 0);
        assert_eq!(forward(&s, &mxn, Side::BuyCurrency, 0), MXN_SPOT);
        assert_eq!(forward(&s, &mxn, Side::SellCurrency, 0), MXN_SPOT);
        set_spread(&s, 50);
        // ceil(S × 10050 / 10000) ; S × 10050 est divisible par 10000 ici.
        assert_eq!(forward(&s, &mxn, Side::BuyCurrency, 0), 5_527_500_000_000);
        assert_eq!(forward(&s, &mxn, Side::SellCurrency, 0), 5_472_500_000_000);
    }

    #[test]
    fn pricing_forward_lower_rate_currency() {
        let (s, mxn) = setup_mxn();
        set_spread(&s, 0);
        // MXN mieux rémunéré que l'USD : cote moins cher à terme.
        assert!(forward(&s, &mxn, Side::BuyCurrency, D90) < MXN_SPOT);
        // EUR moins rémunéré que l'USD : cote plus cher à terme.
        let eur = Symbol::new(&s.env, "EUR");
        let eur_spot: i128 = 110_000_000_000_000; // 1,1 USD/EUR
        set_price(&s, &eur, eur_spot);
        PoolClient::new(&s.env, &s.pool).set_rate(&eur, &200);
        let f = forward(&s, &eur, Side::BuyCurrency, D90);
        assert!(f > eur_spot);
        assert_eq!(f, 110_539_803_707_742);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #303)")]
    fn pricing_forward_rate_missing_panics() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        forward(&s, &mxn, Side::BuyCurrency, D90);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #304)")]
    fn pricing_forward_duration_too_long_panics() {
        let (s, mxn) = setup_mxn();
        forward(&s, &mxn, Side::BuyCurrency, SECS_PER_YEAR + 1);
    }

    #[test]
    fn pricing_set_rate_rejects_absurd_values() {
        let s = setup();
        let pool = PoolClient::new(&s.env, &s.pool);
        let mxn = mxn(&s.env);
        assert!(pool.try_set_rate(&mxn, &10_000).is_ok());
        assert_eq!(pool.try_set_rate(&mxn, &10_001), Err(Ok(err(PricingError::RateTooHigh))));
        assert!(pool.try_set_spread(&500).is_ok());
        assert_eq!(pool.try_set_spread(&501), Err(Ok(err(PricingError::SpreadTooHigh))));
    }

    #[test]
    fn pricing_set_rate_requires_admin() {
        // Pas de mock_all_auths : l'admin n'a rien signé.
        let env = Env::default();
        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let oracle = env.register(TestOracle, ());
        let pool = PoolClient::new(&env, &env.register(Pool, (&admin, &usdc, &oracle, 50u32)));
        assert!(pool.try_set_rate(&mxn(&env), &700).is_err());
        assert!(pool.try_set_spread(&50).is_err());
    }

    #[test]
    fn pricing_quote_amounts() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert_eq!(
            pool.quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Quote {
                spot: MXN_SPOT,
                forward_rate: MXN_F90_BUY_50,
                usdc_cost: 548_730_541_341, // 54 873,05 USDC
                margin: 54_873_054_135,     // ceil(54_873_054_134,1)
            }
        );
        assert_eq!(pool.quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &500).margin, 27_436_527_068);
        assert_eq!(
            pool.quote(&mxn, &Side::SellCurrency, &NOTIONAL_1M, &D90, &1000),
            Quote {
                spot: MXN_SPOT,
                forward_rate: MXN_F90_SELL_50,
                usdc_cost: 543_270_535_954,
                margin: 54_327_053_596,
            }
        );
    }

    #[test]
    fn pricing_quote_margin_rounds_up() {
        let (s, mxn) = setup_mxn_full();
        let q = PoolClient::new(&s.env, &s.pool).quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000);
        assert!(q.margin * BPS >= q.usdc_cost * 1000);
        // 548_730_541_341 × 1000 / 10_000 ne tombe pas juste : marge = floor + 1.
        assert_ne!(q.usdc_cost * 1000 % BPS, 0);
        assert_eq!(q.margin, q.usdc_cost * 1000 / BPS + 1);
    }

    #[test]
    fn pricing_min_cap() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &400),
            Err(Ok(err(PricingError::CapBelowMinimum)))
        );
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &500).is_ok());
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000).is_ok());
    }

    #[test]
    fn pricing_quote_unknown_currency_rejected() {
        let (s, _) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        let brl = Symbol::new(&s.env, "BRL");
        set_price(&s, &brl, 18_000_000_000_000);
        pool.set_rate(&brl, &1050);
        // Prix et taux présents, mais pas de plafond minimum : devise non ouverte.
        assert_eq!(
            pool.try_quote(&brl, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::CurrencyNotEnabled)))
        );
    }

    #[test]
    fn pricing_quote_cap_too_high_rejected() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &MAX_CAP_BPS).is_ok());
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &(MAX_CAP_BPS + 1)),
            Err(Ok(err(PricingError::CapTooHigh)))
        );
    }

    #[test]
    fn pricing_quote_invalid_notional_rejected() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        let invalid = Err(Ok(err(PricingError::InvalidNotional)));
        assert_eq!(pool.try_quote(&mxn, &Side::BuyCurrency, &0, &D90, &1000), invalid);
        assert_eq!(pool.try_quote(&mxn, &Side::BuyCurrency, &-1, &D90, &1000), invalid);
    }

    #[test]
    fn pricing_set_min_cap_bounds() {
        let s = setup();
        let pool = PoolClient::new(&s.env, &s.pool);
        let cur = mxn(&s.env);
        let out_of_range = Err(Ok(err(PricingError::CapOutOfRange)));
        assert_eq!(pool.try_set_min_cap(&cur, &0), out_of_range);
        assert!(pool.try_set_min_cap(&cur, &MAX_CAP_BPS).is_ok());
        assert_eq!(pool.try_set_min_cap(&cur, &(MAX_CAP_BPS + 1)), out_of_range);

        // Sans mock_all_auths : l'admin n'a rien signé.
        let env = Env::default();
        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let oracle = env.register(TestOracle, ());
        let pool = PoolClient::new(&env, &env.register(Pool, (&admin, &usdc, &oracle, 50u32)));
        assert!(pool.try_set_min_cap(&mxn(&env), &500).is_err());
    }

    /// Les erreurs de l'oracle et des taux remontent telles quelles par `quote`, avec leur code.
    #[test]
    fn pricing_quote_propagates_oracle_and_rate_errors() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        s.env.ledger().set_timestamp(T0 + MAX_PRICE_AGE_SECS + 1);
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::StalePrice)))
        );
        set_price(&s, &mxn, MXN_SPOT);
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &(SECS_PER_YEAR + 1), &1000),
            Err(Ok(err(PricingError::DurationTooLong)))
        );
        // EUR ouvert et coté, mais sans taux : refus explicite, jamais de taux par défaut.
        let eur = Symbol::new(&s.env, "EUR");
        set_price(&s, &eur, 111_930_000_000_000);
        set_min_cap(&s, &eur, 300);
        assert_eq!(
            pool.try_quote(&eur, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::RateNotSet)))
        );
        // Ouvert mais pas coté.
        let brl = Symbol::new(&s.env, "BRL");
        set_min_cap(&s, &brl, 700);
        assert_eq!(
            pool.try_quote(&brl, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::NoPrice)))
        );
    }

    /// MXN ouvert mais sans prix sur l'oracle : le refus doit venir de `check_cap`, pas de `spot`.
    #[test]
    #[should_panic(expected = "Error(Contract, #307)")]
    fn pricing_quote_rejects_before_oracle_call() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_min_cap(&s, &mxn, 500);
        s.env.as_contract(&s.pool, || pricing::quote(&s.env, &mxn, Side::BuyCurrency, NOTIONAL_1M, D90, 400));
    }
}

// ---- Paiement ----

use crate::hedge::{self, HedgeError};
use crate::vault::VaultError;
use crate::{Hedge, Pool, PoolClient, Side};
use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger, MockAuth, MockAuthInvoke};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Event, IntoVal, Symbol};

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
    admin: Address,
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
    // paramètres Pricing du setup testnet : taux annuels et MXN ouverte (plafond minimum 5 %)
    pool.set_rate(&Symbol::new(&env, "USD"), &400);
    pool.set_rate(&Symbol::new(&env, "MXN"), &700);
    pool.set_min_cap(&Symbol::new(&env, "MXN"), &500);

    Setup {
        usdc: TokenClient::new(&env, &usdc_id),
        env,
        admin,
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

/// Taux bloqué réel de la couverture de démo (vrai Pricing, paramètres du setup testnet :
/// USD 4 %, MXN 7 %, spread 0,5 %, 60 s) : spot 0.055 → F = 0.05499999686, × 1,005 = 0.0552749968.
const HEDGE_LOCKED: i128 = 5_527_499_684_504;
/// Coût figé en USDC de 1 000 000 MXN à ce taux : 55 274,99 USDC.
const HEDGE_COST: i128 = 552_749_968_450;
/// Marge du plafond 10 % (`margin_for`, arrondie vers le haut) : 5 527,50 USDC.
const HEDGE_MARGIN: i128 = 55_274_996_845;

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
    let margin = s.pool.get_hedge(&id).unwrap().margin;
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - margin + payout);
    assert_eq!(pool_books(s), (LP_CAPITAL + pool_pnl, 0));
    // Plus aucune marge en dépôt : tout l'USDC du contrat appartient aux LPs
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + pool_pnl);
    assert!(s.pool.get_hedge(&id).unwrap().settled);
}

#[test]
fn settle_currency_up_pool_pays() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000); // +8 %

    // marge 5 527,50 + P&L 4 125,00 = 9 652,50 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 96_525_028_394, -41_250_031_549);
}

#[test]
fn settle_currency_down_user_pays() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_060_000_000_000); // −8 %

    // marge 5 527,50 − perte 4 675,00 = 852,50 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 8_525_028_394, 46_749_968_451);
}

#[test]
fn settle_beyond_cap_is_capped() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 6_325_000_000_000); // +15 %

    // écart 7 975 borné à la marge : 2 × 5 527,50 = 11 055 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 2 * HEDGE_MARGIN, -HEDGE_MARGIN);
}

#[test]
fn settle_total_loss_pays_nothing() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 4_675_000_000_000); // −15 %

    // perte bornée à la marge : rien à rendre, toute la marge va au pool
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 0, HEDGE_MARGIN);
}

#[test]
fn settle_sell_side() {
    let s = setup();
    let id = open_funded(&s, Side::SellCurrency);
    at_maturity(&s, 5_060_000_000_000); // −8 %, l'exportateur est protégé

    // taux vendeur 0.0547250 (spread vers le bas) : marge 5 472,50 + P&L 4 125,00 = 9 597,50 USDC
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 95_974_965_641, -41_249_968_764);
}

#[test]
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
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START + hedge::pnl(Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, SPOT_0, HEDGE_MARGIN));
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
fn open_locks_rate_and_takes_margin() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();

    let id = open_demo(&s);

    assert_eq!(id, 0);
    // Lucía a signé l'ouverture
    assert_eq!(s.env.auths()[0].0, s.lucia);
    // marge de 5 527,50 USDC passée de Lucía au contrat, et réservée dans le pool
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - HEDGE_MARGIN);
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + HEDGE_MARGIN);
    assert_eq!(pool_books(&s), (LP_CAPITAL, HEDGE_MARGIN));
    assert_eq!(s.pool.capacity(), LP_CAPITAL - HEDGE_MARGIN);

    let h = s.pool.get_hedge(&id).unwrap();
    assert_eq!(
        h,
        Hedge {
            owner: s.lucia.clone(),
            supplier: s.supplier.clone(),
            currency: Symbol::new(&s.env, "MXN"),
            side: Side::BuyCurrency,
            notional: NOTIONAL,
            locked_rate: HEDGE_LOCKED,
            cap_bps: 1000,
            margin: HEDGE_MARGIN,
            maturity: now + MATURITY_IN,
            settled: false,
        }
    );
}

#[test]
fn open_ids_increment() {
    let s = setup();
    fund_pool(&s);
    assert_eq!(open_demo(&s), 0);
    assert_eq!(open_demo(&s), 1);
    assert_eq!(pool_books(&s), (LP_CAPITAL, 2 * HEDGE_MARGIN));
}

#[test]
fn open_margin_follows_cap() {
    let s = setup();
    fund_pool(&s);
    // plafond 20 % : marge 11 055 USDC
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &2000);
    assert_eq!(s.pool.get_hedge(&id).unwrap().margin, 2 * HEDGE_MARGIN);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - 2 * HEDGE_MARGIN);
}

#[test]
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
fn open_then_settle_full_flow() {
    let s = setup();
    fund_pool(&s);
    let id = open_demo(&s);

    at_maturity(&s, 5_940_000_000_000); // +8 %
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 96_525_028_394, -41_250_031_549);
}

#[test]
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
        locked_rate: HEDGE_LOCKED,
        cap_bps: 1000,
        margin: HEDGE_MARGIN,
        maturity: now + MATURITY_IN,
    };
    // le vault émet aussi les siens (position_reserved / position_released) : l'événement
    // Paiement est le dernier émis par le pool dans la transaction
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(events.events().last(), Some(&expected.to_xdr(&s.env, &s.pool.address)));
}

#[test]
fn settle_emits_event() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000); // +8 %
    s.pool.settle(&id);

    let expected = hedge::HedgeSettled {
        hedge_id: id,
        owner: s.lucia.clone(),
        spot: 5_940_000_000_000,
        pnl: 41_250_031_549,
        payout: 96_525_028_394,
    };
    // le vault émet aussi les siens (position_reserved / position_released) : l'événement
    // Paiement est le dernier émis par le pool dans la transaction
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(events.events().last(), Some(&expected.to_xdr(&s.env, &s.pool.address)));
}

#[test]
fn open_and_settle_track_exposure() {
    let s = setup();
    let mxn = Symbol::new(&s.env, "MXN");
    let id = open_funded(&s, Side::BuyCurrency);

    // ouverture : notionnel 55 274,99 USDC au taux bloqué et marge côté acheteur
    let e = s.pool.exposure(&mxn);
    assert_eq!((e.buy_notional, e.buy_margin, e.sell_notional, e.sell_margin), (HEDGE_COST, HEDGE_MARGIN, 0, 0));
    assert_eq!(e.net_notional, HEDGE_COST);

    // règlement : l'exposition retombe à zéro
    at_maturity(&s, SPOT_0);
    s.pool.settle(&id);
    let e = s.pool.exposure(&mxn);
    assert_eq!((e.buy_notional, e.buy_margin, e.net_notional), (0, 0, 0));
}

#[test]
fn open_refused_over_currency_limit() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");
    // limite MXN de l'admin : 5 % du capital = 5 000 USDC de marges, sous la marge de 5 527,50
    s.pool.set_currency_limit(&mxn, &500);
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000),
        Err(Ok(contract_err(VaultError::CurrencyLimitExceeded as u32)))
    );
}

/// 90 jours, en secondes : l'échéance racontée dans la démo.
const DAYS_90: u64 = 7_776_000;
/// Facteur de démo : 90 jours annoncés → 300 s réelles.
const DEMO_FACTOR: u32 = 25_920;

#[test]
fn time_factor_defaults_to_one() {
    let s = setup();
    assert_eq!(s.pool.time_factor(), 1);
}

#[test]
fn accelerated_hedge_prices_on_announced_duration() {
    let s = setup();
    fund_pool(&s);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();

    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000);

    // prix et marge à 90 jours (54 873,05 et 5 487,31 USDC), échéance réelle 300 s plus tard
    let h = s.pool.get_hedge(&id).unwrap();
    assert_eq!(h.locked_rate, 5_487_305_413_412);
    assert_eq!(h.margin, 54_873_054_135);
    assert_eq!(h.maturity, now + 300);
}

#[test]
fn accelerated_hedge_settles_after_real_maturity() {
    let s = setup();
    fund_pool(&s);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000);

    s.env.ledger().set_timestamp(now + 299);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(HedgeError::NotMatured as u32))));

    // +12 % : protection plafonnée, 2 × 5 487,31 = 10 974,61 USDC versés
    s.env.ledger().set_timestamp(now + 300);
    set_spot(&s, 6_160_000_000_000);
    assert_eq!(s.pool.settle(&id), 109_746_108_270);
}

#[test]
fn set_time_factor_is_admin_only_and_public() {
    let s = setup();
    s.pool.set_time_factor(&DEMO_FACTOR);

    // signé par l'admin du pool
    assert_eq!(s.env.auths()[0].0, s.admin);
    // réglage visible sur la chaîne
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(
        events.events().last(),
        Some(&hedge::TimeFactorSet { factor: DEMO_FACTOR }.to_xdr(&s.env, &s.pool.address))
    );
    assert_eq!(s.pool.time_factor(), DEMO_FACTOR);

    // facteur nul refusé
    assert_eq!(s.pool.try_set_time_factor(&0), Err(Ok(contract_err(HedgeError::InvalidTimeFactor as u32))));
}

#[test]
fn accelerated_maturity_is_at_least_one_second() {
    let s = setup();
    fund_pool(&s);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();
    // 60 s / 25 920 = 0 : l'échéance réelle reste dans le futur
    let id = open_demo(&s);
    assert_eq!(s.pool.get_hedge(&id).unwrap().maturity, now + 1);
}

#[test]
fn open_refused_without_owner_signature() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");

    let open_args = (&s.lucia, &s.supplier, &mxn, Side::BuyCurrency, NOTIONAL, MATURITY_IN, 1000u32);
    // signature attendue : `open_hedge`, et dedans le transfert de la marge Lucía → pool
    let transfer = MockAuthInvoke {
        contract: &s.usdc.address,
        fn_name: "transfer",
        args: (&s.lucia, &s.pool.address, HEDGE_MARGIN).into_val(&s.env),
        sub_invokes: &[],
    };
    let open = MockAuthInvoke {
        contract: &s.pool.address,
        fn_name: "open_hedge",
        args: open_args.into_val(&s.env),
        sub_invokes: core::slice::from_ref(&transfer),
    };
    let signed_by = |who| MockAuth { address: who, invoke: &open };

    // le fournisseur signe à la place de Lucía : refusé
    let by_supplier = s
        .pool
        .mock_auths(&[signed_by(&s.supplier)])
        .try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000);
    assert!(by_supplier.is_err());

    // personne ne signe : refusé
    let unsigned = s
        .pool
        .set_auths(&[])
        .try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000);
    assert!(unsigned.is_err());

    // rien n'a bougé
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START);
    assert_eq!(pool_books(&s), (LP_CAPITAL, 0));

    // témoin : le même appel signé par Lucía passe, donc les refus venaient bien de la signature
    let id = s
        .pool
        .mock_auths(&[signed_by(&s.lucia)])
        .open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000);
    assert_eq!(s.pool.get_hedge(&id).unwrap().owner, s.lucia);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - HEDGE_MARGIN);
}

#[test]
fn settle_needs_no_signature() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, SPOT_0);

    // règlement sans aucune signature : n'importe qui peut le déclencher
    let paid = s.pool.set_auths(&[]).settle(&id);
    assert_eq!(paid, HEDGE_MARGIN + hedge::pnl(Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, SPOT_0, HEDGE_MARGIN));
    assert!(s.pool.get_hedge(&id).unwrap().settled);
}
