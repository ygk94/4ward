// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
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
//!
//! Exposition par devise : `open_position` / `close_position` (appelés par hedge.rs) réservent
//! la marge et tiennent, par devise, les volumes acheteur / vendeur (USDC au taux bloqué) et
//! les marges de chaque côté. Limite par devise (admin) : Σ marges de la devise ≤ x % du capital.
//! C'est la somme BRUTE des marges : sans netting, c'est la seule borne sûre de la perte
//! maximale d'une devise (deux couvertures de sens opposé, bloquées à des dates différentes,
//! peuvent gagner toutes les deux contre le pool).
//!
//! Tranches : `TotalDeposits` = capital senior + capital junior.
//! - Senior : parts dont le prix progresse au taux fixe `SENIOR_RATE_BPS` (intérêts simples entre
//!   deux opérations senior, capitalisés à chaque `deposit_senior` / `withdraw_senior` dans
//!   `SeniorPrice` / `SeniorUpdatedAt`). Valeur senior = parts × prix courant, plafonnée à
//!   `TotalDeposits`.
//! - Junior : le reste, `TotalDeposits − valeur senior`. Les parts LP historiques (`deposit_lp`,
//!   `withdraw`) sont les parts junior : prix plein, prix de sortie et capacité se calculent sur
//!   la valeur junior. `book_gain` / `book_loss` font bouger `TotalDeposits`, donc le junior
//!   absorbe gains et pertes en premier, sans code de cascade.
//! - Le rendement senior est payé par le junior : tant que le capital libre n'est pas placé,
//!   il sort du capital junior.
//! - Ratio minimum : tant qu'il y a des parts senior, junior ≥ `MIN_JUNIOR_BPS` du capital ;
//!   un dépôt senior ou un retrait junior qui passerait dessous est refusé (contrôlé à ces
//!   opérations seulement : les intérêts senior courus peuvent faire baisser le junior ensuite).
//!
//! Netting : `basket_max_loss` calcule la perte maximale exacte d'un panier de couvertures,
//! mais n'est PAS branché sur la réserve. Conditions pour le brancher :
//! 1. fixing commun : déjà fait (`settle` lit le cours à l'échéance via `pricing::fixing`) ;
//!    reste à aligner les échéances (point 2) ;
//! 2. échéances arrondies à des dates de fixing (aujourd'hui `now + duration` à la seconde :
//!    deux couvertures n'ont jamais la même échéance, les paniers sont de taille 1) ;
//! 3. règlement par panier (toutes les jambes dans la même transaction) ou comptabilisation
//!    de ce que doit le vendeur au fixing : sinon, si le perdant du pool est réglé en premier,
//!    `book_loss` passe avant le gain de la jambe opposée et le capital tombe sous la réserve.
use crate::{DataKey, Hedge, Side, BPS, PRICE_DECIMALS};
use soroban_sdk::{
    contracterror, contractevent, contracttype, panic_with_error, token, Address, Env, IntoVal, Symbol,
    TryFromVal, Val,
};

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
    /// Ouverture refusée : la limite de la devise (Σ marges ≤ x % du capital) serait dépassée.
    CurrencyLimitExceeded = 108,
    /// `close_position` d'une couverture qui n'a pas été ouverte par `open_position`.
    UnknownPosition = 109,
    /// Débordement d'un calcul (montant démesuré).
    Overflow = 110,
    /// Dépôt senior ou retrait junior refusé : le junior passerait sous `MIN_JUNIOR_BPS` du capital.
    JuniorRatioTooLow = 111,
    /// Retrait de plus de parts senior que le LP n'en détient.
    InsufficientSeniorShares = 112,
}

/// Clés de stockage propres au vault.
/// `TotalDeposits` et `Reserved` restent dans `DataKey` (posées par le constructeur).
#[contracttype]
pub enum VaultKey {
    /// Parts d'un LP (persistent).
    Shares(Address),
    /// Total des parts émises (instance).
    TotalShares,
    /// Exposition d'une devise (persistent).
    Exposure(Symbol),
    /// Limite d'une devise en bps du capital (persistent), absente = sans limite.
    CurrencyLimit(Symbol),
    /// Parts senior d'un LP (persistent).
    SeniorShares(Address),
    /// Total des parts senior émises (instance).
    SeniorTotalShares,
    /// Prix d'une part senior à `SeniorUpdatedAt`, 18 décimales (instance).
    SeniorPrice,
    /// Horodatage (secondes) de la dernière capitalisation du prix senior (instance).
    SeniorUpdatedAt,
}

/// Valeur des deux tranches en USDC (7 décimales). senior + junior = TotalDeposits.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrancheValues {
    /// Parts senior × prix courant, plafonné à TotalDeposits.
    pub senior: i128,
    /// TotalDeposits − senior : absorbe gains et pertes en premier.
    pub junior: i128,
    /// Prix courant d'une part senior, 7 décimales (1 USDC au premier dépôt).
    pub senior_price: i128,
}

/// Exposition du pool sur une devise, couvertures ouvertes seulement (USDC, 7 décimales).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exposure {
    /// Σ notionnels des acheteurs de la devise, en USDC au taux bloqué.
    pub buy_notional: i128,
    /// Σ notionnels des vendeurs de la devise, en USDC au taux bloqué.
    pub sell_notional: i128,
    /// buy_notional − sell_notional (> 0 : le pool perd si la devise monte).
    pub net_notional: i128,
    pub buy_margin: i128,
    pub sell_margin: i128,
}

/// Un LP a déposé des USDC. `shares` = parts émises par ce dépôt.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpDeposited {
    #[topic]
    pub lp: Address,
    pub amount: i128,
    pub shares: i128,
}

/// Un LP a brûlé `shares` parts contre `usdc` (prix de sortie).
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpWithdrew {
    #[topic]
    pub lp: Address,
    pub shares: i128,
    pub usdc: i128,
}

/// Couverture ouverte : marge réservée sur la devise.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionReserved {
    #[topic]
    pub currency: Symbol,
    pub side: Side,
    pub margin: i128,
}

/// Couverture clôturée : marge libérée sur la devise.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionReleased {
    #[topic]
    pub currency: Symbol,
    pub side: Side,
    pub margin: i128,
}

/// Un LP senior a déposé des USDC. `shares` = parts senior émises par ce dépôt.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeniorDeposited {
    #[topic]
    pub lp: Address,
    pub amount: i128,
    pub shares: i128,
}

/// Un LP senior a brûlé `shares` parts senior contre `usdc`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeniorWithdrew {
    #[topic]
    pub lp: Address,
    pub shares: i128,
    pub usdc: i128,
}

/// L'admin a changé la limite d'une devise.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrencyLimitSet {
    #[topic]
    pub currency: Symbol,
    pub limit_bps: u32,
}

/// 1 part = 1 USDC au premier dépôt : prix exprimé en 7 décimales.
const PRICE_ONE: i128 = 10_000_000;

/// Durée de vie du stockage, en ledgers (~5 s) : prolongée à 120 jours dès qu'elle passe sous
/// 30 jours (mêmes valeurs que les couvertures dans hedge.rs).
const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const TTL_EXTEND_TO: u32 = 120 * DAY_IN_LEDGERS;

/// Taux fixe du senior, en bps par an (intérêts simples entre deux opérations senior).
pub(crate) const SENIOR_RATE_BPS: i128 = 450;
/// Part minimale du junior dans le capital tant qu'il y a des parts senior (2 000 = 20 %).
pub(crate) const MIN_JUNIOR_BPS: i128 = 2_000;
/// Prix senior en 18 décimales : à 7 décimales, l'intérêt de quelques secondes s'arrondirait à 0.
const SENIOR_PRICE_ONE: i128 = 1_000_000_000_000_000_000;
const SECS_PER_YEAR: i128 = 31_536_000;

