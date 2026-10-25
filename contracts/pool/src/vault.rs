//! VAULT — dépôts / retraits des LPs, capital réservé, flux USDC du pool.
//!
//! Compta : `TotalDeposits` = capital du pool (bouge avec le P&L des couvertures).
//! `Reserved` = Σ des marges des couvertures ouvertes. Capacité = TotalDeposits - Reserved.
//! Invariant : solde USDC du contrat = TotalDeposits + Σ marges des couvertures ouvertes
//! (les marges appartiennent aux PME, pas aux LPs).
//! MVP : soldes LP en mapping simple (pas de parts SEP-41). Si le P&L doit être réparti
//! entre LPs, passer à des parts (shares) : décision du Vault.
use crate::DataKey;
use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env};

/// Erreurs du vault. Codes 100+ pour ne pas entrer en collision avec Paiement / Pricing.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    /// Montant nul ou négatif (négatif seulement pour `book_gain` / `book_loss`).
    InvalidAmount = 100,
    /// `reserve` au-delà du capital non réservé.
    InsufficientCapacity = 101,
    /// `release` au-delà du capital réservé.
    ReleaseExceedsReserved = 102,
    /// `book_loss` rendrait le capital des LPs négatif.
    LossExceedsCapital = 103,
}

/// Clés de stockage propres au vault (phase 2 : parts LP).
/// `TotalDeposits` et `Reserved` restent dans `DataKey` (posées par le constructeur).
#[contracttype]
#[allow(dead_code)]
pub enum VaultKey {
    Shares(Address),
    TotalShares,
}

fn total_deposits(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::TotalDeposits).unwrap_or(0)
}

fn set_total_deposits(env: &Env, value: i128) {
    env.storage().instance().set(&DataKey::TotalDeposits, &value);
}

fn reserved(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::Reserved).unwrap_or(0)
}

fn set_reserved(env: &Env, value: i128) {
    env.storage().instance().set(&DataKey::Reserved, &value);
}

// ---- appelé par lib.rs (points d'entrée publics) ----

pub(crate) fn deposit(_env: &Env, _lp: &Address, _amount: i128) -> i128 {
    panic!("TODO vault")
}

pub(crate) fn withdraw(_env: &Env, _lp: &Address, _amount: i128) -> i128 {
    panic!("TODO vault")
}

pub(crate) fn balance(env: &Env, lp: &Address) -> i128 {
    env.storage().persistent().get(&DataKey::LpBalance(lp.clone())).unwrap_or(0)
}

pub(crate) fn capacity(env: &Env) -> i128 {
    total_deposits(env) - reserved(env)
}

// ---- appelé par hedge.rs (interface Paiement → Vault) ----

/// Bloque `amount` USDC de capacité pour une nouvelle couverture. Panique si capacité insuffisante.
pub(crate) fn reserve(_env: &Env, _amount: i128) {
    panic!("TODO vault")
}

/// Libère `amount` de capacité réservée (couverture réglée).
pub(crate) fn release(_env: &Env, _amount: i128) {
    panic!("TODO vault")
}

/// Le pool encaisse un gain (la PME a perdu sur sa marge) : `TotalDeposits += amount`.
/// Les USDC sont déjà dans le contrat (ils viennent de la marge).
pub(crate) fn book_gain(_env: &Env, _amount: i128) {
    panic!("TODO vault")
}

