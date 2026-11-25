// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
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
    use crate::pricing::{
        self, Asset, FixingRead, PriceData, PricingError, FIXING_GRACE_SECS, MAX_CAP_BPS, MAX_FIXING_AGE_SECS,
        MAX_PRICE_AGE_SECS, SECS_PER_YEAR,
    };
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
    /// Taux de test arbitraires (≠ snapshot 360/555 de la démo, vérifiés par `demo_figures`).
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
        // 330 s : âge normal d'un prix Reflector juste avant la publication suivante.
        s.env.ledger().set_timestamp(T0 + 330);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
        // Exactement à la limite (600 s, une période manquée) : accepté.
        assert_eq!(MAX_PRICE_AGE_SECS, 600);
        s.env.ledger().set_timestamp(T0 + 600);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
        // Oracle « en avance » sur le ledger : pas périmé.
        s.env.ledger().set_timestamp(T0 - 10);
        assert_eq!(spot(&s, &mxn), MXN_SPOT);
    }

    /// L'ancienne limite (900 s) n'est plus acceptée : à 601 s le devis est refusé.
    #[test]
    fn pricing_quote_refused_past_600_s() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        s.env.ledger().set_timestamp(T0 + 600);
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000).is_ok());
        s.env.ledger().set_timestamp(T0 + 601);
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::StalePrice)))
        );
    }

    // ---- dépassements de capacité : code 313, jamais de panique sans numéro ----

    /// Notionnel démesuré : `to_usdc` (notional × taux) déborde.
    #[test]
    fn pricing_quote_huge_notional_returns_overflow() {
        let (s, mxn) = setup_mxn_full();
        let pool = PoolClient::new(&s.env, &s.pool);
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &i128::MAX, &D90, &1000),
            Err(Ok(err(PricingError::Overflow)))
        );
        // Le plus grand notionnel qui passe encore à ce taux, puis le premier qui déborde.
        let rate = pool.quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000).forward_rate;
        let max_ok = i128::MAX / rate;
        assert!(pool.try_quote(&mxn, &Side::BuyCurrency, &max_ok, &D90, &1000).is_ok());
        assert_eq!(
            pool.try_quote(&mxn, &Side::BuyCurrency, &(max_ok + 1), &D90, &1000),
            Err(Ok(err(PricingError::Overflow)))
        );
    }

    /// Cours démesuré renvoyé par l'oracle (contrat externe) : le taux forward déborde.
    #[test]
    fn pricing_forward_huge_oracle_price_returns_overflow() {
        let (s, mxn) = setup_mxn_full();
        set_price(&s, &mxn, i128::MAX / 1_000);
        assert_eq!(
            PoolClient::new(&s.env, &s.pool).try_quote(&mxn, &Side::BuyCurrency, &NOTIONAL_1M, &D90, &1000),
            Err(Ok(err(PricingError::Overflow)))
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #313)")]
    fn pricing_margin_for_overflow_panics_with_code() {
        let s = setup();
        s.env.as_contract(&s.pool, || pricing::margin_for(&s.env, i128::MAX, 1000));
    }

    /// Arrondi vers le haut dont l'addition déborde alors que le produit tient encore.
    #[test]
    #[should_panic(expected = "Error(Contract, #313)")]
    fn pricing_margin_for_ceil_addition_overflow_panics_with_code() {
        let s = setup();
        s.env.as_contract(&s.pool, || pricing::margin_for(&s.env, i128::MAX, 1));
    }

    #[test]
    fn pricing_to_usdc_and_margin_unchanged_on_normal_values() {
        let s = setup();
        s.env.as_contract(&s.pool, || {
            // 1 000 000 MXN à 0,055 → 55 000 USDC ; marge 10 % → 5 500 USDC.
            assert_eq!(pricing::to_usdc(&s.env, NOTIONAL_1M, MXN_SPOT), 550_000_000_000);
            assert_eq!(pricing::margin_for(&s.env, 550_000_000_000, 1000), 55_000_000_000);
            // Arrondis : conversion vers le bas, marge vers le haut.
            assert_eq!(pricing::to_usdc(&s.env, 1, MXN_SPOT), 0);
            assert_eq!(pricing::margin_for(&s.env, 1, 1), 1);
            assert_eq!(pricing::margin_for(&s.env, 0, 1000), 0);
        });
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

    // ---- fixing à l'échéance, sur le vrai mock-oracle (historique des prix) ----

    /// Pool branché sur `mock_oracle::MockOracle` : vérifie aussi que l'appel `price` coïncide en XDR.
    /// Retourne (env, pool, oracle).
    fn setup_mock_oracle() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(T0);
        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let oracle = env.register(mock_oracle::MockOracle, (&admin,));
        let pool = env.register(Pool, (&admin, &usdc, &oracle, 50u32));
        (env, pool, oracle)
    }

    /// Avance le ledger à `ts` puis pose le prix MXN sur le mock (horodaté à `ts`).
    fn mock_price_at(env: &Env, oracle: &Address, ts: u64, price: i128) {
        env.ledger().set_timestamp(ts);
        let mxn = mock_oracle::Asset::Other(mxn(env));
        mock_oracle::MockOracleClient::new(env, oracle).set_price(&mxn, &price);
    }

    fn read(env: &Env, pool: &Address, maturity: u64) -> FixingRead {
        env.as_contract(pool, || pricing::read_fixing(env, &mxn(env), maturity))
    }

    fn fixing(env: &Env, pool: &Address, maturity: u64) -> i128 {
        match read(env, pool, maturity) {
            FixingRead::Found(p) => p,
            other => panic!("pas de cours : {:?}", other),
        }
    }

    /// Le crash posé avant l'échéance compte ; un prix posé après l'échéance est ignoré,
    /// quel que soit le moment où `settle` est appelé.
    #[test]
    fn pricing_fixing_uses_price_in_force_at_maturity() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + 300;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        mock_price_at(&env, &oracle, T0 + 120, 6_160_000_000_000); // crash +12 % avant l'échéance
        mock_price_at(&env, &oracle, T0 + 400, 7_000_000_000_000); // après l'échéance : ignoré
        env.ledger().set_timestamp(T0 + 1_000);
        assert_eq!(fixing(&env, &pool, maturity), 6_160_000_000_000);
        // Prix posé exactement à l'échéance : c'est lui qui compte.
        assert_eq!(fixing(&env, &pool, T0 + 400), 7_000_000_000_000);
    }

    #[test]
    fn pricing_fixing_price_at_max_age_is_accepted() {
        let (env, pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(T0 + 2_000);
        assert_eq!(fixing(&env, &pool, T0 + MAX_FIXING_AGE_SECS), MXN_SPOT);
    }

    /// Cours à l'échéance trop ancien : on prend le premier cours publié après l'échéance, pas l'ancien.
    #[test]
    fn pricing_fixing_stale_falls_back_to_first_price_after() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + MAX_FIXING_AGE_SECS + 1;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        mock_price_at(&env, &oracle, maturity + 700, 6_000_000_000_000);
        mock_price_at(&env, &oracle, maturity + 2_000, 7_000_000_000_000);
        env.ledger().set_timestamp(maturity + 2_500);
        assert_eq!(fixing(&env, &pool, maturity), 6_000_000_000_000);
    }

    /// Premier prix après l'échéance, sans aucun prix avant : il compte aussi.
    #[test]
    fn pricing_fixing_no_price_before_maturity_uses_price_after() {
        let (env, pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0 + 400, MXN_SPOT);
        env.ledger().set_timestamp(T0 + 600);
        assert_eq!(fixing(&env, &pool, T0 + 300), MXN_SPOT);
    }

    /// Cours trop ancien, rien de neuf, l'heure n'est pas écoulée : pas encore.
    #[test]
    fn pricing_fixing_pending_within_grace() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + 1_000;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(maturity + FIXING_GRACE_SECS - 1);
        assert_eq!(read(&env, &pool, maturity), FixingRead::Pending);
    }

    /// Échéance dans le futur : pas encore.
    #[test]
    fn pricing_fixing_future_maturity_is_pending() {
        let (env, pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        assert_eq!(read(&env, &pool, T0 + 60), FixingRead::Pending);
    }

    /// Aucun cours utilisable une heure après l'échéance : absent, même si un prix arrive ensuite.
    #[test]
    fn pricing_fixing_missing_after_grace() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + 1_000;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(maturity + FIXING_GRACE_SECS);
        assert_eq!(read(&env, &pool, maturity), FixingRead::Missing);
        assert!(env.as_contract(&pool, || pricing::fixing_missing(&env, 0, &mxn(&env), maturity)));
        // un prix publié après la fenêtre ne rattrape rien
        mock_price_at(&env, &oracle, maturity + FIXING_GRACE_SECS + 10, 7_000_000_000_000);
        assert_eq!(read(&env, &pool, maturity), FixingRead::Missing);
    }

    /// Figé par `record_fixing`, le cours ne change plus (nouveau prix, oubli de l'historique > 24 h,
    /// second appel), sert au règlement, puis l'entrée disparaît.
    #[test]
    fn pricing_fixing_recorded_survives_oracle_and_is_consumed() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + 300;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(maturity + 10);
        let record = |env: &Env| env.as_contract(&pool, || pricing::record_fixing(env, 7, &mxn(env), maturity));
        let locked = |env: &Env| env.as_contract(&pool, || pricing::locked_fixing(env, 7));
        assert_eq!(record(&env), MXN_SPOT);
        assert_eq!(locked(&env), Some(MXN_SPOT));
        mock_price_at(&env, &oracle, maturity + 100_000, 9_000_000_000_000);
        assert_eq!(read(&env, &pool, maturity), FixingRead::Missing); // l'oracle a oublié
        assert_eq!(record(&env), MXN_SPOT); // second appel : cours inchangé, pas de panique
        assert_eq!(locked(&env), Some(MXN_SPOT));
        assert!(!env.as_contract(&pool, || pricing::fixing_missing(&env, 7, &mxn(&env), maturity)));
        let spot = env.as_contract(&pool, || pricing::settlement_fixing(&env, 7, &mxn(&env), maturity));
        assert_eq!(spot, MXN_SPOT);
        assert_eq!(locked(&env), None);
    }

    /// Règlement direct sur l'oracle : rien n'est écrit.
    #[test]
    fn pricing_settlement_fixing_direct_read_writes_nothing() {
        let (env, pool, oracle) = setup_mock_oracle();
        let maturity = T0 + 300;
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(maturity + 10);
        let spot = env.as_contract(&pool, || pricing::settlement_fixing(&env, 7, &mxn(&env), maturity));
        assert_eq!(spot, MXN_SPOT);
        assert_eq!(env.as_contract(&pool, || pricing::locked_fixing(&env, 7)), None);
    }

    /// Pas encore de cours : `record_fixing` échoue avec 312.
    #[test]
    #[should_panic(expected = "Error(Contract, #312)")]
    fn pricing_record_fixing_pending_is_no_fixing() {
        let (env, pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(T0 + 2_000);
        env.as_contract(&pool, || pricing::record_fixing(&env, 3, &mxn(&env), T0 + 1_000));
    }

    /// Pas encore de cours : `settlement_fixing` échoue avec 312.
    #[test]
    #[should_panic(expected = "Error(Contract, #312)")]
    fn pricing_settlement_fixing_pending_is_no_fixing() {
        let (env, pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        env.ledger().set_timestamp(T0 + 2_000);
        env.as_contract(&pool, || pricing::settlement_fixing(&env, 3, &mxn(&env), T0 + 1_000));
    }

    /// Le mock, comme Reflector, n'a plus de prix pour une date de plus de 24 h.
    #[test]
    fn mock_oracle_forgets_after_24h() {
        let (env, _pool, oracle) = setup_mock_oracle();
        mock_price_at(&env, &oracle, T0, MXN_SPOT);
        let client = mock_oracle::MockOracleClient::new(&env, &oracle);
        let mxn = mock_oracle::Asset::Other(mxn(&env));
        env.ledger().set_timestamp(T0 + 86_400);
        assert!(client.price(&mxn, &T0).is_some());
        env.ledger().set_timestamp(T0 + 86_401);
        assert_eq!(client.price(&mxn, &T0), None);
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
use crate::pricing::PricingError;
use crate::vault::VaultError;
use crate::{Hedge, Pool, PoolClient, Role, Side};
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

    let usdc_sac = env.register_stellar_asset_contract_v2(admin.clone());
    // émetteur révocable, comme Circle : permet de simuler un compte qui ne peut plus recevoir d'USDC
    usdc_sac.issuer().set_flag(soroban_sdk::testutils::IssuerFlags::RevocableFlag);
    let usdc_id = usdc_sac.address();
    let usdc_admin = StellarAssetClient::new(&env, &usdc_id);
    usdc_admin.mint(&lp, &10_000_000_000_000); // 1 000 000 USDC
    usdc_admin.mint(&lucia, &2_000_000_000_000); // 200 000 USDC

    let oracle_id = env.register(MockOracle, (&admin,));
    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&OracleAsset::Other(Symbol::new(&env, "MXN")), &SPOT_0);

    let pool_id = env.register(Pool, (&admin, &usdc_id, &oracle_id, 50u32));
    let pool = PoolClient::new(&env, &pool_id);
    pool.set_allowed(&Role::Lp, &lp, &true); // liste blanche des LP active par défaut (B9)
    pool.set_allowed(&Role::Sme, &lucia, &true); // liste blanche des PME active par défaut (B9)
    // taux de test arbitraires (anciens taux 400/700, ≠ snapshot 360/555 : voir `demo_figures`)
    // et MXN ouverte (plafond minimum 5 %)
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
    let env = Env::default();
    // +8 % : 0.0594 → 59 400 USDC au comptant, le pool doit 4 530 USDC
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, NOTIONAL, LOCKED, 5_940_000_000_000, MARGIN), 45_300_000_000);
}