/// `a × b / c` arrondi vers le bas, `Overflow` (110) au lieu d'une panique sans code.
pub(crate) fn mul_div(env: &Env, a: i128, b: i128, c: i128) -> i128 {
    a.checked_mul(b)
        .and_then(|x| x.checked_div_euclid(c))
        .unwrap_or_else(|| panic_with_error!(env, VaultError::Overflow))
}

fn add(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_add(b).unwrap_or_else(|| panic_with_error!(env, VaultError::Overflow))
}

/// Notionnel en USDC au taux bloqué (14 décimales) : comme pricing::to_usdc, mais protégé.
fn notional_usdc(env: &Env, hedge: &Hedge) -> i128 {
    mul_div(env, hedge.notional, hedge.locked_rate, 10i128.pow(PRICE_DECIMALS))
}

// ---- stockage ----

/// Prolonge l'instance (compteurs TotalDeposits, Reserved, TotalShares, et le contrat lui-même).
fn bump_instance(env: &Env) {
    env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Lecture persistent qui prolonge l'entrée si elle existe (extend_ttl panique sur une clé absente).
fn read_persistent<V: TryFromVal<Env, Val>>(env: &Env, key: &VaultKey) -> Option<V> {
    let value = env.storage().persistent().get(key);
    if value.is_some() {
        env.storage().persistent().extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    value
}

fn write_persistent<V: IntoVal<Env, Val>>(env: &Env, key: &VaultKey, value: &V) {
    env.storage().persistent().set(key, value);
    env.storage().persistent().extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
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
        write_persistent(env, &key, &value);
    }
}

/// Capital libre, jamais négatif. La capacité peut devenir négative quand les intérêts senior
/// courus font passer le junior sous Reserved (aucun retrait junior alors), ou par un mauvais
/// usage (`book_loss` sans `release`) ; on ne verse ni n'affiche alors rien.
fn free_capital(env: &Env) -> i128 {
    capacity(env).max(0)
}

// ---- tranches ----

fn senior_total_shares(env: &Env) -> i128 {
    env.storage().instance().get(&VaultKey::SeniorTotalShares).unwrap_or(0)
}

fn set_senior_total_shares(env: &Env, value: i128) {
    env.storage().instance().set(&VaultKey::SeniorTotalShares, &value);
}

fn senior_shares(env: &Env, lp: &Address) -> i128 {
    read_persistent(env, &VaultKey::SeniorShares(lp.clone())).unwrap_or(0)
}

fn set_senior_shares(env: &Env, lp: &Address, value: i128) {
    let key = VaultKey::SeniorShares(lp.clone());
    if value == 0 {
        env.storage().persistent().remove(&key);
    } else {
        write_persistent(env, &key, &value);
    }
}

/// Prix courant d'une part senior (18 décimales) : prix stocké + intérêts simples depuis
/// `SeniorUpdatedAt`. 1 au premier dépôt.
fn senior_price_now(env: &Env) -> i128 {
    let s = env.storage().instance();
    let stored: i128 = s.get(&VaultKey::SeniorPrice).unwrap_or(SENIOR_PRICE_ONE);
    let updated_at: u64 = s.get(&VaultKey::SeniorUpdatedAt).unwrap_or(env.ledger().timestamp());
    let elapsed = env.ledger().timestamp().saturating_sub(updated_at) as i128;
    let rate_time = SENIOR_RATE_BPS.checked_mul(elapsed).unwrap_or_else(|| panic_with_error!(env, VaultError::Overflow));
    add(env, stored, mul_div(env, stored, rate_time, BPS * SECS_PER_YEAR))
}

/// Capitalise les intérêts : à appeler avant toute opération senior, pour que chaque dépôt
/// et chaque retrait se fasse au prix du moment (et non depuis le premier dépôt).
fn accrue_senior(env: &Env) -> i128 {
    let price = senior_price_now(env);
    let s = env.storage().instance();
    s.set(&VaultKey::SeniorPrice, &price);
    s.set(&VaultKey::SeniorUpdatedAt, &env.ledger().timestamp());
    price
}

/// Valeur senior = parts × prix courant, plafonnée au capital (le junior ne descend pas sous 0).
fn senior_value(env: &Env) -> i128 {
    let promised = mul_div(env, senior_total_shares(env), senior_price_now(env), SENIOR_PRICE_ONE);
    promised.min(total_deposits(env).max(0))
}

/// Valeur junior = capital − valeur senior, ≥ 0.
fn junior_value(env: &Env) -> i128 {
    (total_deposits(env) - senior_value(env)).max(0)
}

/// Refuse un état où le junior serait sous `MIN_JUNIOR_BPS` du capital.
/// À appeler seulement quand il y a (ou va y avoir) des parts senior.
fn check_junior_ratio(env: &Env, junior: i128, total: i128) {
    // junior / total ≥ MIN / BPS, en produits croisés : pas d'arrondi en défaveur du senior.
    let lhs = junior.checked_mul(BPS).unwrap_or_else(|| panic_with_error!(env, VaultError::Overflow));
    let rhs = total.checked_mul(MIN_JUNIOR_BPS).unwrap_or_else(|| panic_with_error!(env, VaultError::Overflow));
    if lhs < rhs {
        panic_with_error!(env, VaultError::JuniorRatioTooLow);
    }
}

// ---- appelé par lib.rs (points d'entrée publics) ----

/// Dépose `amount` USDC contre des parts junior au prix plein (valeur junior / parts junior),
/// arrondi vers le bas. Retourne les parts du LP après le dépôt.
/// Limite connue : si toutes les parts junior ont été brûlées avec du capital restant
/// (Reserved laissé par le dernier LP sorti), le déposant suivant reçoit des parts à 1:1 et
/// hérite de ce capital (et des pertes des couvertures encore ouvertes). À terme : refuser
/// dans `withdraw` de brûler la dernière part tant que `reserved > 0`.
/// Erreurs : `InvalidAmount`, `PoolDepleted`, `DepositTooSmall`, `Overflow`.
pub(crate) fn deposit(env: &Env, lp: &Address, amount: i128) -> i128 {
    lp.require_auth();
    bump_instance(env);
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let total = total_deposits(env);
    let junior = junior_value(env);
    let shares_total = total_shares(env);
    let minted = if shares_total == 0 {
        amount
    } else {
        if junior == 0 {
            panic_with_error!(env, VaultError::PoolDepleted);
        }
        mul_div(env, amount, shares_total, junior)
    };
    if minted == 0 {
        panic_with_error!(env, VaultError::DepositTooSmall);
    }
    let shares = add(env, lp_shares(env, lp), minted);
    let new_total_shares = add(env, shares_total, minted);
    let new_total = add(env, total, amount);

    usdc(env).transfer(lp, &env.current_contract_address(), &amount);

    set_lp_shares(env, lp, shares);
    set_total_shares(env, new_total_shares);
    set_total_deposits(env, new_total);
    LpDeposited { lp: lp.clone(), amount, shares: minted }.publish(env);
    shares
}

/// Brûle `shares` parts junior du LP et lui verse leur valeur au prix de sortie
/// ((valeur junior − Reserved) / parts junior), arrondi vers le bas.
/// Le capital réservé n'est pas versé : le montant ne dépasse jamais `capacity()`.
/// Retourne l'USDC versé.
/// Erreurs : `InvalidAmount`, `InsufficientShares`, `WithdrawTooSmall`, `JuniorRatioTooLow`,
/// `Overflow`.
pub(crate) fn withdraw(env: &Env, lp: &Address, shares: i128) -> i128 {
    lp.require_auth();
    bump_instance(env);
    if shares <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let owned = lp_shares(env, lp);
    if shares > owned {
        panic_with_error!(env, VaultError::InsufficientShares);
    }
    // shares ≤ owned ≤ TotalShares, donc TotalShares > 0.
    let shares_total = total_shares(env);
    let amount = mul_div(env, shares, free_capital(env), shares_total);
    if amount <= 0 {
        panic_with_error!(env, VaultError::WithdrawTooSmall);
    }
    let total_after = total_deposits(env) - amount;
    if senior_total_shares(env) > 0 {
        check_junior_ratio(env, junior_value(env) - amount, total_after);
    }

    // État mis à jour avant le transfert sortant.
    set_lp_shares(env, lp, owned - shares);
    set_total_shares(env, shares_total - shares);
    set_total_deposits(env, total_after);

    usdc(env).transfer(&env.current_contract_address(), lp, &amount);
    LpWithdrew { lp: lp.clone(), shares, usdc: amount }.publish(env);
    amount
}

/// Valeur en USDC des parts du LP au prix de sortie (ce qu'il recevrait en retirant tout).
pub(crate) fn balance(env: &Env, lp: &Address) -> i128 {
    bump_instance(env);
    let shares_total = total_shares(env);
    if shares_total == 0 {
        return 0;
    }
    mul_div(env, lp_shares(env, lp), free_capital(env), shares_total)
}

pub(crate) fn lp_shares(env: &Env, lp: &Address) -> i128 {
    bump_instance(env);
    read_persistent(env, &VaultKey::Shares(lp.clone())).unwrap_or(0)
}

/// Prix plein d'une part junior : valeur junior / TotalShares (7 décimales), 1 USDC si aucune part.
/// Sans senior, valeur junior = TotalDeposits.
pub(crate) fn share_price(env: &Env) -> i128 {
    bump_instance(env);
    price(env, junior_value(env))
}

/// Prix de sortie d'une part junior : (valeur junior − Reserved) / TotalShares (7 décimales), ≥ 0.
pub(crate) fn exit_price(env: &Env) -> i128 {
    bump_instance(env);
    price(env, free_capital(env))
}

pub(crate) fn total_assets(env: &Env) -> i128 {
    bump_instance(env);
    total_deposits(env)
}

fn price(env: &Env, assets: i128) -> i128 {
    let shares_total = total_shares(env);
    if shares_total == 0 {
        return PRICE_ONE;
    }
    mul_div(env, assets, PRICE_ONE, shares_total)
}

fn usdc(env: &Env) -> token::Client<'_> {
    let address: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    token::Client::new(env, &address)
}

