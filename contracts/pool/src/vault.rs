//! VAULT — dépôts / retraits des LPs en parts, capital réservé, flux USDC du pool.
//!
//! Compta : `TotalDeposits` = capital du pool (bouge avec le P&L des couvertures).
//! `Reserved` = Σ des marges des couvertures ouvertes. Capacité = TotalDeposits - Reserved.
//! Invariant : solde USDC du contrat = TotalDeposits + Σ marges des couvertures ouvertes
//! (les marges appartiennent aux PME, pas aux LPs).
//!
//! Parts LP (modèle de fonds, type ERC-4626), 7 décimales comme l'USDC :
//! - dépôt au prix plein `TotalDeposits / TotalShares` (1:1 au premier dépôt) ;
//! - retrait au prix de sortie `(TotalDeposits - Reserved) / TotalShares` : le capital réservé
//!   n'est pas versé, la décote reste aux LPs restants ;
//! - arrondis toujours vers le bas, en faveur du pool.
//! Le prix se calcule sur la compta interne, pas sur le solde USDC : un envoi direct d'USDC
//! au contrat ne change pas le prix de la part (pas d'attaque par inflation).
use crate::DataKey;
use soroban_sdk::{contracterror, contracttype, panic_with_error, token, Address, Env};

/// Erreurs du vault. Codes 100+ pour ne pas entrer en collision avec Paiement / Pricing.
/// Ne jamais réutiliser un numéro : ajouter à la suite.
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
    /// Dépôt refusé : il reste des parts mais plus de capital (pool vidé par des pertes).
    PoolDepleted = 104,
    /// Dépôt trop petit : il donnerait 0 part après arrondi.
    DepositTooSmall = 105,
    /// Retrait de plus de parts que le LP n'en détient.
    InsufficientShares = 106,
    /// Retrait qui verserait 0 USDC (capital entièrement réservé ou trop peu de parts).
    WithdrawTooSmall = 107,
}

/// Clés de stockage propres au vault.
/// `TotalDeposits` et `Reserved` restent dans `DataKey` (posées par le constructeur).
#[contracttype]
pub enum VaultKey {
    /// Parts d'un LP (persistent).
    Shares(Address),
    /// Total des parts émises (instance).
    TotalShares,
}

/// 1 part = 1 USDC au premier dépôt : prix exprimé en 7 décimales.
const PRICE_ONE: i128 = 10_000_000;

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

fn total_shares(env: &Env) -> i128 {
    env.storage().instance().get(&VaultKey::TotalShares).unwrap_or(0)
}

fn set_total_shares(env: &Env, value: i128) {
    env.storage().instance().set(&VaultKey::TotalShares, &value);
}

fn set_lp_shares(env: &Env, lp: &Address, value: i128) {
    let key = VaultKey::Shares(lp.clone());
    if value == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &value);
    }
}

// ---- appelé par lib.rs (points d'entrée publics) ----

/// Dépose `amount` USDC contre des parts au prix plein, arrondi vers le bas.
/// Retourne les parts du LP après le dépôt.
/// Erreurs : `InvalidAmount`, `PoolDepleted`, `DepositTooSmall`.
pub(crate) fn deposit(env: &Env, lp: &Address, amount: i128) -> i128 {
    lp.require_auth();
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let total = total_deposits(env);
    let shares_total = total_shares(env);
    let minted = if shares_total == 0 {
        amount
    } else {
        if total == 0 {
            panic_with_error!(env, VaultError::PoolDepleted);
        }
        amount * shares_total / total
    };
    if minted == 0 {
        panic_with_error!(env, VaultError::DepositTooSmall);
    }

    usdc(env).transfer(lp, &env.current_contract_address(), &amount);

    let shares = lp_shares(env, lp) + minted;
    set_lp_shares(env, lp, shares);
    set_total_shares(env, shares_total + minted);
    set_total_deposits(env, total + amount);
    shares
}