#[test]
fn pnl_currency_down_user_loses() {
    let env = Env::default();
    // −8 % : 0.0506 → 50 600 USDC au comptant, Lucía doit 4 270 USDC sur sa marge
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, NOTIONAL, LOCKED, 5_060_000_000_000, MARGIN), -42_700_000_000);
}

#[test]
fn pnl_is_capped_at_margin() {
    let env = Env::default();
    // +15 % : écart de 8 380 USDC, borné à la marge de 5 487 USDC
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, NOTIONAL, LOCKED, 6_325_000_000_000, MARGIN), MARGIN);
    // −15 % : borné à −marge
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, NOTIONAL, LOCKED, 4_675_000_000_000, MARGIN), -MARGIN);
}

#[test]
fn pnl_sell_side_is_reversed() {
    let env = Env::default();
    // L'exportateur gagne quand la devise baisse
    assert_eq!(hedge::pnl(&env, Side::SellCurrency, NOTIONAL, LOCKED, 5_060_000_000_000, MARGIN), 42_700_000_000);
    assert_eq!(hedge::pnl(&env, Side::SellCurrency, NOTIONAL, LOCKED, 5_940_000_000_000, MARGIN), -45_300_000_000);
}

#[test]
fn pnl_rounds_in_favour_of_pool() {
    let env = Env::default();
    // 1 unité de devise (1e-7), écart de 1e-14 USD : P&L réel = 1e-21 USDC
    // le pool ne paie rien quand il doit une fraction...
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, 1, LOCKED, LOCKED + 1, MARGIN), 0);
    // ...et encaisse l'unité entière quand c'est l'utilisateur qui doit une fraction
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, 1, LOCKED, LOCKED - 1, MARGIN), -1);
}

