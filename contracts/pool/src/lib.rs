#![no_std]
//! Contrat principal : pool USDC des LPs + couvertures de change plafonnées.
//!
//! Découpage par personne (voir TODO.md) :
//! - `vault.rs`   — Vault : dépôts / retraits LP, capital réservé, flux USDC du pool
//! - `pricing.rs` — Pricing : spot oracle, taux forward, spread, plafonds minimum
//! - `hedge.rs`   — Paiement : ouverture et règlement des couvertures (marge, P&L, restitution)
//!
//! Ce fichier ne contient que les points d'entrée publics et les types partagés.
//! Les fonctions `pub(crate)` entre modules sont le contrat d'interface entre vous :
//! ne pas changer leur signature sans prévenir les deux autres.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

mod hedge;
mod pricing;
mod vault;

/// Décimales de l'oracle (Reflector FX) : prix en USD par unité de devise.
pub const PRICE_DECIMALS: u32 = 14;
/// Décimales des montants (USDC et notionnels en devise, comme les actifs Stellar).
pub const AMOUNT_DECIMALS: u32 = 7;
pub const BPS: i128 = 10_000;

/// Sens de la couverture, vu par l'utilisateur.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// Doit PAYER la devise (importateur) : protégé si la devise monte.
    BuyCurrency,
    /// Va RECEVOIR la devise (exportateur) : protégé si la devise baisse.
    SellCurrency,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quote {
    /// Spot de l'oracle (USD par unité, 14 décimales).
    pub spot: i128,
    /// Taux forward spread inclus (USD par unité, 14 décimales) = taux bloqué.
    pub forward_rate: i128,
    /// Coût total en USDC (7 décimales) au taux bloqué : notional × forward_rate.
    pub usdc_cost: i128,
    /// Marge à déposer en USDC (7 décimales) pour le plafond demandé.
    pub margin: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hedge {
    pub owner: Address,
    /// Fournisseur à payer en devise à l'échéance (utilisé par le front pour le path payment).
    pub supplier: Address,
    pub currency: Symbol,
    pub side: Side,
    /// Montant en devise, 7 décimales (1 000 000 MXN = 10_000_000_000_000).
    pub notional: i128,
    /// Taux bloqué (USD par unité, 14 décimales).
    pub locked_rate: i128,
    /// Plafond = marge, en points de base (1000 = 10 %).
    pub cap_bps: u32,
    /// Marge déposée en USDC (7 décimales). Égale au capital réservé dans le vault.
    pub margin: i128,
    /// Échéance, en secondes Unix (`env.ledger().timestamp()`).
    pub maturity: u64,
    pub settled: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Usdc,
    Oracle,
    // -- pricing
    SpreadBps,
    /// Taux d'intérêt annuel par devise, en bps. `USD` inclus.
    Rate(Symbol),
    /// Plafond minimum par devise, en bps.
    MinCap(Symbol),
    // -- vault
    LpBalance(Address),
    TotalDeposits,
    /// Σ des marges des couvertures ouvertes = perte max du pool.
    Reserved,
    // -- hedge
    NextId,
    Hedge(u64),
}

pub(crate) fn admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

#[contract]
pub struct Pool;

#[contractimpl]
impl Pool {
    pub fn __constructor(env: Env, admin: Address, usdc: Address, oracle: Address, spread_bps: u32) {
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Usdc, &usdc);
        s.set(&DataKey::Oracle, &oracle);
        s.set(&DataKey::SpreadBps, &spread_bps);
        s.set(&DataKey::TotalDeposits, &0i128);
        s.set(&DataKey::Reserved, &0i128);
        s.set(&DataKey::NextId, &0u64);
    }

    // ---- Vault ----

    /// Dépose `amount` USDC (7 décimales) dans le pool contre des parts au prix plein.
    /// Retourne le nombre de parts du LP après le dépôt (7 décimales).
    pub fn deposit_lp(env: Env, lp: Address, amount: i128) -> i128 {
        vault::deposit(&env, &lp, amount)
    }

    /// Brûle `amount` PARTS du LP et lui verse leur valeur au prix de sortie
    /// ((valeur junior - Reserved) / TotalShares), donc jamais plus que le capital libre.
    /// Retourne l'USDC versé (7 décimales).
    pub fn withdraw(env: Env, lp: Address, amount: i128) -> i128 {
        vault::withdraw(&env, &lp, amount)
    }

    /// Capital encore disponible pour de nouvelles couvertures.
    pub fn capacity(env: Env) -> i128 {
        vault::capacity(&env)
    }

    /// Valeur en USDC des parts du LP au prix de sortie (ce qu'il recevrait en retirant tout).
    pub fn lp_balance(env: Env, lp: Address) -> i128 {
        vault::balance(&env, &lp)
    }

    /// Parts détenues par le LP (7 décimales).
    pub fn lp_shares(env: Env, lp: Address) -> i128 {
        vault::lp_shares(&env, &lp)
    }

    /// Prix plein d'une part junior en USDC (7 décimales) : valeur junior / TotalShares.
    pub fn share_price(env: Env) -> i128 {
        vault::share_price(&env)
    }

    /// Prix de sortie d'une part junior en USDC (7 décimales) : (valeur junior - Reserved) / TotalShares.
    pub fn exit_price(env: Env) -> i128 {
        vault::exit_price(&env)
    }

    /// Capital des LPs, senior + junior (TotalDeposits), en USDC.
    pub fn total_assets(env: Env) -> i128 {
        vault::total_assets(&env)
    }

    /// Dépose `amount` USDC dans la tranche senior (rendement fixe, protégée par le junior).
    /// Retourne le nombre de parts senior du LP après le dépôt (7 décimales).
    pub fn deposit_senior(env: Env, lp: Address, amount: i128) -> i128 {
        vault::deposit_senior(&env, &lp, amount)
    }

    /// Brûle `shares` PARTS senior du LP et lui verse leur valeur au prix courant.
    /// Retourne l'USDC versé (7 décimales).
    pub fn withdraw_senior(env: Env, lp: Address, shares: i128) -> i128 {
        vault::withdraw_senior(&env, &lp, shares)
    }

    /// Valeur en USDC des parts senior du LP au prix courant.
    pub fn senior_balance(env: Env, lp: Address) -> i128 {
        vault::senior_balance(&env, &lp)
    }

    /// Valeur des deux tranches (senior + junior = total_assets) et prix d'une part senior.
    pub fn tranche_values(env: Env) -> vault::TrancheValues {
        vault::tranche_values(&env)
    }

    /// Exposition du pool sur une devise (couvertures ouvertes) : volumes acheteur / vendeur
    /// en USDC au taux bloqué, net = acheteur − vendeur, et marges de chaque côté.
    pub fn exposure(env: Env, currency: Symbol) -> vault::Exposure {
        vault::exposure(&env, &currency)
    }

    /// Limite de la devise en bps du capital (Σ marges de la devise), `None` = sans limite.
    pub fn currency_limit(env: Env, currency: Symbol) -> Option<u32> {
        vault::currency_limit(&env, &currency)
    }

    /// Admin : Σ marges ouvertes d'une devise ≤ `limit_bps` × capital / 10 000.
    /// 0 = plus de nouvelles couvertures sur cette devise.
    pub fn set_currency_limit(env: Env, currency: Symbol, limit_bps: u32) {
        vault::set_currency_limit(&env, &currency, limit_bps)
    }

    // ---- Pricing ----

    pub fn quote(env: Env, currency: Symbol, side: Side, notional: i128, duration_secs: u64, cap_bps: u32) -> Quote {
        pricing::quote(&env, &currency, side, notional, duration_secs, cap_bps)
    }

    pub fn set_rate(env: Env, currency: Symbol, rate_bps: u32) {
        pricing::set_rate(&env, &currency, rate_bps)
    }

    pub fn set_min_cap(env: Env, currency: Symbol, cap_bps: u32) {
        pricing::set_min_cap(&env, &currency, cap_bps)
    }

    pub fn set_spread(env: Env, spread_bps: u32) {
        pricing::set_spread(&env, spread_bps)
    }

    // ---- Couvertures / paiement ----

    /// Ouvre une couverture et retourne son id.
    pub fn open_hedge(
        env: Env,
        owner: Address,
        supplier: Address,
        currency: Symbol,
        side: Side,
        notional: i128,
        duration_secs: u64,
        cap_bps: u32,
    ) -> u64 {
        hedge::open(&env, &owner, &supplier, &currency, side, notional, duration_secs, cap_bps)
    }

    /// Permissionless après l'échéance. Retourne le montant USDC versé au propriétaire.
    pub fn settle(env: Env, hedge_id: u64) -> i128 {
        hedge::settle(&env, hedge_id)
    }

    /// Propriétaire seul, avant l'échéance : clôture au prix du marché (forward du jour pour la durée restante,
    /// spread inclus). Le perdant paie. Retourne le montant USDC versé au propriétaire.
    pub fn close_early(env: Env, hedge_id: u64) -> i128 {
        hedge::close_early(&env, hedge_id)
    }

    pub fn get_hedge(env: Env, hedge_id: u64) -> Option<Hedge> {
        hedge::get(&env, hedge_id)
    }

    /// Mode démo : échéance réelle = durée annoncée / `time_factor` (le prix reste calculé sur la
    /// durée annoncée). 1 par défaut = temps réel.
    pub fn time_factor(env: Env) -> u32 {
        hedge::time_factor(&env)
    }

    /// Admin, testnet / démo uniquement : ex. 25 920 → une couverture à 90 jours échoit en 300 s.
    pub fn set_time_factor(env: Env, factor: u32) {
        hedge::set_time_factor(&env, factor)
    }
}

mod test;