/// Brûle `shares` parts du LP et lui verse leur valeur au prix de sortie, arrondi vers le bas.
/// Le capital réservé n'est pas versé : le montant ne dépasse jamais `capacity()`.
/// Retourne l'USDC versé.
/// Erreurs : `InvalidAmount`, `InsufficientShares`, `WithdrawTooSmall`.
pub(crate) fn withdraw(env: &Env, lp: &Address, shares: i128) -> i128 {
    lp.require_auth();
    if shares <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let owned = lp_shares(env, lp);
    if shares > owned {
        panic_with_error!(env, VaultError::InsufficientShares);
    }
    let amount = shares * capacity(env) / total_shares(env);
    if amount == 0 {
        panic_with_error!(env, VaultError::WithdrawTooSmall);
    }

    // État mis à jour avant le transfert sortant.
    set_lp_shares(env, lp, owned - shares);
    set_total_shares(env, total_shares(env) - shares);
    set_total_deposits(env, total_deposits(env) - amount);

    usdc(env).transfer(&env.current_contract_address(), lp, &amount);
    amount
}

/// Valeur en USDC des parts du LP au prix de sortie (ce qu'il recevrait en retirant tout).
pub(crate) fn balance(env: &Env, lp: &Address) -> i128 {
    let shares_total = total_shares(env);
    if shares_total == 0 {
        return 0;
    }
    lp_shares(env, lp) * capacity(env) / shares_total
}

pub(crate) fn lp_shares(env: &Env, lp: &Address) -> i128 {
    env.storage().persistent().get(&VaultKey::Shares(lp.clone())).unwrap_or(0)
}

/// Prix plein d'une part : TotalDeposits / TotalShares (7 décimales), 1 USDC si aucune part.
pub(crate) fn share_price(env: &Env) -> i128 {
    price(env, total_deposits(env))
}

/// Prix de sortie d'une part : (TotalDeposits - Reserved) / TotalShares (7 décimales).
pub(crate) fn exit_price(env: &Env) -> i128 {
    price(env, capacity(env))
}

pub(crate) fn total_assets(env: &Env) -> i128 {
    total_deposits(env)
}

fn price(env: &Env, assets: i128) -> i128 {
    let shares_total = total_shares(env);
    if shares_total == 0 {
        return PRICE_ONE;
    }
    assets * PRICE_ONE / shares_total
}

fn usdc(env: &Env) -> token::Client<'_> {
    let address: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    token::Client::new(env, &address)
}

pub(crate) fn capacity(env: &Env) -> i128 {
    total_deposits(env) - reserved(env)
}

// ---- appelé par hedge.rs (interface Paiement → Vault) ----

/// Bloque `amount` USDC de capacité pour une nouvelle couverture. Panique si capacité insuffisante.
/// Erreurs : `InvalidAmount` si `amount <= 0`, `InsufficientCapacity` si `amount > capacity()`.
pub(crate) fn reserve(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    if amount > capacity(env) {
        panic_with_error!(env, VaultError::InsufficientCapacity);
    }
    set_reserved(env, reserved(env) + amount);
}

/// Libère `amount` de capacité réservée (couverture réglée).
/// Erreurs : `InvalidAmount` si `amount <= 0`, `ReleaseExceedsReserved` si `amount > Reserved`.
pub(crate) fn release(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let current = reserved(env);
    if amount > current {
        panic_with_error!(env, VaultError::ReleaseExceedsReserved);
    }
    set_reserved(env, current - amount);
}

/// Le pool encaisse un gain (la PME a perdu sur sa marge) : `TotalDeposits += amount`.
/// Les USDC sont déjà dans le contrat (ils viennent de la marge).
/// `amount == 0` est accepté (P&L nul) ; `InvalidAmount` si négatif.
pub(crate) fn book_gain(env: &Env, amount: i128) {
    if amount < 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    if amount == 0 {
        return;
    }
    set_total_deposits(env, total_deposits(env) + amount);
}

