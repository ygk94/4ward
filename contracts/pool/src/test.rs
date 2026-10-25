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
    use crate::pricing::{self, Asset, PriceData, MAX_PRICE_AGE_SECS};
    use crate::{Pool, PRICE_DECIMALS};
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
}

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