#[test]
#[should_panic(expected = "Error(Contract, #207)")]
fn pnl_overflow_returns_code_instead_of_trapping() {
    // écart démesuré : notional × écart dépasse i128, refus avec un code (207) et non un piège de la VM
    let env = Env::default();
    hedge::pnl(&env, Side::BuyCurrency, i128::MAX / 2, LOCKED, i128::MAX / 4, MARGIN);
}

#[test]
fn pnl_unchanged_spot_is_zero() {
    let env = Env::default();
    assert_eq!(hedge::pnl(&env, Side::BuyCurrency, NOTIONAL, LOCKED, LOCKED, MARGIN), 0);
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
    s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &side, &NOTIONAL, &MATURITY_IN, &1000, &None)
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

/// Vérifie l'état après `settle` ou `settle_fallback` : le versement est **crédité** au solde réclamable de Lucía
/// (rien ne lui est envoyé), P&L du pool, capital libéré, couverture close ; puis Lucía réclame et reçoit tout.
fn assert_settled(s: &Setup, id: u64, paid: i128, payout: i128, pool_pnl: i128) {
    assert_eq!(paid, payout);
    let margin = s.pool.get_hedge(&id).unwrap().margin;
    assert!(s.pool.get_hedge(&id).unwrap().settled);
    assert_eq!(pool_books(s), (LP_CAPITAL + pool_pnl, 0));
    // crédité, pas versé : l'USDC reste dans le contrat, au nom de Lucía
    assert_eq!(s.pool.claimable(&s.lucia), payout);
    assert_eq!(s.pool.total_claimable(), payout);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - margin);
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + pool_pnl + payout);

    // Lucía réclame : elle reçoit tout, et tout l'USDC du contrat appartient de nouveau aux LPs
    assert_eq!(s.pool.claim(&s.lucia), payout);
    assert_eq!(s.pool.claimable(&s.lucia), 0);
    assert_eq!(s.pool.total_claimable(), 0);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - margin + payout);
    assert_eq!(s.usdc.balance(&s.pool.address), LP_CAPITAL + pool_pnl);
}

/// Vérifie l'état après `close_early` : signée par Lucía, la clôture lui verse directement l'argent.
fn assert_closed_early(s: &Setup, id: u64, paid: i128, payout: i128, pool_pnl: i128) {
    assert_eq!(paid, payout);
    let margin = s.pool.get_hedge(&id).unwrap().margin;
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - margin + payout);
    assert_eq!(s.pool.claimable(&s.lucia), 0);
    assert_eq!(pool_books(s), (LP_CAPITAL + pool_pnl, 0));
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
    s.pool.claim(&s.lucia);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START + hedge::pnl(&s.env, Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, SPOT_0, HEDGE_MARGIN));
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
        &None,
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
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &2000, &None);
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
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &MATURITY_IN, &1000, &None),
        Err(Ok(contract_err(VaultError::InsufficientCapacity as u32)))
    );

    fund_pool(&s);
    // plafond sous le minimum de la devise (erreur de Pricing, code à venir)
    assert!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &MATURITY_IN, &400, &None).is_err());
    // notionnel nul ou négatif
    let invalid_notional = Err(Ok(contract_err(HedgeError::InvalidNotional as u32)));
    assert_eq!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &0, &MATURITY_IN, &1000, &None), invalid_notional);
    assert_eq!(s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &-NOTIONAL, &MATURITY_IN, &1000, &None), invalid_notional);
    // durée nulle
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &buy, &NOTIONAL, &0, &1000, &None),
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
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None),
        Err(Ok(contract_err(VaultError::CurrencyLimitExceeded as u32)))
    );
}

/// 90 jours, en secondes : l'échéance racontée dans la démo.
#[cfg(feature = "demo")]
const DAYS_90: u64 = 7_776_000;
/// Facteur de démo : 90 jours annoncés → 300 s réelles.
#[cfg(feature = "demo")]
const DEMO_FACTOR: u32 = 25_920;

#[test]
fn time_factor_defaults_to_one() {
    let s = setup();
    assert_eq!(s.pool.time_factor(), 1);
}

/// Version de production (sans la feature `demo`) : le mode démo n'existe pas. `set_time_factor` est refusé
/// (208) et les échéances restent réelles, quoi qu'il y ait dans le stockage.
#[cfg(not(feature = "demo"))]
#[test]
fn production_build_refuses_demo_mode() {
    let s = setup();
    fund_pool(&s);
    assert_eq!(s.pool.try_set_time_factor(&25_920), Err(Ok(contract_err(HedgeError::DemoDisabled as u32))));
    assert_eq!(s.pool.time_factor(), 1);
    let now = s.env.ledger().timestamp();
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    assert_eq!(s.pool.get_hedge(&id).unwrap().maturity, now + MATURITY_IN);
}

#[cfg(feature = "demo")]
#[test]
fn accelerated_hedge_prices_on_announced_duration() {
    let s = setup();
    fund_pool(&s);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();

    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000, &None);

    // prix et marge à 90 jours (54 873,05 et 5 487,31 USDC), échéance réelle 300 s plus tard
    let h = s.pool.get_hedge(&id).unwrap();
    assert_eq!(h.locked_rate, 5_487_305_413_412);
    assert_eq!(h.margin, 54_873_054_135);
    assert_eq!(h.maturity, now + 300);
}

#[cfg(feature = "demo")]
#[test]
fn accelerated_hedge_settles_after_real_maturity() {
    let s = setup();
    fund_pool(&s);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000, &None);

    s.env.ledger().set_timestamp(now + 299);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(HedgeError::NotMatured as u32))));

    // +12 % : protection plafonnée, 2 × 5 487,31 = 10 974,61 USDC versés
    s.env.ledger().set_timestamp(now + 300);
    set_spot(&s, 6_160_000_000_000);
    assert_eq!(s.pool.settle(&id), 109_746_108_270);
}