/// Le pool paie une perte (la PME est protégée) : `TotalDeposits -= amount`.
/// Le transfert vers la PME est fait par hedge.rs avec la restitution de la marge.
pub(crate) fn book_loss(_env: &Env, _amount: i128) {
    panic!("TODO vault")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pool;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::token;

    /// 1 USDC en 7 décimales.
    const USDC: i128 = 10_000_000;

    struct Setup {
        env: Env,
        pool: Address,
        usdc: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let oracle = Address::generate(&env);
        let pool = env.register(Pool, (&admin, &usdc, &oracle, &50u32));
        Setup { env, pool, usdc }
    }

    impl Setup {
        fn total_deposits(&self) -> i128 {
            self.env.as_contract(&self.pool, || total_deposits(&self.env))
        }

        fn reserved(&self) -> i128 {
            self.env.as_contract(&self.pool, || reserved(&self.env))
        }

        fn usdc_balance(&self, who: &Address) -> i128 {
            token::Client::new(&self.env, &self.usdc).balance(who)
        }

        /// Dépôt LP simulé (phase 2 pas encore écrite) : USDC vers le contrat + TotalDeposits.
        fn simulate_lp_deposit(&self, amount: i128) {
            token::StellarAssetClient::new(&self.env, &self.usdc).mint(&self.pool, &amount);
            self.env.as_contract(&self.pool, || {
                set_total_deposits(&self.env, total_deposits(&self.env) + amount)
            });
        }

        /// Marge simulée : la PME envoie ses USDC au contrat (comme le fera hedge::open).
        fn simulate_margin(&self, sme: &Address, margin: i128) {
            token::StellarAssetClient::new(&self.env, &self.usdc).mint(sme, &margin);
            token::Client::new(&self.env, &self.usdc).transfer(sme, &self.pool, &margin);
        }

        /// Transfert contrat → PME fait depuis le contrat, comme le fera hedge::settle.
        fn pay_sme(&self, sme: &Address, amount: i128) {
            self.env.as_contract(&self.pool, || {
                token::Client::new(&self.env, &self.usdc).transfer(&self.pool, sme, &amount)
            });
        }

        fn vault<R>(&self, f: impl FnOnce(&Env) -> R) -> R {
            self.env.as_contract(&self.pool, || f(&self.env))
        }

        /// Invariant : solde USDC du contrat = capital des LPs + ce qui est dû aux PME.
        /// Hors règlement, `owed_to_smes` = Σ marges ouvertes ; pendant un règlement,
        /// c'est marge ± P&L de la couverture en cours.
        fn assert_invariant(&self, owed_to_smes: i128) {
            assert_eq!(
                self.usdc_balance(&self.pool),
                self.total_deposits() + owed_to_smes,
                "invariant : solde USDC = TotalDeposits + dû aux PME"
            );
        }
    }

    // ---- Garde-fous ----

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn reserve_over_capacity_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 1_000 * USDC + 1));
    }

    #[test]
    fn reserve_up_to_capacity_then_capacity_is_zero() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 1_000 * USDC));
        assert_eq!(s.vault(capacity), 0);
        assert_eq!(s.reserved(), 1_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn second_reserve_over_remaining_capacity_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 600 * USDC));
        s.vault(|env| reserve(env, 600 * USDC));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn reserve_zero_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 0));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn reserve_negative_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #102)")]
    fn release_over_reserved_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| release(env, 100 * USDC + 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn release_zero_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| release(env, 0));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn release_negative_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| release(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn book_gain_negative_panics() {
        let s = setup();
        s.vault(|env| book_gain(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn book_loss_negative_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| book_loss(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #103)")]
    fn book_loss_over_capital_panics() {
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| book_loss(env, 1_000 * USDC + 1));
    }

    #[test]
    fn book_zero_is_a_no_op() {
        // Spot à l'échéance = taux bloqué : P&L nul, le règlement ne doit pas échouer.
        let s = setup();
        s.simulate_lp_deposit(1_000 * USDC);
        s.vault(|env| book_gain(env, 0));
        s.vault(|env| book_loss(env, 0));
        assert_eq!(s.total_deposits(), 1_000 * USDC);
    }

    // ---- Cas types : LP 100 000 USDC, Lucía notionnel ≈ 55 000 USDC, plafond 10 % ----

    const LP_CAPITAL: i128 = 100_000 * USDC;
    const MARGIN: i128 = 5_500 * USDC;

    /// Départ commun : dépôt LP puis ouverture de la couverture de Lucía.
    fn open_lucia() -> (Setup, Address) {
        let s = setup();
        s.assert_invariant(0);
        s.simulate_lp_deposit(LP_CAPITAL);
        s.assert_invariant(0);

        let lucia = Address::generate(&s.env);
        s.simulate_margin(&lucia, MARGIN);
        s.vault(|env| reserve(env, MARGIN));
        assert_eq!(s.reserved(), MARGIN);
        assert_eq!(s.vault(capacity), LP_CAPITAL - MARGIN);
        s.assert_invariant(MARGIN);
        (s, lucia)
    }

    #[test]
    fn case_a_peso_up_12_pct_loss_capped_at_margin() {
        let (s, lucia) = open_lucia();

        // Le pool doit 6 600, plafonné à la marge : 5 500.
        let loss = 5_500 * USDC;
        s.vault(|env| release(env, MARGIN));
        assert_eq!(s.reserved(), 0);
        s.assert_invariant(MARGIN);

        s.vault(|env| book_loss(env, loss));
        s.assert_invariant(MARGIN + loss);

        s.pay_sme(&lucia, MARGIN + loss);
        s.assert_invariant(0);

        assert_eq!(s.usdc_balance(&lucia), 11_000 * USDC);
        assert_eq!(s.total_deposits(), 94_500 * USDC);
        assert_eq!(s.reserved(), 0);
        assert_eq!(s.usdc_balance(&s.pool), 94_500 * USDC);
    }

    #[test]
    fn case_b_peso_down_8_pct_pool_gains() {
        let (s, lucia) = open_lucia();

        // Lucía doit 4 400 au pool, prélevés sur sa marge.
        let gain = 4_400 * USDC;
        s.vault(|env| release(env, MARGIN));
        s.assert_invariant(MARGIN);

        s.vault(|env| book_gain(env, gain));
        s.assert_invariant(MARGIN - gain);

        s.pay_sme(&lucia, MARGIN - gain);
        s.assert_invariant(0);

        assert_eq!(s.usdc_balance(&lucia), 1_100 * USDC);
        assert_eq!(s.total_deposits(), 104_400 * USDC);
        assert_eq!(s.reserved(), 0);
        assert_eq!(s.usdc_balance(&s.pool), 104_400 * USDC);
    }
}
