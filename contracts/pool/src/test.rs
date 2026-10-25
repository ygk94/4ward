#![cfg(test)]
//! Tests de départ, un bloc par personne : retirer les #[ignore] au fur et à mesure.
//! Scénario commun : 1 000 000 MXN, spot 0.055 USD/MXN, plafond 10 %, oracle = mock-oracle.

// ---- Vault ----

#[test]
#[ignore = "TODO vault: dépôt puis retrait, soldes et TotalDeposits à jour"]
fn vault_deposit_withdraw() {}

#[test]
#[ignore = "TODO vault: retrait refusé au-delà du capital non réservé"]
fn vault_withdraw_limited_to_capacity() {}

#[test]
#[ignore = "TODO vault: reserve refusé si capacité insuffisante"]
fn vault_reserve_over_capacity_panics() {}

// ---- Pricing ----

mod pricing_tests {
    //! Oracle de test minimal, même interface de lecture SEP-40 que Reflector FX et que `contracts/mock-oracle`
    //! (`lastprice(Asset::Other(sym))`, `decimals()` = 14). Réutilise les types `Asset` / `PriceData` du client
    //! déclaré dans `pricing.rs`, ce qui vérifie au passage que l'encodage coïncide.
    use crate::pricing::{self, Asset, PriceData, MAX_CAP_BPS, MAX_PRICE_AGE_SECS, SECS_PER_YEAR};
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
    #[should_panic(expected = "no price")]
    fn pricing_spot_no_price_panics() {
        let s = setup();
        let brl = Symbol::new(&s.env, "BRL");
        spot(&s, &brl);
    }

    #[test]
    #[should_panic(expected = "invalid price")]
    fn pricing_spot_non_positive_price_panics() {
        let s = setup();
        let mxn = mxn(&s.env);
        // Le mock-oracle réel refuse un prix ≤ 0 ; l'oracle de test le laisse passer pour tester la garde côté pool.
        set_price(&s, &mxn, 0);
        spot(&s, &mxn);
    }

    #[test]
    #[should_panic(expected = "stale price")]
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
    #[should_panic(expected = "rate not set")]
    fn pricing_forward_rate_missing_panics() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_price(&s, &mxn, MXN_SPOT);
        forward(&s, &mxn, Side::BuyCurrency, D90);
    }

    #[test]
    #[should_panic(expected = "duration too long")]
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
        assert!(pool.try_set_rate(&mxn, &10_001).is_err());
        assert!(pool.try_set_spread(&500).is_ok());
        assert!(pool.try_set_spread(&501).is_err());
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
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &400).is_err());
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
        assert!(pool.try_quote(&brl, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000).is_err());
    }

    #[test]
    fn pricing_quote_cap_too_high_rejected() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &MAX_CAP_BPS).is_ok());
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &(MAX_CAP_BPS + 1)).is_err());
    }

    #[test]
    fn pricing_quote_invalid_notional_rejected() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &0, &D90, &1000).is_err());
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &-1, &D90, &1000).is_err());
    }

    #[test]
    fn pricing_set_min_cap_bounds() {
        let s = setup();
        let pool = PoolClient::new(&s.env, &s.pool);
        let cur = mxn(&s.env);
        assert!(pool.try_set_min_cap(&cur, &0).is_err());
        assert!(pool.try_set_min_cap(&cur, &MAX_CAP_BPS).is_ok());
        assert!(pool.try_set_min_cap(&cur, &(MAX_CAP_BPS + 1)).is_err());

        // Sans mock_all_auths : l'admin n'a rien signé.
        let env = Env::default();
        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let oracle = env.register(TestOracle, ());
        let pool = PoolClient::new(&env, &env.register(Pool, (&admin, &usdc, &oracle, 50u32)));
        assert!(pool.try_set_min_cap(&mxn(&env), &500).is_err());
    }

    /// MXN ouvert mais sans prix sur l'oracle : le refus doit venir de `check_cap`, pas de `spot`.
    #[test]
    #[should_panic(expected = "cap below minimum")]
    fn pricing_quote_rejects_before_oracle_call() {
        let s = setup();
        let mxn = mxn(&s.env);
        set_min_cap(&s, &mxn, 500);
        s.env.as_contract(&s.pool, || pricing::quote(&s.env, &mxn, Side::BuyCurrency, NOTIONAL_1M, D90, 400));
    }
}

// ---- Paiement ----

#[test]
#[ignore = "TODO paiement: la devise monte de 8 %, le pool paie l'écart à l'importateur"]
fn settle_currency_up_pool_pays() {}

#[test]
#[ignore = "TODO paiement: la devise baisse de 8 %, l'importateur paie le pool sur sa marge"]
fn settle_currency_down_user_pays() {}

#[test]
#[ignore = "TODO paiement: la devise monte de 15 %, paiement borné au plafond de 10 %"]
fn settle_beyond_cap_is_capped() {}

#[test]
#[ignore = "TODO paiement: settle refusé avant l'échéance et refusé deux fois"]
fn settle_guards() {}