/// Chiffres exacts de la vidéo et du README : taux de `scripts/rates.snapshot` (USD 360, MXN 555),
/// spot 0,055, 1 000 000 MXN à 90 jours, plafond 10 %, mode démo, crash +12 % avant l'échéance.
/// Mêmes valeurs que `sim/contract.py` (auto-test) et `docs/points-a-trancher.md`.
#[cfg(feature = "demo")]
#[test]
fn demo_figures() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");
    s.pool.set_rate(&Symbol::new(&s.env, "USD"), &360);
    s.pool.set_rate(&mxn, &555);
    s.pool.set_time_factor(&DEMO_FACTOR);
    let now = s.env.ledger().timestamp();

    // prix figé 55 012,81 USDC, dépôt 5 501,28 USDC
    let q = s.pool.quote(&mxn, &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000);
    assert_eq!(q.forward_rate, 5_501_281_368_668);
    assert_eq!(q.usdc_cost, 550_128_136_866);
    assert_eq!(q.margin, 55_012_813_687);

    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &DAYS_90, &1000, &None);
    let h = s.pool.get_hedge(&id).unwrap();
    assert_eq!(h.locked_rate, q.forward_rate);
    assert_eq!(h.margin, q.margin);

    // +12 % (0,0616) : protection plafonnée, 11 002,56 USDC versés ; coût total 61 600 − 11 002,56 + 5 501,28 = 56 098,72
    s.env.ledger().set_timestamp(now + 300);
    set_spot(&s, 6_160_000_000_000);
    assert_eq!(s.pool.settle(&id), 110_025_627_374);
}

#[cfg(feature = "demo")]
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

#[cfg(feature = "demo")]
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

    let open_args = (&s.lucia, &s.supplier, &mxn, Side::BuyCurrency, NOTIONAL, MATURITY_IN, 1000u32, Option::<i128>::None);
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
        .try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    assert!(by_supplier.is_err());

    // personne ne signe : refusé
    let unsigned = s
        .pool
        .set_auths(&[])
        .try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    assert!(unsigned.is_err());

    // rien n'a bougé
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START);
    assert_eq!(pool_books(&s), (LP_CAPITAL, 0));

    // témoin : le même appel signé par Lucía passe, donc les refus venaient bien de la signature
    let id = s
        .pool
        .mock_auths(&[signed_by(&s.lucia)])
        .open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
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
    assert_eq!(paid, HEDGE_MARGIN + hedge::pnl(&s.env, Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, SPOT_0, HEDGE_MARGIN));
    assert!(s.pool.get_hedge(&id).unwrap().settled);
}

/// Avance le ledger à `ts` et y pose le cours MXN du mock (l'historique garde la date).
fn price_at(s: &Setup, ts: u64, price: i128) {
    s.env.ledger().set_timestamp(ts);
    set_spot(s, price);
}

#[test]
fn settle_uses_rate_at_maturity() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    // crash +12 % 10 s avant l'échéance : c'est lui qui compte
    price_at(&s, now + MATURITY_IN - 10, 6_160_000_000_000);
    // le peso retombe (−8 %) après l'échéance : ignoré, même si on règle maintenant
    price_at(&s, now + MATURITY_IN + 60, 5_060_000_000_000);

    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 2 * HEDGE_MARGIN, -HEDGE_MARGIN);
}

#[test]
fn settle_stale_at_maturity_uses_first_price_after() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();
    // échéance à 2 000 s, aucun prix posé entre l'ouverture et l'échéance
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &2_000, &1000, &None);

    // crash +12 % publié 10 s après l'échéance : pas encore lisible (recherche par pas de 300 s)
    price_at(&s, now + 2_010, 6_160_000_000_000);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(PricingError::NoFixing as u32))));
    assert!(!s.pool.get_hedge(&id).unwrap().settled);

    // 300 s après l'échéance, ce premier cours est retenu ; un prix plus tardif est ignoré
    price_at(&s, now + 2_400, SPOT_0);
    s.env.ledger().set_timestamp(now + 2_500);
    let margin = s.pool.get_hedge(&id).unwrap().margin;
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 2 * margin, -margin);
}

#[test]
fn settle_refused_while_no_rate_and_hedge_stays_open() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    // personne ne règle pendant plus de 24 h : l'oracle a oublié le cours d'échéance, rien n'a été figé.
    // 312, couverture ouverte (cas du futur `settle_fallback`).
    s.env.ledger().set_timestamp(now + MATURITY_IN + 86_401);
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(PricingError::NoFixing as u32))));
    assert!(!s.pool.get_hedge(&id).unwrap().settled);
}

/// Un jour plus une seconde : Reflector (et le mock) ont oublié le cours d'échéance.
const AFTER_HISTORY: u64 = 86_401;

// ---- Tests aléatoires du règlement (dossier Instaward, livrable 1) ----
//
// Des scénarios tirés au hasard (graine fixe, donc rejouables) enchaînent ouvertures dans les deux sens, mouvements du
// peso, passage du temps, règlements, clôtures anticipées, voie de secours et réclamations. Après chaque étape on
// vérifie les propriétés qui doivent tenir quoi qu'il arrive :
// 1. conservation de l'USDC : solde du contrat = capital des LPs + marges des couvertures ouvertes + soldes réclamables ;
// 2. jeu à somme nulle : ce que le pool a gagné ou perdu = − Σ P&L des PME sur les couvertures closes ;
// 3. bornes : chaque versement est entre 0 et 2 × marge, et la réserve du pool = Σ marges ouvertes ;
// 4. l'argent de la PME : son wallet + son solde réclamable + ses marges ouvertes = départ + Σ P&L.

extern crate std;
use std::{format, vec::Vec};

/// Générateur pseudo-aléatoire minimal (xorshift64*), sans dépendance : même graine, même scénario.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Entier dans [lo, hi].
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i128
    }
}

/// Couverture ouverte suivie par le test.
struct Open {
    id: u64,
    margin: i128,
    maturity: u64,
}

/// État attendu, tenu à côté du contrat.
#[derive(Default)]
struct Books {
    /// Σ P&L des PME sur les couvertures closes (versement − marge).
    user_pnl: i128,
    closed: u32,
    fallbacks: u32,
}

fn check_invariants(s: &Setup, open: &[Open], books: &Books, step: &str) {
    let margins: i128 = open.iter().map(|h| h.margin).sum();
    let total = s.pool.total_assets();
    let claimable = s.pool.total_claimable();
    // 1. conservation de l'USDC
    assert_eq!(s.usdc.balance(&s.pool.address), total + margins + claimable, "conservation ({step})");
    // 2. somme nulle entre le pool et les PME
    assert_eq!(total - LP_CAPITAL, -books.user_pnl, "somme nulle ({step})");
    // 3. réserve = marges ouvertes
    let e = s.pool.exposure(&Symbol::new(&s.env, "MXN"));
    assert_eq!(e.buy_margin + e.sell_margin, margins, "réserve ({step})");
    // 4. l'argent de Lucía
    assert_eq!(
        s.usdc.balance(&s.lucia) + s.pool.claimable(&s.lucia) + margins,
        LUCIA_START + books.user_pnl,
        "argent de la PME ({step})"
    );
}

/// Clôt une couverture échue comme le ferait le robot : `settle` si un cours existe, sinon `settle_fallback` si
/// l'absence de cours est constatée. `None` si ni l'un ni l'autre n'est encore possible (cours en attente).
fn close_matured(s: &Setup, id: u64, margin: i128, books: &mut Books) -> Option<i128> {
    if let Ok(Ok(payout)) = s.pool.try_settle(&id) {
        return Some(payout);
    }
    match s.pool.try_settle_fallback(&id) {
        Ok(Ok(payout)) => {
            assert_eq!(payout, margin, "secours : marge rendue telle quelle");
            books.fallbacks += 1;
            Some(payout)
        }
        // un cours existe ou peut encore arriver : le secours refuse, c'est voulu
        Err(Ok(e)) if e == contract_err(HedgeError::FixingAvailable as u32) => None,
        other => panic!("clôture inattendue : {:?}", other),
    }
}