/// Valeur junior − Reserved : seul le junior porte les couvertures. Sans senior,
/// TotalDeposits − Reserved. Peut devenir négatif quand les intérêts senior courus font passer
/// le junior sous Reserved (aucun retrait junior alors), voir `free_capital`.
pub(crate) fn capacity(env: &Env) -> i128 {
    bump_instance(env);
    junior_value(env) - reserved(env)
}

/// Dépose `amount` USDC contre des parts senior au prix courant (capitalisé à cet instant),
/// arrondi vers le bas. Retourne les parts senior du LP après le dépôt.
/// Erreurs : `InvalidAmount`, `DepositTooSmall`, `JuniorRatioTooLow`, `Overflow`.
pub(crate) fn deposit_senior(env: &Env, lp: &Address, amount: i128) -> i128 {
    lp.require_auth();
    bump_instance(env);
    if amount <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let price = accrue_senior(env);
    let minted = mul_div(env, amount, SENIOR_PRICE_ONE, price);
    if minted == 0 {
        panic_with_error!(env, VaultError::DepositTooSmall);
    }
    // Le dépôt ne change pas la valeur junior : seul le capital total grossit.
    let new_total = add(env, total_deposits(env), amount);
    let shares = add(env, senior_shares(env, lp), minted);
    let new_senior_total = add(env, senior_total_shares(env), minted);
    check_junior_ratio(env, junior_value(env), new_total);

    usdc(env).transfer(lp, &env.current_contract_address(), &amount);

    set_senior_shares(env, lp, shares);
    set_senior_total_shares(env, new_senior_total);
    set_total_deposits(env, new_total);
    SeniorDeposited { lp: lp.clone(), amount, shares: minted }.publish(env);
    shares
}

/// Brûle `shares` parts senior du LP et lui verse leur valeur (valeur senior × parts / parts
/// senior totales), arrondi vers le bas. Le retrait senior ne touche pas la valeur junior,
/// donc ni la capacité ni le ratio ne bougent, SAUF si le junior est déjà passé sous Reserved
/// (intérêts senior courus) : le retrait peut alors laisser TotalDeposits < Reserved, et un
/// `settle` perdant pour le pool échouerait en `LossExceedsCapital` tant qu'un LP ne redépose
/// pas. Limite connue, à corriger après le hackathon (refus si TotalDeposits − amount < Reserved,
/// nouveau code 113). Retourne l'USDC versé.
/// Erreurs : `InvalidAmount`, `InsufficientSeniorShares`, `WithdrawTooSmall`, `Overflow`.
pub(crate) fn withdraw_senior(env: &Env, lp: &Address, shares: i128) -> i128 {
    lp.require_auth();
    bump_instance(env);
    if shares <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    let owned = senior_shares(env, lp);
    if shares > owned {
        panic_with_error!(env, VaultError::InsufficientSeniorShares);
    }
    accrue_senior(env);
    // shares ≤ owned ≤ SeniorTotalShares, donc SeniorTotalShares > 0.
    let senior_total = senior_total_shares(env);
    let amount = mul_div(env, shares, senior_value(env), senior_total);
    if amount <= 0 {
        panic_with_error!(env, VaultError::WithdrawTooSmall);
    }

    // État mis à jour avant le transfert sortant.
    set_senior_shares(env, lp, owned - shares);
    set_senior_total_shares(env, senior_total - shares);
    set_total_deposits(env, total_deposits(env) - amount);

    usdc(env).transfer(&env.current_contract_address(), lp, &amount);
    SeniorWithdrew { lp: lp.clone(), shares, usdc: amount }.publish(env);
    amount
}

/// Valeur en USDC des parts senior du LP au prix courant.
pub(crate) fn senior_balance(env: &Env, lp: &Address) -> i128 {
    bump_instance(env);
    let senior_total = senior_total_shares(env);
    if senior_total == 0 {
        return 0;
    }
    mul_div(env, senior_shares(env, lp), senior_value(env), senior_total)
}

/// Valeur des deux tranches et prix courant d'une part senior (7 décimales).
pub(crate) fn tranche_values(env: &Env) -> TrancheValues {
    bump_instance(env);
    let senior = senior_value(env);
    TrancheValues {
        senior,
        junior: (total_deposits(env) - senior).max(0),
        senior_price: mul_div(env, senior_price_now(env), PRICE_ONE, SENIOR_PRICE_ONE),
    }
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
    bump_instance(env);
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
    bump_instance(env);
    if amount < 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    if amount == 0 {
        return;
    }
    set_total_deposits(env, add(env, total_deposits(env), amount));
}