/// Le pool paie une perte (la PME est protégée) : `TotalDeposits -= amount`.
/// Le transfert vers la PME est fait par hedge.rs avec la restitution de la marge.
/// `amount == 0` est accepté (P&L nul) ; `InvalidAmount` si négatif,
/// `LossExceedsCapital` si le capital des LPs deviendrait négatif.
pub(crate) fn book_loss(env: &Env, amount: i128) {
    if amount < 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    if amount == 0 {
        return;
    }
    let total = total_deposits(env);
    if amount > total {
        panic_with_error!(env, VaultError::LossExceedsCapital);
    }
    set_total_deposits(env, total - amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pool, PoolClient};
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
        fn client(&self) -> PoolClient<'_> {
            PoolClient::new(&self.env, &self.pool)
        }

        fn total_deposits(&self) -> i128 {
            self.env.as_contract(&self.pool, || total_deposits(&self.env))
        }

        fn reserved(&self) -> i128 {
            self.env.as_contract(&self.pool, || reserved(&self.env))
        }

        fn total_shares(&self) -> i128 {
            self.env.as_contract(&self.pool, || total_shares(&self.env))
        }

        fn usdc_balance(&self, who: &Address) -> i128 {
            token::Client::new(&self.env, &self.usdc).balance(who)
        }

        fn mint(&self, to: &Address, amount: i128) {
            token::StellarAssetClient::new(&self.env, &self.usdc).mint(to, &amount);
        }

        /// Nouveau LP : reçoit `amount` USDC puis les dépose via `deposit_lp`.
        fn new_lp(&self, amount: i128) -> Address {
            let lp = Address::generate(&self.env);
            self.mint(&lp, amount);
            self.client().deposit_lp(&lp, &amount);
            lp
        }

        /// Marge : la PME envoie ses USDC au contrat et le vault réserve (comme hedge::open).
        fn open_margin(&self, sme: &Address, margin: i128) {
            self.mint(sme, margin);
            token::Client::new(&self.env, &self.usdc).transfer(sme, &self.pool, &margin);
            self.vault(|env| reserve(env, margin));
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

    // ---- Phase 1 : garde-fous de reserve / release / book_* ----

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn reserve_over_capacity_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 1_000 * USDC + 1));
    }

    #[test]
    fn reserve_up_to_capacity_then_capacity_is_zero() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 1_000 * USDC));
        assert_eq!(s.vault(capacity), 0);
        assert_eq!(s.reserved(), 1_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn second_reserve_over_remaining_capacity_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 600 * USDC));
        s.vault(|env| reserve(env, 600 * USDC));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn reserve_zero_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 0));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn reserve_negative_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #102)")]
    fn release_over_reserved_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| release(env, 100 * USDC + 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn release_zero_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| release(env, 0));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn release_negative_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
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
        s.new_lp(1_000 * USDC);
        s.vault(|env| book_loss(env, -1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #103)")]
    fn book_loss_over_capital_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| book_loss(env, 1_000 * USDC + 1));
    }

    #[test]
    fn book_zero_is_a_no_op() {
        // Spot à l'échéance = taux bloqué : P&L nul, le règlement ne doit pas échouer.
        let s = setup();
        s.new_lp(1_000 * USDC);
        s.vault(|env| book_gain(env, 0));
        s.vault(|env| book_loss(env, 0));
        assert_eq!(s.total_deposits(), 1_000 * USDC);
    }

    // ---- Phase 1 : cas types, LP 100 000 USDC, Lucía notionnel ≈ 55 000 USDC, plafond 10 % ----

    const LP_CAPITAL: i128 = 100_000 * USDC;
    const MARGIN: i128 = 5_500 * USDC;

    /// Départ commun : dépôt LP puis ouverture de la couverture de Lucía.
    fn open_lucia() -> (Setup, Address) {
        let s = setup();
        s.assert_invariant(0);
        s.new_lp(LP_CAPITAL);
        s.assert_invariant(0);

        let lucia = Address::generate(&s.env);
        s.open_margin(&lucia, MARGIN);
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

    // ---- Phase 2 : parts LP ----

    /// Couverture réglée en faveur du pool : la PME perd toute sa marge, gain = marge.
    fn settle_pool_gains_margin(s: &Setup, margin: i128) {
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, margin);
        s.vault(|env| release(env, margin));
        s.vault(|env| book_gain(env, margin));
        s.assert_invariant(0);
    }

    /// Couverture réglée en faveur de la PME : le pool perd `margin`, la PME reçoit 2 × marge.
    fn settle_pool_loses_margin(s: &Setup, margin: i128) {
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, margin);
        s.vault(|env| release(env, margin));
        s.vault(|env| book_loss(env, margin));
        s.pay_sme(&sme, 2 * margin);
        s.assert_invariant(0);
    }

    #[test]
    fn first_deposit_is_one_to_one() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        let c = s.client();

        assert_eq!(c.lp_shares(&lp), 1_000 * USDC);
        assert_eq!(s.total_shares(), 1_000 * USDC);
        assert_eq!(c.total_assets(), 1_000 * USDC);
        assert_eq!(c.share_price(), PRICE_ONE);
        assert_eq!(c.lp_balance(&lp), 1_000 * USDC);
        assert_eq!(s.usdc_balance(&lp), 0);
        assert_eq!(s.usdc_balance(&s.pool), 1_000 * USDC);
        s.assert_invariant(0);
    }

    #[test]
    fn deposit_returns_lp_shares_after_deposit() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        s.mint(&lp, 500 * USDC);
        assert_eq!(s.client().deposit_lp(&lp, &(500 * USDC)), 1_500 * USDC);
        s.assert_invariant(0);
    }

    #[test]
    fn second_lp_gets_same_price() {
        let s = setup();
        let a = s.new_lp(1_000 * USDC);
        let b = s.new_lp(3_000 * USDC);
        let c = s.client();

        assert_eq!(c.lp_shares(&a), 1_000 * USDC);
        assert_eq!(c.lp_shares(&b), 3_000 * USDC);
        assert_eq!(c.share_price(), PRICE_ONE);
        assert_eq!(c.total_assets(), 4_000 * USDC);
        s.assert_invariant(0);
    }

    #[test]
    fn gain_raises_share_price_and_next_deposit_gets_fewer_shares() {
        let s = setup();
        let a = s.new_lp(1_000 * USDC);
        settle_pool_gains_margin(&s, 100 * USDC);
        let c = s.client();

        // 1 100 USDC pour 1 000 parts : 1,1 USDC la part.
        assert_eq!(c.share_price(), 11_000_000);
        assert_eq!(c.lp_balance(&a), 1_100 * USDC);

        let b = s.new_lp(1_100 * USDC);
        assert_eq!(c.lp_shares(&b), 1_000 * USDC);
        assert_eq!(c.share_price(), 11_000_000);
        s.assert_invariant(0);
    }

    #[test]
    fn loss_lowers_share_price() {
        let s = setup();
        let a = s.new_lp(1_000 * USDC);
        settle_pool_loses_margin(&s, 100 * USDC);
        let c = s.client();

        assert_eq!(c.share_price(), 9_000_000);
        assert_eq!(c.lp_balance(&a), 900 * USDC);
        assert_eq!(c.lp_shares(&a), 1_000 * USDC);
    }

    #[test]
    fn deposit_shares_are_rounded_down() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        settle_pool_gains_margin(&s, 100 * USDC);

        // 10 stroops × 1 000 / 1 100 = 9,09 → 9 parts : la fraction reste au pool.
        let lp = Address::generate(&s.env);
        s.mint(&lp, 10);
        assert_eq!(s.client().deposit_lp(&lp, &10), 9);
        s.assert_invariant(0);
    }

    #[test]
    fn withdraw_everything_without_open_hedges() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        let c = s.client();

        assert_eq!(c.withdraw(&lp, &(1_000 * USDC)), 1_000 * USDC);
        assert_eq!(s.usdc_balance(&lp), 1_000 * USDC);
        assert_eq!(c.lp_shares(&lp), 0);
        assert_eq!(s.total_shares(), 0);
        assert_eq!(c.total_assets(), 0);
        s.assert_invariant(0);
    }

    #[test]
    fn withdraw_after_gain_pays_the_gain() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        settle_pool_gains_margin(&s, 100 * USDC);

        assert_eq!(s.client().withdraw(&lp, &(1_000 * USDC)), 1_100 * USDC);
        s.assert_invariant(0);
    }

    #[test]
    fn withdraw_with_open_hedge_uses_exit_price_within_capacity() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 500 * USDC);
        let c = s.client();

        assert_eq!(c.share_price(), PRICE_ONE);
        assert_eq!(c.exit_price(), 5_000_000);
        assert_eq!(c.lp_balance(&lp), 500 * USDC);

        // Tout retirer : seulement le capital libre, jamais plus que capacity().
        let capacity_before = c.capacity();
        let paid = c.withdraw(&lp, &(1_000 * USDC));
        assert_eq!(paid, 500 * USDC);
        assert!(paid <= capacity_before);
        assert_eq!(c.capacity(), 0);
        assert_eq!(c.total_assets(), 500 * USDC);
        s.assert_invariant(500 * USDC);
    }

    #[test]
    fn two_lps_exiting_each_get_their_share_of_free_capital() {
        let s = setup();
        let a = s.new_lp(50 * USDC);
        let b = s.new_lp(50 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 50 * USDC);
        let c = s.client();

        // A sort le premier : 50 parts × (100 − 50) / 100 = 25, pas 50.
        let capacity_before = c.capacity();
        let paid_a = c.withdraw(&a, &(50 * USDC));
        assert_eq!(paid_a, 25 * USDC);
        assert!(paid_a <= capacity_before);
        s.assert_invariant(50 * USDC);

        // B sort ensuite : 50 × (75 − 50) / 50 = 25, la même chose.
        let capacity_before = c.capacity();
        let paid_b = c.withdraw(&b, &(50 * USDC));
        assert_eq!(paid_b, 25 * USDC);
        assert!(paid_b <= capacity_before);
        s.assert_invariant(50 * USDC);

        // Ensemble ils ont retiré exactement le capital libre.
        assert_eq!(c.capacity(), 0);
        assert_eq!(s.usdc_balance(&a), 25 * USDC);
        assert_eq!(s.usdc_balance(&b), 25 * USDC);
    }

    #[test]
    fn exit_discount_stays_with_remaining_lps() {
        let s = setup();
        let a = s.new_lp(50 * USDC);
        let b = s.new_lp(50 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 50 * USDC);
        let c = s.client();

        assert_eq!(c.withdraw(&a, &(50 * USDC)), 25 * USDC);

        // La couverture se termine sans P&L : la marge est rendue, B garde la décote de A.
        s.vault(|env| release(env, 50 * USDC));
        s.pay_sme(&sme, 50 * USDC);
        s.assert_invariant(0);
        assert_eq!(c.share_price(), 15_000_000);
        assert_eq!(c.lp_balance(&b), 75 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #106)")]
    fn withdraw_more_shares_than_owned_panics() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        s.new_lp(1_000 * USDC);
        s.client().withdraw(&lp, &(1_000 * USDC + 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn withdraw_zero_shares_panics() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        s.client().withdraw(&lp, &0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #107)")]
    fn withdraw_refused_when_all_capital_is_reserved() {
        let s = setup();
        let lp = s.new_lp(100 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 100 * USDC);
        // Prix de sortie nul : brûler des parts contre 0 USDC est refusé.
        s.client().withdraw(&lp, &(10 * USDC));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn deposit_zero_panics() {
        let s = setup();
        let lp = Address::generate(&s.env);
        s.client().deposit_lp(&lp, &0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #105)")]
    fn deposit_too_small_panics() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        settle_pool_gains_margin(&s, 100 * USDC);
        // 1 stroop × 1 000 / 1 100 = 0 part.
        let lp = Address::generate(&s.env);
        s.mint(&lp, 1);
        s.client().deposit_lp(&lp, &1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #104)")]
    fn deposit_refused_when_pool_depleted() {
        let s = setup();
        s.new_lp(100 * USDC);
        // Une perte égale à tout le capital : il reste des parts qui ne valent plus rien.
        settle_pool_loses_margin(&s, 100 * USDC);
        assert_eq!(s.total_deposits(), 0);
        s.new_lp(100 * USDC);
    }
}