/// Ferme `h` par `settle`, `close_early` ou `settle_fallback` et vérifie les bornes du versement.
fn record_close(open: &mut Vec<Open>, idx: usize, payout: i128, books: &mut Books) {
    let h = open.swap_remove(idx);
    assert!((0..=2 * h.margin).contains(&payout), "versement {payout} hors de [0, 2 × {}]", h.margin);
    books.user_pnl += payout - h.margin;
    books.closed += 1;
}

fn run_scenario(seed: u64) -> Books {
    let s = setup();
    fund_pool(&s);
    s.pool.set_rate(&Symbol::new(&s.env, "USD"), &400);
    let mxn = Symbol::new(&s.env, "MXN");
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut open: Vec<Open> = Vec::new();
    let mut books = Books::default();
    let mut spot = SPOT_0;
    let mut now = s.env.ledger().timestamp();

    for step in 0..40 {
        // le temps passe (au plus 5 min, sous l'âge maximal du prix) et le peso bouge de ±3 % au plus
        now += rng.range(1, 300) as u64;
        spot = (spot * rng.range(9_700, 10_300) / 10_000).clamp(SPOT_0 / 2, SPOT_0 * 2);
        price_at(&s, now, spot);

        match rng.range(0, 9) {
            // ouvrir, dans un sens ou l'autre (un refus de capacité ou de limite est normal)
            0..=3 => {
                let side = if rng.range(0, 1) == 0 { Side::BuyCurrency } else { Side::SellCurrency };
                let notional = rng.range(1, 2_000_000) * 10_000_000;
                let duration = rng.range(60, 1_200) as u64;
                let cap = rng.range(500, 2_000) as u32;
                if let Ok(Ok(id)) = s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &side, &notional, &duration, &cap, &None) {
                    let h = s.pool.get_hedge(&id).unwrap();
                    open.push(Open { id, margin: h.margin, maturity: h.maturity });
                }
            }
            // régler tout ce qui est échu
            4..=5 => {
                let mut i = 0;
                while i < open.len() {
                    if open[i].maturity <= now {
                        if let Some(payout) = close_matured(&s, open[i].id, open[i].margin, &mut books) {
                            record_close(&mut open, i, payout, &mut books);
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            // clôture anticipée d'une couverture pas encore échue
            6 => {
                if let Some(i) = open.iter().position(|h| h.maturity > now) {
                    let payout = s.pool.close_early(&open[i].id);
                    record_close(&mut open, i, payout, &mut books);
                }
            }
            // la PME réclame (ou le robot pousse) son solde
            7 => {
                if rng.range(0, 1) == 0 { s.pool.claim(&s.lucia) } else { s.pool.claim_for(&s.lucia) };
                assert_eq!(s.pool.claimable(&s.lucia), 0);
            }
            // personne ne règle pendant plus d'un jour : l'oracle oublie, voie de secours à P&L nul
            8 => {
                if let Some(i) = open.iter().position(|h| h.maturity <= now) {
                    // au-delà de l'heure de grâce plus 24 h : plus aucun cours de la fenêtre n'est lisible
                    now = now.max(open[i].maturity + 3_600) + 86_401;
                    s.env.ledger().set_timestamp(now);
                    let (id, margin) = (open[i].id, open[i].margin);
                    let fallbacks = books.fallbacks;
                    let payout = close_matured(&s, id, margin, &mut books).expect("plus aucun cours : secours possible");
                    assert_eq!(books.fallbacks, fallbacks + 1, "c'est bien la voie de secours qui a servi");
                    record_close(&mut open, i, payout, &mut books);
                    // prix frais pour la suite
                    price_at(&s, now, spot);
                }
            }
            _ => {}
        }
        check_invariants(&s, &open, &books, &format!("graine {seed}, étape {step}"));
    }

    // fin : tout ce qui reste est réglé (ou clos en secours), tout est réclamé, le pool retrouve sa capacité
    // le robot repasse jusqu'à ce que tout soit clos : règlement si un cours existe, secours sinon
    for _ in 0..5 {
        now += 1_200;
        price_at(&s, now, spot);
        now += 300; // pas de lecture de 300 s du pricing : le cours devient lisible
        s.env.ledger().set_timestamp(now);
        let mut i = 0;
        while i < open.len() {
            if let Some(payout) = close_matured(&s, open[i].id, open[i].margin, &mut books) {
                record_close(&mut open, i, payout, &mut books);
            } else {
                i += 1;
            }
        }
        if open.is_empty() {
            break;
        }
        now += 86_400 + 3_600; // ce qui reste en attente finit en secours
        s.env.ledger().set_timestamp(now);
    }
    assert!(open.is_empty(), "graine {seed} : couvertures jamais closes");
    s.pool.claim(&s.lucia);
    check_invariants(&s, &open, &books, &format!("graine {seed}, fin"));
    assert_eq!(s.pool.total_claimable(), 0);
    assert_eq!(s.usdc.balance(&s.pool.address), s.pool.total_assets());
    books
}

/// 40 scénarios par défaut (~20 s) ; `POOL_SCENARIOS=1000 cargo test -p pool random_scenarios` pour une passe longue.
#[test]
fn random_scenarios_keep_settlement_invariants() {
    let n: u64 = std::env::var("POOL_SCENARIOS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    let (mut closed, mut fallbacks) = (0u64, 0u64);
    for seed in 1..=n {
        let b = run_scenario(seed);
        closed += b.closed as u64;
        fallbacks += b.fallbacks as u64;
    }
    // les scénarios exercent vraiment chaque voie de sortie
    assert!(closed > 7 * n, "{closed} couvertures closes seulement");
    assert!(fallbacks > n / 4, "{fallbacks} passages par la voie de secours seulement");
}

#[test]
fn pnl_is_bounded_and_rounds_for_the_pool() {
    let env = Env::default();
    let mut rng = Rng(0xDEAD_BEEF);
    for _ in 0..20_000 {
        let side = if rng.range(0, 1) == 0 { Side::BuyCurrency } else { Side::SellCurrency };
        let notional = rng.range(1, 100_000_000) * 10_000_000;
        let locked = rng.range(1_000_000_000, 100_000_000_000_000);
        let spot = rng.range(1_000_000_000, 100_000_000_000_000);
        let margin = rng.range(1, 1_000_000_000_000);
        let pnl = hedge::pnl(&env, side, notional, locked, spot, margin);
        // bornée à ±marge
        assert!((-margin..=margin).contains(&pnl));
        // jamais plus que le P&L exact : l'arrondi va au pool
        let diff = match side {
            Side::BuyCurrency => spot - locked,
            Side::SellCurrency => locked - spot,
        };
        let exact_scaled = notional * diff; // P&L exact × 10^14
        assert!(pnl * 100_000_000_000_000 <= exact_scaled.max(-margin * 100_000_000_000_000));
        // même sens que le marché (sauf arrondi d'une unité contre la PME)
        if diff > 0 {
            assert!(pnl >= 0);
        }
        if diff < 0 {
            assert!(pnl < 0);
        }
    }
}

// ---- Pilote côté couvertures (B9) ----

/// Une PME hors liste blanche, avec de quoi payer une marge.
fn outsider(s: &Setup) -> Address {
    let who = Address::generate(&s.env);
    StellarAssetClient::new(&s.env, &s.usdc.address).mint(&who, &LUCIA_START);
    who
}

fn open_as(s: &Setup, who: &Address) -> Result<u64, soroban_sdk::Error> {
    match s.pool.try_open_hedge(who, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None) {
        Ok(Ok(id)) => Ok(id),
        Err(Ok(e)) => Err(e),
        other => panic!("résultat inattendu : {:?}", other),
    }
}

#[test]
fn sme_allowlist_is_on_by_default() {
    let s = setup();
    fund_pool(&s);
    let bob = outsider(&s);
    assert!(s.pool.pilot_limits().sme_allowlist_on);
    assert!(!s.pool.is_allowed(&Role::Sme, &bob));
    assert_eq!(open_as(&s, &bob), Err(contract_err(HedgeError::SmeNotAllowed as u32)));

    // l'admin l'ajoute : il peut ouvrir
    s.pool.set_allowed(&Role::Sme, &bob, &true);
    assert_eq!(s.env.auths()[0].0, s.admin);
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(
        events.events().last(),
        Some(&hedge::SmeAllowedSet { sme: bob.clone(), allowed: true }.to_xdr(&s.env, &s.pool.address))
    );
    assert!(s.pool.is_allowed(&Role::Sme, &bob));
    assert!(open_as(&s, &bob).is_ok());
}

#[test]
fn sme_allowlist_can_be_turned_off() {
    let s = setup();
    fund_pool(&s);
    let bob = outsider(&s);
    s.pool.set_allowlist_enabled(&Role::Sme, &false);
    assert!(!s.pool.pilot_limits().sme_allowlist_on);
    assert!(s.pool.is_allowed(&Role::Sme, &bob));
    assert!(open_as(&s, &bob).is_ok());
    // l'interrupteur des PME ne touche pas à celui des LP
    assert!(s.pool.pilot_limits().lp_allowlist_on);
}

/// Une PME retirée de la liste ne peut plus ouvrir, mais ses couvertures ouvertes se règlent normalement.
#[test]
fn removed_sme_keeps_its_open_hedges() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.pool.set_allowed(&Role::Sme, &s.lucia, &false);
    assert_eq!(open_as(&s, &s.lucia), Err(contract_err(HedgeError::SmeNotAllowed as u32)));
    at_maturity(&s, 5_940_000_000_000);
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 96_525_028_394, -41_250_031_549);
}

#[test]
fn hedge_limits_cap_size_duration_and_open_count() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");
    let open = |notional: i128, duration: u64| {
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &notional, &duration, &1000, &None)
    };

    // montant : 55 274,99 USDC maximum par couverture, juste sous la facture de démo
    s.pool.set_hedge_limits(&Some(HEDGE_COST - 1), &None, &None);
    assert_eq!(open(NOTIONAL, MATURITY_IN), Err(Ok(contract_err(HedgeError::HedgeTooLarge as u32))));
    s.pool.set_hedge_limits(&Some(HEDGE_COST), &None, &None);
    let first = open(NOTIONAL, MATURITY_IN).unwrap().unwrap();

    // durée : 60 s maximum
    s.pool.set_hedge_limits(&None, &Some(MATURITY_IN), &None);
    assert_eq!(open(NOTIONAL, MATURITY_IN + 1), Err(Ok(contract_err(HedgeError::HedgeTooLong as u32))));

    // nombre : une seule couverture ouverte par PME ; une fois réglée, la place se libère
    s.pool.set_hedge_limits(&None, &None, &Some(1));
    assert_eq!(open(NOTIONAL, MATURITY_IN), Err(Ok(contract_err(HedgeError::TooManyOpenHedges as u32))));
    at_maturity(&s, SPOT_0);
    s.pool.settle(&first);
    set_spot(&s, SPOT_0);
    assert!(open(NOTIONAL, MATURITY_IN).is_ok());

    let limits = s.pool.pilot_limits();
    assert_eq!((limits.max_hedge_notional, limits.max_duration_secs, limits.max_open_per_sme), (None, None, Some(1)));
}