/// Le pool paie une perte (la PME est protégée) : `TotalDeposits -= amount`.
/// Le transfert vers la PME est fait par hedge.rs avec la restitution de la marge.
/// `amount == 0` est accepté (P&L nul) ; `InvalidAmount` si négatif,
/// `LossExceedsCapital` si le capital des LPs deviendrait négatif.
pub(crate) fn book_loss(env: &Env, amount: i128) {
    bump_instance(env);
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

/// Ouverture d'une couverture : réserve la marge, contrôle la limite de la devise et ajoute
/// l'exposition. À appeler par hedge::open une fois la `Hedge` construite.
/// Erreurs : `InvalidAmount`, `InsufficientCapacity`, `CurrencyLimitExceeded`, `Overflow`.
pub(crate) fn open_position(env: &Env, hedge: &Hedge) {
    if hedge.notional <= 0 {
        panic_with_error!(env, VaultError::InvalidAmount);
    }
    reserve(env, hedge.margin);

    let mut e = exposure(env, &hedge.currency);
    let margins_after = add(env, add(env, e.buy_margin, e.sell_margin), hedge.margin);
    if let Some(limit_bps) = currency_limit(env, &hedge.currency) {
        // Plafond arrondi vers le bas : en faveur du pool.
        if margins_after > mul_div(env, total_deposits(env), limit_bps as i128, BPS) {
            panic_with_error!(env, VaultError::CurrencyLimitExceeded);
        }
    }

    // Même calcul à l'ouverture et à la clôture : le compteur retombe à zéro pile.
    let notional_usd = notional_usdc(env, hedge);
    match hedge.side {
        Side::BuyCurrency => {
            e.buy_notional = add(env, e.buy_notional, notional_usd);
            e.buy_margin += hedge.margin;
        }
        Side::SellCurrency => {
            e.sell_notional = add(env, e.sell_notional, notional_usd);
            e.sell_margin += hedge.margin;
        }
    }
    set_exposure(env, &hedge.currency, e);
    PositionReserved { currency: hedge.currency.clone(), side: hedge.side, margin: hedge.margin }.publish(env);
}

/// Clôture d'une couverture : libère la marge et retire l'exposition ajoutée à l'ouverture.
/// À appeler par hedge::settle à la place de `release`, avant `book_gain` / `book_loss`.
/// Erreurs : `UnknownPosition`.
pub(crate) fn close_position(env: &Env, hedge: &Hedge) {
    let mut e = exposure(env, &hedge.currency);
    let notional_usd = notional_usdc(env, hedge);
    let (notional, margin) = match hedge.side {
        Side::BuyCurrency => (&mut e.buy_notional, &mut e.buy_margin),
        Side::SellCurrency => (&mut e.sell_notional, &mut e.sell_margin),
    };
    if *notional < notional_usd || *margin < hedge.margin {
        panic_with_error!(env, VaultError::UnknownPosition);
    }
    *notional -= notional_usd;
    *margin -= hedge.margin;
    set_exposure(env, &hedge.currency, e);

    release(env, hedge.margin);
    PositionReleased { currency: hedge.currency.clone(), side: hedge.side, margin: hedge.margin }.publish(env);
}

pub(crate) fn exposure(env: &Env, currency: &Symbol) -> Exposure {
    bump_instance(env);
    read_persistent(env, &VaultKey::Exposure(currency.clone()))
        .unwrap_or(Exposure { buy_notional: 0, sell_notional: 0, net_notional: 0, buy_margin: 0, sell_margin: 0 })
}

/// Recalcule le net et supprime la clé quand la devise n'a plus de couverture ouverte.
fn set_exposure(env: &Env, currency: &Symbol, mut e: Exposure) {
    e.net_notional = e.buy_notional - e.sell_notional;
    let key = VaultKey::Exposure(currency.clone());
    if e.buy_notional == 0 && e.sell_notional == 0 && e.buy_margin == 0 && e.sell_margin == 0 {
        env.storage().persistent().remove(&key);
    } else {
        write_persistent(env, &key, &e);
    }
}

pub(crate) fn currency_limit(env: &Env, currency: &Symbol) -> Option<u32> {
    bump_instance(env);
    read_persistent(env, &VaultKey::CurrencyLimit(currency.clone()))
}

/// Admin : limite de la devise en bps du capital. 0 bloque les nouvelles couvertures,
/// 10 000 ou plus revient à ne pas avoir de limite (la capacité du pool borne déjà la réserve).
pub(crate) fn set_currency_limit(env: &Env, currency: &Symbol, limit_bps: u32) {
    crate::admin(env).require_auth();
    bump_instance(env);
    write_persistent(env, &VaultKey::CurrencyLimit(currency.clone()), &limit_bps);
    CurrencyLimitSet { currency: currency.clone(), limit_bps }.publish(env);
}

// ---- netting : calcul exact, NON branché (voir l'en-tête) ----

/// Une jambe d'un panier (même devise, même échéance, même fixing).
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) struct Leg {
    pub side: Side,
    /// Montant en devise, 7 décimales.
    pub notional: i128,
    /// Taux bloqué, USD par unité, 14 décimales.
    pub locked_rate: i128,
    /// Marge en USDC = perte max de la jambe pour le pool.
    pub margin: i128,
}

/// P&L de l'utilisateur au cours `spot` (> 0 : le pool paie). Même formule que hedge::pnl.
#[allow(dead_code)]
pub(crate) fn leg_pnl(leg: &Leg, spot: i128) -> i128 {
    let diff = match leg.side {
        Side::BuyCurrency => spot - leg.locked_rate,
        Side::SellCurrency => leg.locked_rate - spot,
    };
    let raw = (leg.notional * diff).div_euclid(10i128.pow(PRICE_DECIMALS));
    raw.clamp(-leg.margin, leg.margin)
}