#[test]
fn open_count_is_released_by_every_way_out() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();
    s.pool.set_hedge_limits(&None, &None, &Some(1));
    // clôture anticipée
    let id = open_as(&s, &s.lucia).unwrap();
    s.env.ledger().set_timestamp(now + 10);
    set_spot(&s, SPOT_0);
    s.pool.close_early(&id);
    // voie de secours
    let id = open_as(&s, &s.lucia).unwrap();
    s.env.ledger().set_timestamp(now + 10 + MATURITY_IN + AFTER_HISTORY);
    s.pool.settle_fallback(&id);
    set_spot(&s, SPOT_0);
    assert!(open_as(&s, &s.lucia).is_ok());
}

#[test]
fn hedge_limits_are_admin_only_public_and_checked() {
    let s = setup();
    s.pool.set_hedge_limits(&Some(HEDGE_COST), &Some(120 * 86_400), &Some(3));
    assert_eq!(s.env.auths()[0].0, s.admin);
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(
        events.events().last(),
        Some(
            &hedge::HedgeLimitsSet { max_hedge_notional: Some(HEDGE_COST), max_duration_secs: Some(120 * 86_400), max_open_per_sme: Some(3) }
                .to_xdr(&s.env, &s.pool.address)
        )
    );
    let invalid = Err(Ok(contract_err(HedgeError::InvalidLimit as u32)));
    assert_eq!(s.pool.try_set_hedge_limits(&Some(0), &None, &None), invalid);
    assert_eq!(s.pool.try_set_hedge_limits(&None, &Some(0), &None), invalid);
    assert_eq!(s.pool.try_set_hedge_limits(&None, &None, &Some(0)), invalid);
}

// ---- Taux limite à l'ouverture (B8) ----

#[test]
fn open_refused_when_rate_is_worse_than_the_limit() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");

    // importateur : refusé si le taux bloqué dépasse son maximum, accepté pile au maximum
    let worse = s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &Some(HEDGE_LOCKED - 1));
    assert_eq!(worse, Err(Ok(contract_err(HedgeError::RateWorseThanLimit as u32))));
    // rien n'a bougé : pas de marge prise, pas de couverture
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START);
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &Some(HEDGE_LOCKED));
    assert_eq!(s.pool.get_hedge(&id).unwrap().locked_rate, HEDGE_LOCKED);

    // exportateur : refusé si le taux bloqué passe sous son minimum, accepté pile au minimum
    let sell_rate = s.pool.quote(&mxn, &Side::SellCurrency, &NOTIONAL, &MATURITY_IN, &1000).forward_rate;
    let worse = s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::SellCurrency, &NOTIONAL, &MATURITY_IN, &1000, &Some(sell_rate + 1));
    assert_eq!(worse, Err(Ok(contract_err(HedgeError::RateWorseThanLimit as u32))));
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &mxn, &Side::SellCurrency, &NOTIONAL, &MATURITY_IN, &1000, &Some(sell_rate));
    assert_eq!(s.pool.get_hedge(&id).unwrap().locked_rate, sell_rate);
}

/// Le cours bouge entre le devis affiché et la signature : le taux limite protège la PME.
#[test]
fn rate_limit_protects_against_a_move_after_the_quote() {
    let s = setup();
    fund_pool(&s);
    let mxn = Symbol::new(&s.env, "MXN");
    let quoted = s.pool.quote(&mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000).forward_rate;
    set_spot(&s, 5_610_000_000_000); // +2 % avant que la transaction passe
    assert_eq!(
        s.pool.try_open_hedge(&s.lucia, &s.supplier, &mxn, &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &Some(quoted)),
        Err(Ok(contract_err(HedgeError::RateWorseThanLimit as u32)))
    );
}

// ---- Solde réclamable (B4) ----

/// Retire (ou rend) à Lucía le droit de recevoir de l'USDC : même effet qu'une ligne de confiance supprimée.
fn set_lucia_can_receive(s: &Setup, ok: bool) {
    StellarAssetClient::new(&s.env, &s.usdc.address).set_authorized(&s.lucia, &ok);
}

#[test]
fn claim_requires_owner_signature_and_emits_event() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000);
    let payout = s.pool.settle(&id);

    assert_eq!(s.pool.claim(&s.lucia), payout);
    let auths = s.env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.lucia);
    let expected = hedge::Claimed { owner: s.lucia.clone(), amount: payout };
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(events.events().last(), Some(&expected.to_xdr(&s.env, &s.pool.address)));
}

#[test]
fn claim_for_pays_the_owner_without_signature() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000);
    let payout = s.pool.settle(&id);

    // le robot pousse le versement : sans signature, et l'argent va toujours au propriétaire
    assert_eq!(s.pool.set_auths(&[]).claim_for(&s.lucia), payout);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START - HEDGE_MARGIN + payout);
    assert_eq!(s.pool.claimable(&s.lucia), 0);
}

#[test]
fn claim_with_nothing_due_pays_nothing() {
    let s = setup();
    assert_eq!(s.pool.claim(&s.lucia), 0);
    assert_eq!(s.pool.claim_for(&s.lucia), 0);
    assert_eq!(s.usdc.balance(&s.lucia), LUCIA_START);
}

#[test]
fn claimable_adds_up_across_hedges() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    let id2 = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    at_maturity(&s, 5_940_000_000_000);
    let total = s.pool.settle(&id) + s.pool.settle(&id2);
    assert_eq!(s.pool.claimable(&s.lucia), total);
    assert_eq!(s.pool.total_claimable(), total);
    assert_eq!(s.pool.claim(&s.lucia), total);
}

/// Scénario de la relecture de Hugo : une PME perdante supprime sa ligne USDC pour faire échouer le règlement,
/// attend que l'oracle oublie le cours, puis passe par la voie de secours pour récupérer toute sa marge.
/// Avec le solde réclamable, le règlement ne dépend plus de son compte : il passe, et le secours est fermé.
#[test]
fn owner_who_cannot_receive_cannot_block_settlement() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_060_000_000_000); // −8 % : Lucía perd une partie de sa marge
    set_lucia_can_receive(&s, false);

    let payout = s.pool.settle(&id);
    assert!(payout > 0 && payout < HEDGE_MARGIN);
    assert!(s.pool.get_hedge(&id).unwrap().settled);
    // plus de secours possible : la perte est actée
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + AFTER_HISTORY);
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));

    // elle ne peut pas réclamer tant qu'elle ne peut pas recevoir ; son solde l'attend
    assert!(s.pool.try_claim(&s.lucia).is_err());
    assert_eq!(s.pool.claimable(&s.lucia), payout);
    set_lucia_can_receive(&s, true);
    assert_eq!(s.pool.claim(&s.lucia), payout);
}

#[test]
fn claim_works_while_paused() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    at_maturity(&s, 5_940_000_000_000);
    let payout = s.pool.settle(&id);
    s.pool.pause(&s.admin);
    assert_eq!(s.pool.claim(&s.lucia), payout);
}

// ---- Cours figé et règlement de secours ----

#[test]
fn record_fixing_locks_rate_for_a_later_settle() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    // crash +12 % juste avant l'échéance ; le robot fige le cours une minute après, sans signature
    price_at(&s, now + MATURITY_IN - 10, 6_160_000_000_000);
    s.env.ledger().set_timestamp(now + MATURITY_IN + 60);
    assert_eq!(s.pool.set_auths(&[]).record_fixing(&id), 6_160_000_000_000);
    // appel répétable : même cours, rien ne change
    assert_eq!(s.pool.record_fixing(&id), 6_160_000_000_000);
    assert!(!s.pool.get_hedge(&id).unwrap().settled);

    // règlement plus de 24 h après : l'oracle a oublié, le cours figé sert
    s.env.ledger().set_timestamp(now + MATURITY_IN + AFTER_HISTORY);
    let paid = s.pool.settle(&id);
    assert_settled(&s, id, paid, 2 * HEDGE_MARGIN, -HEDGE_MARGIN);
}

#[test]
fn record_fixing_guards() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    assert_eq!(s.pool.try_record_fixing(&99), Err(Ok(contract_err(HedgeError::UnknownHedge as u32))));
    assert_eq!(s.pool.try_record_fixing(&id), Err(Ok(contract_err(HedgeError::NotMatured as u32))));

    // échéance passée mais pas encore de cours utilisable
    s.env.ledger().set_timestamp(now + MATURITY_IN + AFTER_HISTORY);
    assert_eq!(s.pool.try_record_fixing(&id), Err(Ok(contract_err(PricingError::NoFixing as u32))));

    // couverture réglée : plus rien à figer (cours frais pour pouvoir ouvrir)
    set_spot(&s, SPOT_0);
    let id2 = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    at_maturity(&s, SPOT_0);
    s.pool.settle(&id2);
    assert_eq!(s.pool.try_record_fixing(&id2), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));
}

#[test]
fn settle_fallback_closes_without_gain_or_loss() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    // personne n'a figé ni réglé pendant 24 h : plus aucun cours, n'importe qui clôt sans signature
    s.env.ledger().set_timestamp(now + MATURITY_IN + AFTER_HISTORY);
    let paid = s.pool.set_auths(&[]).settle_fallback(&id);
    // événement public (lu avant tout autre appel : les tests ne gardent que ceux du dernier appel)
    let expected = hedge::HedgeFallbackSettled { hedge_id: id, owner: s.lucia.clone(), payout: HEDGE_MARGIN };
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(events.events().last(), Some(&expected.to_xdr(&s.env, &s.pool.address)));

    // marge rendue telle quelle, P&L nul, capital libéré, exposition à zéro
    assert_settled(&s, id, paid, HEDGE_MARGIN, 0);
    assert_eq!(s.pool.exposure(&Symbol::new(&s.env, "MXN")).buy_margin, 0);

    // une seule fois
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));
    assert_eq!(s.pool.try_settle(&id), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));
}

#[test]
fn settle_fallback_refused_while_a_rate_exists_or_may_come() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);

    assert_eq!(s.pool.try_settle_fallback(&99), Err(Ok(contract_err(HedgeError::UnknownHedge as u32))));
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::NotMatured as u32))));

    // cours disponible à l'échéance : il faut régler normalement
    at_maturity(&s, 6_160_000_000_000);
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::FixingAvailable as u32))));

    // cours figé puis oublié par l'oracle : le cours figé compte toujours, pas de secours
    s.pool.record_fixing(&id);
    s.env.ledger().set_timestamp(now + MATURITY_IN + AFTER_HISTORY);
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::FixingAvailable as u32))));
    assert!(!s.pool.get_hedge(&id).unwrap().settled);
}