/// Perte maximale du pool sur un panier, pour tous les cours de fixing possibles (≥ 0).
///
/// Chaque P&L est linéaire par morceaux en fonction du cours, avec deux cassures où il atteint
/// ±marge : `locked_rate ± marge / notional`. La somme est donc linéaire entre deux cassures
/// et son maximum est à une cassure, ou aux extrêmes (cours nul, ou au-delà de toutes les
/// cassures où tout est plafonné). On évalue la somme exacte à chaque cassure (±1 pour
/// l'arrondi) et au cours nul. Les arrondis vers le bas de chaque P&L peuvent faire dévier la
/// somme d'un stroop par jambe entre deux cassures : on ajoute ce stroop, en faveur du pool.
/// Valable quels que soient les taux bloqués et les plafonds de chaque jambe.
#[allow(dead_code)]
pub(crate) fn basket_max_loss(legs: &[Leg]) -> i128 {
    if legs.is_empty() {
        return 0;
    }
    let scale = 10i128.pow(PRICE_DECIMALS);
    let sum_at = |spot: i128| -> i128 { legs.iter().map(|l| leg_pnl(l, spot)).sum() };

    let mut worst = sum_at(0);
    for l in legs {
        if l.notional <= 0 {
            continue;
        }
        // Écart de cours qui épuise la marge.
        let width = l.margin * scale / l.notional;
        for breakpoint in [l.locked_rate - width, l.locked_rate + width] {
            for spot in [breakpoint - 1, breakpoint, breakpoint + 1] {
                if spot >= 0 {
                    worst = worst.max(sum_at(spot));
                }
            }
        }
    }
    worst.max(0) + legs.len() as i128
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pricing, Pool, PoolClient};
    use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
    use soroban_sdk::testutils::{Address as _, AuthorizedFunction, Events as _, Ledger as _};
    use soroban_sdk::{symbol_short, token, Event as _};

    /// 1 USDC en 7 décimales.
    const USDC: i128 = 10_000_000;

    struct Setup {
        env: Env,
        admin: Address,
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
        Setup { env, admin, pool, usdc }
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

    // ---- Phase 3 : exposition par devise ----

    /// 1 000 000 MXN (7 décimales).
    const MXN_1M: i128 = 10_000_000_000_000;
    /// 0,055 USD par MXN (14 décimales).
    const RATE_055: i128 = 5_500_000_000_000;

    /// Couverture construite comme hedge::open : marge = to_usdc × plafond, arrondie vers le haut.
    fn make_hedge(s: &Setup, currency: Symbol, side: Side, notional: i128, locked_rate: i128, cap_bps: u32) -> Hedge {
        let margin = (pricing::to_usdc(notional, locked_rate) * cap_bps as i128 + BPS - 1) / BPS;
        Hedge {
            owner: Address::generate(&s.env),
            supplier: Address::generate(&s.env),
            currency,
            side,
            notional,
            locked_rate,
            cap_bps,
            margin,
            maturity: 0,
            settled: false,
        }
    }

    /// La PME envoie sa marge au contrat, puis open_position (ordre de hedge::open).
    fn open_pos(s: &Setup, h: &Hedge) {
        s.mint(&h.owner, h.margin);
        token::Client::new(&s.env, &s.usdc).transfer(&h.owner, &s.pool, &h.margin);
        s.vault(|env| open_position(env, h));
    }

    /// Clôture à P&L nul : close_position puis la marge est rendue (ordre de hedge::settle).
    fn close_pos_flat(s: &Setup, h: &Hedge) {
        s.vault(|env| close_position(env, h));
        s.pay_sme(&h.owner, h.margin);
    }

    fn zero_exposure() -> Exposure {
        Exposure { buy_notional: 0, sell_notional: 0, net_notional: 0, buy_margin: 0, sell_margin: 0 }
    }

    #[test]
    fn exposure_is_empty_by_default() {
        let s = setup();
        assert_eq!(s.client().exposure(&symbol_short!("MXN")), zero_exposure());
    }

    #[test]
    fn open_position_reserves_margin_and_adds_buy_exposure() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let lucia = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        assert_eq!(lucia.margin, MARGIN);

        open_pos(&s, &lucia);
        assert_eq!(s.reserved(), MARGIN);
        assert_eq!(s.client().capacity(), LP_CAPITAL - MARGIN);
        assert_eq!(
            s.client().exposure(&symbol_short!("MXN")),
            Exposure {
                buy_notional: 55_000 * USDC,
                sell_notional: 0,
                net_notional: 55_000 * USDC,
                buy_margin: MARGIN,
                sell_margin: 0,
            }
        );
        s.assert_invariant(MARGIN);
    }

    #[test]
    fn buyer_and_seller_give_net_exposure() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let buyer = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        let seller = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, MXN_1M / 4, RATE_055, 1000);
        open_pos(&s, &buyer);
        open_pos(&s, &seller);

        let e = s.client().exposure(&symbol_short!("MXN"));
        assert_eq!(e.buy_notional, 55_000 * USDC);
        assert_eq!(e.sell_notional, 13_750 * USDC);
        assert_eq!(e.net_notional, 41_250 * USDC);
        assert_eq!(e.sell_margin, 1_375 * USDC);
        // Sans netting, la réserve reste la somme brute des marges.
        assert_eq!(s.reserved(), MARGIN + 1_375 * USDC);
        // Les autres devises ne bougent pas.
        assert_eq!(s.client().exposure(&symbol_short!("EUR")), zero_exposure());
        s.assert_invariant(MARGIN + 1_375 * USDC);
    }

    #[test]
    fn close_position_removes_exposure_exactly() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let buyer = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        // Taux et notionnel « tordus » : le retrait doit quand même retomber à zéro pile.
        let seller = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, 3_333_333_333_337, 5_472_618_000_001, 1250);
        open_pos(&s, &buyer);
        open_pos(&s, &seller);

        close_pos_flat(&s, &buyer);
        let e = s.client().exposure(&symbol_short!("MXN"));
        assert_eq!(e.buy_notional, 0);
        assert_eq!(e.buy_margin, 0);
        assert_eq!(e.sell_margin, seller.margin);
        s.assert_invariant(seller.margin);

        close_pos_flat(&s, &seller);
        assert_eq!(s.client().exposure(&symbol_short!("MXN")), zero_exposure());
        assert_eq!(s.reserved(), 0);
        assert_eq!(s.client().capacity(), LP_CAPITAL);
        s.assert_invariant(0);
    }

    #[test]
    fn positions_match_phase_2_reserve_release() {
        // Même scénario par open_position / close_position et par reserve / release.
        let a = setup();
        a.new_lp(LP_CAPITAL);
        let h = make_hedge(&a, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&a, &h);
        let cap_open_a = a.client().capacity();
        a.vault(|env| close_position(env, &h));

        let b = setup();
        b.new_lp(LP_CAPITAL);
        b.vault(|env| reserve(env, h.margin));
        let cap_open_b = b.client().capacity();
        b.vault(|env| release(env, h.margin));

        assert_eq!(cap_open_a, cap_open_b);
        assert_eq!(a.reserved(), b.reserved());
        assert_eq!(a.client().capacity(), b.client().capacity());
    }

    #[test]
    fn case_a_through_positions_keeps_invariant() {
        // Cas A de la phase 1, branché comme le fera hedge.rs.
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let lucia = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &lucia);
        s.assert_invariant(MARGIN);

        s.vault(|env| close_position(env, &lucia));
        s.vault(|env| book_loss(env, MARGIN));
        s.pay_sme(&lucia.owner, 2 * MARGIN);
        s.assert_invariant(0);
        assert_eq!(s.total_deposits(), 94_500 * USDC);
        assert_eq!(s.client().exposure(&symbol_short!("MXN")), zero_exposure());
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #109)")]
    fn close_position_without_open_panics() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        // Ouverte par l'ancien chemin (reserve seul) : pas d'exposition enregistrée.
        s.vault(|env| reserve(env, h.margin));
        s.vault(|env| close_position(env, &h));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #109)")]
    fn close_position_twice_panics() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        let other = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &h);
        open_pos(&s, &other);
        s.vault(|env| close_position(env, &h));
        s.vault(|env| close_position(env, &h));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn open_position_over_capacity_panics() {
        let s = setup();
        s.new_lp(5_000 * USDC);
        let h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &h);
    }

    // ---- Phase 3 : limite par devise ----

    #[test]
    fn currency_limit_is_none_by_default_and_readable() {
        let s = setup();
        let c = s.client();
        assert_eq!(c.currency_limit(&symbol_short!("MXN")), None);
        c.set_currency_limit(&symbol_short!("MXN"), &1000);
        assert_eq!(c.currency_limit(&symbol_short!("MXN")), Some(1000));
        assert_eq!(c.currency_limit(&symbol_short!("EUR")), None);
    }

    #[test]
    fn set_currency_limit_requires_admin_auth() {
        let s = setup();
        s.client().set_currency_limit(&symbol_short!("MXN"), &1000);
        let auths = s.env.auths();
        assert_eq!(auths.len(), 1);
        assert_eq!(auths[0].0, s.admin);
        match &auths[0].1.function {
            AuthorizedFunction::Contract((contract, name, _)) => {
                assert_eq!(contract, &s.pool);
                assert_eq!(name, &Symbol::new(&s.env, "set_currency_limit"));
            }
            _ => panic!("autorisation inattendue"),
        }
    }

    #[test]
    fn open_up_to_currency_limit_is_accepted() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        // 11 % de 100 000 = 11 000 : deux marges de 5 500 passent pile.
        s.client().set_currency_limit(&symbol_short!("MXN"), &1100);
        let a = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        let b = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &a);
        open_pos(&s, &b);
        assert_eq!(s.client().exposure(&symbol_short!("MXN")).buy_margin + s.client().exposure(&symbol_short!("MXN")).sell_margin, 11_000 * USDC);
        s.assert_invariant(11_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #108)")]
    fn open_over_currency_limit_panics() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        // 10 % de 100 000 = 10 000 : la deuxième marge de 5 500 dépasse, même en sens opposé
        // (limite sur la somme brute des marges).
        s.client().set_currency_limit(&symbol_short!("MXN"), &1000);
        let a = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        let b = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &a);
        open_pos(&s, &b);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #108)")]
    fn zero_currency_limit_blocks_new_hedges() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        s.client().set_currency_limit(&symbol_short!("ARS"), &0);
        let h = make_hedge(&s, symbol_short!("ARS"), Side::BuyCurrency, MXN_1M, 100_000_000_000, 1000);
        open_pos(&s, &h);
    }

    #[test]
    fn currency_limit_does_not_affect_other_currencies() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        s.client().set_currency_limit(&symbol_short!("ARS"), &0);
        let h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &h);
        s.assert_invariant(MARGIN);
    }

    #[test]
    fn closing_frees_room_under_currency_limit() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        s.client().set_currency_limit(&symbol_short!("MXN"), &1000);
        let a = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        let b = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &a);
        close_pos_flat(&s, &a);
        open_pos(&s, &b);
        s.assert_invariant(MARGIN);
    }

    // ---- Phase 3 : netting, calcul exact non branché ----

    /// Prix de 1 USD par unité (14 décimales), pour les exemples « à la main ».
    const ONE: i128 = 100_000_000_000_000;

    fn leg(side: Side, notional: i128, locked_rate: i128, cap_bps: u32) -> Leg {
        let margin = (pricing::to_usdc(notional, locked_rate) * cap_bps as i128 + BPS - 1) / BPS;
        Leg { side, notional, locked_rate, margin }
    }

    fn basket_pnl(legs: &[Leg], spot: i128) -> i128 {
        legs.iter().map(|l| leg_pnl(l, spot)).sum()
    }

    /// Propriété : sur la grille −30 % … +30 % par pas de 0,5 % autour de `reference`,
    /// la somme des P&L ne dépasse jamais la réserve du panier. Retourne le max de la grille.
    fn assert_reserve_covers_grid(legs: &[Leg], reference: i128) -> i128 {
        let reserve = basket_max_loss(legs);
        let mut worst = i128::MIN;
        for k in -60i128..=60 {
            let spot = reference * (1000 + 5 * k) / 1000;
            let loss = basket_pnl(legs, spot);
            assert!(loss <= reserve, "cours {spot} : perte {loss} > réserve {reserve}");
            worst = worst.max(loss);
        }
        worst
    }

    /// La réserve vaut `expected` à l'arrondi de sécurité près (1 stroop par jambe, en faveur du pool).
    fn assert_reserve_is(legs: &[Leg], expected: i128) {
        let reserve = basket_max_loss(legs);
        assert!(
            reserve >= expected && reserve <= expected + legs.len() as i128,
            "réserve {reserve}, attendu {expected}"
        );
    }

    #[test]
    fn empty_basket_needs_no_reserve() {
        assert_eq!(basket_max_loss(&[]), 0);
    }

    #[test]
    fn single_leg_reserve_is_its_margin() {
        let buyer = leg(Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        assert_reserve_is(&[buyer], MARGIN);
        let seller = leg(Side::SellCurrency, MXN_1M, RATE_055, 1000);
        assert_reserve_is(&[seller], MARGIN);
    }

    #[test]
    fn opposite_legs_same_cap_offset() {
        // Acheteur et vendeur du même montant, même taux, même plafond : risque nul.
        let legs = [
            leg(Side::BuyCurrency, MXN_1M, RATE_055, 1000),
            leg(Side::SellCurrency, MXN_1M, RATE_055, 1000),
        ];
        assert_reserve_is(&legs, 0);
        assert_reserve_covers_grid(&legs, RATE_055);
    }

    #[test]
    fn different_caps_counterexample_no_abusive_offset() {
        // Acheteur 100 USD plafond 5 % (marge 5) ; vendeur 10 USD plafond 20 % (marge 2).
        // Le net des marges dirait 3 ; à +5 %, le pool paie 5 et ne récupère que 0,5 : 4,5.
        let legs = [
            leg(Side::BuyCurrency, 100 * USDC, ONE, 500),
            leg(Side::SellCurrency, 10 * USDC, ONE, 2000),
        ];
        assert_eq!(legs[0].margin, 5 * USDC);
        assert_eq!(legs[1].margin, 2 * USDC);
        assert_reserve_is(&legs, 45 * USDC / 10);
        let worst = assert_reserve_covers_grid(&legs, ONE);
        assert_eq!(worst, 45 * USDC / 10);
    }

    #[test]
    fn locked_at_different_dates_both_can_win() {
        // Acheteur bloqué à 0,050, vendeur à 0,060 : entre les deux, les deux gagnent contre
        // le pool. La réserve dépasse max(marge acheteur, marge vendeur) : un « net » serait faux.
        let buyer = leg(Side::BuyCurrency, MXN_1M, 5_000_000_000_000, 1000);
        let seller = leg(Side::SellCurrency, MXN_1M, 6_000_000_000_000, 1000);
        let legs = [buyer, seller];
        assert_reserve_is(&legs, 10_000 * USDC);
        assert!(basket_max_loss(&legs) > buyer.margin.max(seller.margin));
        assert_reserve_covers_grid(&legs, RATE_055);
    }

    #[test]
    fn mixed_basket_reserve_covers_every_move_and_nets() {
        // Paniers mixtes : taux bloqués différents (spread de chaque côté), plafond commun.
        let legs = [
            leg(Side::BuyCurrency, MXN_1M, RATE_055 * 1005 / 1000, 1000),
            leg(Side::BuyCurrency, MXN_1M * 3 / 10, RATE_055 * 1002 / 1000, 1000),
            leg(Side::SellCurrency, MXN_1M * 8 / 10, RATE_055 * 995 / 1000, 1000),
            leg(Side::SellCurrency, MXN_1M * 2 / 10, RATE_055 * 990 / 1000, 1000),
        ];
        let gross: i128 = legs.iter().map(|l| l.margin).sum();
        let worst = assert_reserve_covers_grid(&legs, RATE_055);
        let reserve = basket_max_loss(&legs);
        assert!(reserve < gross, "le netting doit réserver moins que la somme brute");
        assert!(reserve >= worst);
    }

    #[test]
    fn pseudo_random_baskets_reserve_covers_grid() {
        // Paniers variés (sens, montants, taux, plafonds différents) tirés d'un générateur fixe.
        let mut seed: u64 = 0x5EED_2026;
        let mut next = |modulo: u64| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % modulo
        };
        for _ in 0..50 {
            let n = 1 + next(6) as usize;
            let mut legs = [leg(Side::BuyCurrency, 1, RATE_055, 1000); 6];
            for l in legs.iter_mut().take(n) {
                let side = if next(2) == 0 { Side::BuyCurrency } else { Side::SellCurrency };
                let notional = (1 + next(2_000_000) as i128) * USDC;
                let rate = RATE_055 * (900 + next(201) as i128) / 1000;
                let cap = [300u32, 500, 1000, 1500, 2000][next(5) as usize];
                *l = leg(side, notional, rate, cap);
            }
            let legs = &legs[..n];
            assert_reserve_covers_grid(legs, RATE_055);
            let gross: i128 = legs.iter().map(|l| l.margin).sum();
            assert!(basket_max_loss(legs) <= gross + n as i128);
        }
    }

    // ---- Phase 5 : durée de vie du stockage (TTL) ----

    fn instance_ttl(s: &Setup) -> u32 {
        s.vault(|env| env.storage().instance().get_ttl())
    }

    fn persistent_ttl(s: &Setup, key: &VaultKey) -> u32 {
        s.vault(|env| env.storage().persistent().get_ttl(key))
    }

    fn advance_days(s: &Setup, days: u32) {
        s.env.ledger().with_mut(|l| l.sequence_number += days * DAY_IN_LEDGERS);
    }

    #[test]
    fn deposit_extends_instance_and_shares_ttl() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        assert_eq!(instance_ttl(&s), TTL_EXTEND_TO);
        assert_eq!(persistent_ttl(&s, &VaultKey::Shares(lp)), TTL_EXTEND_TO);
    }

    #[test]
    fn reading_shares_after_100_days_extends_ttl_again() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        advance_days(&s, 100);
        assert_eq!(persistent_ttl(&s, &VaultKey::Shares(lp.clone())), TTL_EXTEND_TO - 100 * DAY_IN_LEDGERS);

        // Sous le seuil de 30 jours : la lecture prolonge à 120 jours.
        assert_eq!(s.client().lp_shares(&lp), 1_000 * USDC);
        assert_eq!(persistent_ttl(&s, &VaultKey::Shares(lp)), TTL_EXTEND_TO);
        assert_eq!(instance_ttl(&s), TTL_EXTEND_TO);
    }

    #[test]
    fn reading_missing_entries_does_not_panic() {
        // extend_ttl sur une clé absente panique : les lectures doivent l'éviter.
        let s = setup();
        let c = s.client();
        assert_eq!(c.lp_shares(&Address::generate(&s.env)), 0);
        assert_eq!(c.exposure(&symbol_short!("MXN")), zero_exposure());
        assert_eq!(c.currency_limit(&symbol_short!("MXN")), None);
    }

    #[test]
    fn exposure_and_currency_limit_ttl_extended() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        s.client().set_currency_limit(&symbol_short!("MXN"), &5000);
        assert_eq!(persistent_ttl(&s, &VaultKey::CurrencyLimit(symbol_short!("MXN"))), TTL_EXTEND_TO);

        let h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &h);
        assert_eq!(persistent_ttl(&s, &VaultKey::Exposure(symbol_short!("MXN"))), TTL_EXTEND_TO);

        advance_days(&s, 100);
        s.client().exposure(&symbol_short!("MXN"));
        s.client().currency_limit(&symbol_short!("MXN"));
        assert_eq!(persistent_ttl(&s, &VaultKey::Exposure(symbol_short!("MXN"))), TTL_EXTEND_TO);
        assert_eq!(persistent_ttl(&s, &VaultKey::CurrencyLimit(symbol_short!("MXN"))), TTL_EXTEND_TO);
    }

    // ---- Phase 5 : événements ----

    #[test]
    fn deposit_emits_lp_deposited() {
        let s = setup();
        let a = s.new_lp(1_000 * USDC);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [LpDeposited { lp: a, amount: 1_000 * USDC, shares: 1_000 * USDC }.to_xdr(&s.env, &s.pool)]
        );

        // Après un gain, l'événement porte les parts émises par ce dépôt (pas le total du LP).
        settle_pool_gains_margin(&s, 100 * USDC);
        let b = s.new_lp(1_100 * USDC);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [LpDeposited { lp: b, amount: 1_100 * USDC, shares: 1_000 * USDC }.to_xdr(&s.env, &s.pool)]
        );
    }

    #[test]
    fn withdraw_emits_lp_withdrew() {
        let s = setup();
        let lp = s.new_lp(1_000 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 500 * USDC);
        s.client().withdraw(&lp, &(400 * USDC));
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [LpWithdrew { lp, shares: 400 * USDC, usdc: 200 * USDC }.to_xdr(&s.env, &s.pool)]
        );
    }

    #[test]
    fn positions_emit_reserved_and_released() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let h = make_hedge(&s, symbol_short!("MXN"), Side::SellCurrency, MXN_1M, RATE_055, 1000);
        open_pos(&s, &h);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [PositionReserved { currency: symbol_short!("MXN"), side: Side::SellCurrency, margin: MARGIN }
                .to_xdr(&s.env, &s.pool)]
        );
        s.vault(|env| close_position(env, &h));
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [PositionReleased { currency: symbol_short!("MXN"), side: Side::SellCurrency, margin: MARGIN }
                .to_xdr(&s.env, &s.pool)]
        );
    }

    #[test]
    fn set_currency_limit_emits_event() {
        let s = setup();
        s.client().set_currency_limit(&symbol_short!("ARS"), &1000);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [CurrencyLimitSet { currency: symbol_short!("ARS"), limit_bps: 1000 }.to_xdr(&s.env, &s.pool)]
        );
    }

    // ---- Phase 5 : calculs protégés ----

    #[test]
    #[should_panic(expected = "Error(Contract, #110)")]
    fn huge_deposit_returns_overflow_error() {
        let s = setup();
        s.new_lp(1_000 * USDC);
        // montant × TotalShares dépasse i128 : erreur 110 avant tout transfert.
        let lp = Address::generate(&s.env);
        s.client().deposit_lp(&lp, &(i128::MAX / 1_000));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #110)")]
    fn huge_notional_returns_overflow_error() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let mut h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        h.notional = i128::MAX / 2;
        open_pos(&s, &h);
    }

    #[test]
    fn mul_div_rounds_down() {
        let s = setup();
        assert_eq!(s.vault(|env| mul_div(env, 10, 1_000, 1_100)), 9);
        assert_eq!(s.vault(|env| mul_div(env, 7, 3, 2)), 10);
    }

    // ---- Phase 5 : corrections d'audit ----

    /// Mauvais usage (book_loss sans release) : capital sous la réserve, capacité négative.
    fn negative_capacity(s: &Setup) -> Address {
        let lp = s.new_lp(100 * USDC);
        s.vault(|env| reserve(env, 100 * USDC));
        s.vault(|env| book_loss(env, 50 * USDC));
        assert!(s.client().capacity() < 0);
        lp
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #107)")]
    fn withdraw_with_negative_capacity_is_refused() {
        let s = setup();
        let lp = negative_capacity(&s);
        s.client().withdraw(&lp, &(10 * USDC));
    }

    #[test]
    fn reads_never_negative_with_negative_capacity() {
        let s = setup();
        let lp = negative_capacity(&s);
        assert_eq!(s.client().lp_balance(&lp), 0);
        assert_eq!(s.client().exit_price(), 0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn open_position_with_zero_notional_panics() {
        let s = setup();
        s.new_lp(LP_CAPITAL);
        let mut h = make_hedge(&s, symbol_short!("MXN"), Side::BuyCurrency, MXN_1M, RATE_055, 1000);
        h.notional = 0;
        open_pos(&s, &h);
    }

    // ---- Tranches senior / junior ----

    const YEAR: u64 = 31_536_000;

    impl Setup {
        /// Nouveau LP senior : reçoit `amount` USDC puis les dépose via `deposit_senior`.
        fn new_senior(&self, amount: i128) -> Address {
            let lp = Address::generate(&self.env);
            self.mint(&lp, amount);
            self.client().deposit_senior(&lp, &amount);
            lp
        }

        fn advance_secs(&self, secs: u64) {
            self.env.ledger().with_mut(|l| l.timestamp += secs);
        }

        /// Règle comptable des tranches : senior + junior = TotalDeposits, et le junior est
        /// bien la base des prix et de la capacité. Complète `assert_invariant` (solde USDC).
        fn assert_tranches(&self, owed_to_smes: i128) {
            self.assert_invariant(owed_to_smes);
            let t = self.client().tranche_values();
            assert_eq!(t.senior + t.junior, self.total_deposits(), "senior + junior = TotalDeposits");
            assert!(t.senior >= 0 && t.junior >= 0);
            assert_eq!(self.client().capacity(), t.junior - self.reserved(), "capacité = junior − réservé");
        }
    }

    /// Pool type : 200 000 junior, 800 000 senior (ratio minimum de 20 % tout juste respecté).
    fn tranched() -> (Setup, Address, Address) {
        let s = setup();
        let junior = s.new_lp(200_000 * USDC);
        let senior = s.new_senior(800_000 * USDC);
        s.assert_tranches(0);
        (s, junior, senior)
    }

    #[test]
    fn hedge_loss_only_reduces_junior() {
        let (s, junior, senior) = tranched();
        let c = s.client();

        settle_pool_loses_margin(&s, 5_500 * USDC);
        s.assert_tranches(0);

        let t = c.tranche_values();
        assert_eq!(t.senior, 800_000 * USDC);
        assert_eq!(t.junior, 194_500 * USDC);
        assert_eq!(c.senior_balance(&senior), 800_000 * USDC);
        assert_eq!(c.lp_balance(&junior), 194_500 * USDC);
        // Prix de la part junior : 194 500 / 200 000.
        assert_eq!(c.share_price(), 9_725_000);
    }

    #[test]
    fn hedge_gain_only_raises_junior() {
        let (s, junior, senior) = tranched();
        settle_pool_gains_margin(&s, 4_400 * USDC);
        s.assert_tranches(0);
        assert_eq!(s.client().senior_balance(&senior), 800_000 * USDC);
        assert_eq!(s.client().lp_balance(&junior), 204_400 * USDC);
    }

    #[test]
    fn senior_value_grows_at_fixed_rate_paid_by_junior() {
        let (s, junior, senior) = tranched();
        let c = s.client();
        s.advance_secs(YEAR);

        // 800 000 × 4,5 % = 36 000, payés par le junior (capital libre non placé).
        let t = c.tranche_values();
        assert_eq!(t.senior, 836_000 * USDC);
        assert_eq!(t.junior, 164_000 * USDC);
        assert_eq!(t.senior_price, 10_450_000);
        assert_eq!(c.senior_balance(&senior), 836_000 * USDC);
        assert_eq!(c.lp_balance(&junior), 164_000 * USDC);
        s.assert_tranches(0);

        // Le senior retire tout : 836 000 versés, le junior garde 164 000.
        assert_eq!(c.withdraw_senior(&senior, &(800_000 * USDC)), 836_000 * USDC);
        assert_eq!(s.usdc_balance(&senior), 836_000 * USDC);
        assert_eq!(c.tranche_values(), TrancheValues { senior: 0, junior: 164_000 * USDC, senior_price: 10_450_000 });
        s.assert_tranches(0);
    }

    #[test]
    fn capacity_is_junior_minus_reserved() {
        let (s, _, _) = tranched();
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 10_000 * USDC);
        // Le senior ne compte pas : 200 000 − 10 000.
        assert_eq!(s.client().capacity(), 190_000 * USDC);
        s.assert_tranches(10_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn senior_capital_cannot_back_hedges() {
        let (s, _, _) = tranched();
        s.vault(|env| reserve(env, 200_000 * USDC + 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #111)")]
    fn senior_deposit_below_junior_ratio_panics() {
        let (s, _, _) = tranched();
        // Junior déjà à 20 % pile : un stroop de senior de plus le ferait passer dessous.
        s.new_senior(1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #111)")]
    fn first_senior_without_junior_panics() {
        let s = setup();
        s.new_senior(1_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #111)")]
    fn junior_withdraw_below_ratio_panics() {
        let (s, junior, _) = tranched();
        s.client().withdraw(&junior, &(1 * USDC));
    }

    #[test]
    fn junior_withdraw_pays_exit_price_without_reserved_capital() {
        let s = setup();
        let junior = s.new_lp(300_000 * USDC);
        s.new_senior(800_000 * USDC);
        let sme = Address::generate(&s.env);
        s.open_margin(&sme, 100_000 * USDC);

        // Prix de sortie : (300 000 − 100 000) / 300 000 ; 30 000 parts → 20 000 USDC.
        assert_eq!(s.client().withdraw(&junior, &(30_000 * USDC)), 20_000 * USDC);
        assert_eq!(s.client().capacity(), 180_000 * USDC);
        s.assert_tranches(100_000 * USDC);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #101)")]
    fn junior_at_zero_blocks_new_hedges() {
        let (s, _, senior) = tranched();
        settle_pool_loses_margin(&s, 200_000 * USDC);
        let t = s.client().tranche_values();
        assert_eq!(t.junior, 0);
        assert_eq!(t.senior, 800_000 * USDC);
        assert_eq!(s.client().senior_balance(&senior), 800_000 * USDC);
        assert_eq!(s.client().capacity(), 0);
        s.assert_tranches(0);
        s.vault(|env| reserve(env, 1));
    }

    #[test]
    fn two_seniors_at_different_dates_each_earn_from_their_entry() {
        let s = setup();
        s.new_lp(1_000_000 * USDC);
        let a = s.new_senior(800_000 * USDC);
        let c = s.client();

        // Un an plus tard, B entre au prix 1,045 : il reçoit moins de parts que A.
        s.advance_secs(YEAR);
        let b = s.new_senior(800_000 * USDC);
        assert_eq!(c.tranche_values().senior_price, 10_450_000);
        assert_eq!(s.vault(|env| senior_shares(env, &b)), 7_655_502_392_344);
        s.assert_tranches(0);

        // Encore un an : prix capitalisé à l'entrée de B, 1,045 × 1,045 = 1,092025.
        // A a gagné deux ans, B un an : avec un seul « SeniorSince », B toucherait aussi deux ans.
        s.advance_secs(YEAR);
        assert_eq!(c.tranche_values().senior_price, 10_920_250);
        // Arrondis vers le bas (en faveur du pool) : au plus 2 stroops sous la valeur exacte.
        let near = |value: i128, exact: i128| value <= exact && exact - value <= 2;
        let a_value = c.senior_balance(&a);
        assert!(near(a_value, 873_620 * USDC), "A ≈ 800 000 × 1,092025 : {a_value}");
        let b_value = c.senior_balance(&b);
        assert!(near(b_value, 836_000 * USDC), "B ≈ 800 000 × 1,045 : {b_value}");
        s.assert_tranches(0);

        assert!(near(c.withdraw_senior(&a, &(800_000 * USDC)), 873_620 * USDC));
        assert!(near(c.withdraw_senior(&b, &7_655_502_392_344), 836_000 * USDC));
        assert_eq!(c.tranche_values().senior, 0);
        s.assert_tranches(0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #112)")]
    fn withdraw_more_senior_shares_than_owned_panics() {
        let (s, _, senior) = tranched();
        s.client().withdraw_senior(&senior, &(800_000 * USDC + 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #100)")]
    fn deposit_senior_zero_panics() {
        let (s, _, _) = tranched();
        let lp = Address::generate(&s.env);
        s.client().deposit_senior(&lp, &0);
    }

    #[test]
    fn senior_value_capped_when_junior_is_exhausted() {
        // Junior épuisé puis un an d'intérêts : le senior ne peut pas valoir plus que le capital.
        let (s, _, senior) = tranched();
        settle_pool_loses_margin(&s, 200_000 * USDC);
        s.advance_secs(YEAR);
        let t = s.client().tranche_values();
        assert_eq!(t.senior, 800_000 * USDC);
        assert_eq!(t.junior, 0);
        assert_eq!(s.client().withdraw_senior(&senior, &(800_000 * USDC)), 800_000 * USDC);
        s.assert_tranches(0);
    }

    #[test]
    fn senior_deposit_and_withdraw_emit_events_and_extend_ttl() {
        let s = setup();
        s.new_lp(200_000 * USDC);
        let lp = s.new_senior(800_000 * USDC);
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [SeniorDeposited { lp: lp.clone(), amount: 800_000 * USDC, shares: 800_000 * USDC }
                .to_xdr(&s.env, &s.pool)]
        );
        assert_eq!(persistent_ttl(&s, &VaultKey::SeniorShares(lp.clone())), TTL_EXTEND_TO);

        advance_days(&s, 100);
        assert_eq!(s.client().senior_balance(&lp), 800_000 * USDC);
        assert_eq!(persistent_ttl(&s, &VaultKey::SeniorShares(lp.clone())), TTL_EXTEND_TO);

        s.client().withdraw_senior(&lp, &(100_000 * USDC));
        assert_eq!(
            s.env.events().all().filter_by_contract(&s.pool),
            [SeniorWithdrew { lp, shares: 100_000 * USDC, usdc: 100_000 * USDC }.to_xdr(&s.env, &s.pool)]
        );
    }

    #[test]
    fn senior_balance_of_unknown_lp_is_zero() {
        let s = setup();
        assert_eq!(s.client().senior_balance(&Address::generate(&s.env)), 0);
        assert_eq!(s.client().tranche_values(), TrancheValues { senior: 0, junior: 0, senior_price: PRICE_ONE });
    }
}