#[test]
fn settle_fallback_refused_during_grace_window() {
    let s = setup();
    fund_pool(&s);
    let now = s.env.ledger().timestamp();
    // échéance à 2 000 s, aucun cours posé depuis l'ouverture : un cours peut encore arriver pendant une heure
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &2_000, &1000, &None);
    s.env.ledger().set_timestamp(now + 2_000 + 1_800);
    assert_eq!(s.pool.try_settle_fallback(&id), Err(Ok(contract_err(HedgeError::FixingAvailable as u32))));
    assert!(!s.pool.get_hedge(&id).unwrap().settled);
}

#[test]
fn record_fixing_and_fallback_work_while_paused() {
    let s = setup();
    let now = s.env.ledger().timestamp();
    let id = open_funded(&s, Side::BuyCurrency);
    let id2 = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &MATURITY_IN, &1000, &None);
    s.pool.pause(&s.admin);

    // on ne bloque jamais l'argent des gens : figer et clore en secours restent possibles en pause
    at_maturity(&s, SPOT_0);
    assert_eq!(s.pool.record_fixing(&id), SPOT_0);
    s.env.ledger().set_timestamp(now + MATURITY_IN + AFTER_HISTORY);
    assert_eq!(s.pool.settle_fallback(&id2), s.pool.get_hedge(&id2).unwrap().margin);
    assert_eq!(s.pool.claimable(&s.lucia), s.pool.get_hedge(&id2).unwrap().margin);
}

// ---- Clôture anticipée ----

/// Taux de sortie attendu : forward du jour pour `remaining` secondes, sens inverse de `side`.
fn exit_rate(s: &Setup, side: Side, remaining: u64) -> i128 {
    let exit = match side {
        Side::BuyCurrency => Side::SellCurrency,
        Side::SellCurrency => Side::BuyCurrency,
    };
    s.env.as_contract(&s.pool.address, || crate::pricing::forward_rate(&s.env, &Symbol::new(&s.env, "MXN"), exit, remaining))
}

#[test]
fn close_early_unchanged_market_costs_the_spreads() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + MATURITY_IN / 2);
    set_spot(&s, SPOT_0);

    // cours inchangé : Lucía perd l'aller-retour des spreads (≈ 1 %), rien de plus
    let rate = exit_rate(&s, Side::BuyCurrency, MATURITY_IN / 2);
    let pnl = hedge::pnl(&s.env, Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, rate, HEDGE_MARGIN);
    assert!(pnl < 0 && pnl > -HEDGE_COST / 90);
    let paid = s.pool.close_early(&id);
    assert_closed_early(&s, id, paid, HEDGE_MARGIN + pnl, -pnl);
}

#[test]
fn close_early_currency_up_user_gains() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 10);
    set_spot(&s, 5_940_000_000_000); // +8 % : le pool perd, la PME sort avec son gain moins le spread

    let rate = exit_rate(&s, Side::BuyCurrency, MATURITY_IN - 10);
    let pnl = hedge::pnl(&s.env, Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, rate, HEDGE_MARGIN);
    assert!(pnl > 0);
    let paid = s.pool.close_early(&id);
    assert_closed_early(&s, id, paid, HEDGE_MARGIN + pnl, -pnl);
}

#[test]
fn close_early_currency_down_user_pays_capped() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 10);
    set_spot(&s, 4_675_000_000_000); // −15 % : perte bornée à la marge, toute la marge va au pool

    let paid = s.pool.close_early(&id);
    assert_closed_early(&s, id, paid, 0, HEDGE_MARGIN);
}

#[test]
fn close_early_sell_side_buys_back() {
    let s = setup();
    let id = open_funded(&s, Side::SellCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 10);
    set_spot(&s, 5_060_000_000_000); // −8 % : l'exportateur gagne

    let rate = exit_rate(&s, Side::SellCurrency, MATURITY_IN - 10);
    let pnl = hedge::pnl(&s.env, Side::SellCurrency, NOTIONAL, s.pool.get_hedge(&id).unwrap().locked_rate, rate, s.pool.get_hedge(&id).unwrap().margin);
    assert!(pnl > 0);
    let margin = s.pool.get_hedge(&id).unwrap().margin;
    let paid = s.pool.close_early(&id);
    assert_closed_early(&s, id, paid, margin + pnl, -pnl);
}

#[test]
fn close_early_guards() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);

    // à l'échéance : il faut passer par settle
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + MATURITY_IN);
    set_spot(&s, SPOT_0);
    assert_eq!(s.pool.try_close_early(&id), Err(Ok(contract_err(HedgeError::AlreadyMatured as u32))));
    assert_eq!(s.pool.try_close_early(&42), Err(Ok(contract_err(HedgeError::UnknownHedge as u32))));

    // après settle : déjà réglée
    s.pool.settle(&id);
    assert_eq!(s.pool.try_close_early(&id), Err(Ok(contract_err(HedgeError::AlreadySettled as u32))));
}

#[test]
fn close_early_refuses_stale_price() {
    let s = setup();
    fund_pool(&s);
    let id = s.pool.open_hedge(&s.lucia, &s.supplier, &Symbol::new(&s.env, "MXN"), &Side::BuyCurrency, &NOTIONAL, &86_400, &1000, &None);
    // pas de nouveau prix depuis l'ouverture, plus de 900 s plus tard : refusé comme un devis
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 901);
    assert_eq!(s.pool.try_close_early(&id), Err(Ok(contract_err(PricingError::StalePrice as u32))));
}

#[test]
fn close_early_requires_owner() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 10);
    set_spot(&s, SPOT_0);
    s.pool.close_early(&id);
    let auths = s.env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.lucia);
}

#[test]
fn close_early_emits_event() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    s.env.ledger().set_timestamp(s.env.ledger().timestamp() + 10);
    set_spot(&s, 5_940_000_000_000);
    let rate = exit_rate(&s, Side::BuyCurrency, MATURITY_IN - 10);
    let pnl = hedge::pnl(&s.env, Side::BuyCurrency, NOTIONAL, HEDGE_LOCKED, rate, HEDGE_MARGIN);
    s.pool.close_early(&id);

    let expected = hedge::HedgeClosedEarly { hedge_id: id, owner: s.lucia.clone(), close_rate: rate, pnl, payout: HEDGE_MARGIN + pnl };
    let events = s.env.events().all().filter_by_contract(&s.pool.address);
    assert_eq!(events.events().last(), Some(&expected.to_xdr(&s.env, &s.pool.address)));
}

// ---- B9 : notionnel total sur le vrai parcours ----

#[test]
fn total_notional_back_to_zero_after_settle() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    assert_eq!(s.pool.total_notional(), HEDGE_COST);
    at_maturity(&s, SPOT_0);
    s.pool.settle(&id);
    assert_eq!(s.pool.total_notional(), 0);
}

#[test]
fn total_notional_back_to_zero_after_close_early() {
    let s = setup();
    let id = open_funded(&s, Side::BuyCurrency);
    assert_eq!(s.pool.total_notional(), HEDGE_COST);
    s.pool.close_early(&id);
    assert_eq!(s.pool.total_notional(), 0);
}
