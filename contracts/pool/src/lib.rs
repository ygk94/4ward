#![no_std]
//! Contrat principal : pool USDC des LPs + couvertures de change plafonnées (NDF réglé en USDC).
//!
//! Squelette d'interface figé pour que front et contrat avancent en parallèle.
//! Les corps sont à écrire, en commençant par `settle` (voir `test.rs`).
//! Spec complète : README.md et docs/SPEC.md.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

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
    /// Taux forward + spread, en USD par unité de devise, 14 décimales (comme l'oracle).
    pub forward_rate: i128,
    /// Coût total en USDC (7 décimales) au taux bloqué.
    pub usdc_cost: i128,
    /// Spot de l'oracle utilisé pour le calcul.
    pub spot: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hedge {
    pub owner: Address,
    pub currency: Symbol,
    pub side: Side,
    /// Montant en devise (ex. 1_000_000 MXN), en unités entières de devise.
    pub notional: i128,
    pub locked_rate: i128,
    /// Plafond = marge, en points de base (1000 = 10 %).
    pub cap_bps: u32,
    /// Marge déposée en USDC (7 décimales).
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
    SpreadBps,
    /// Taux d'intérêt annuel par devise, en bps (codés en dur pour le MVP).
    Rate(Symbol),
    /// Plafond minimum par devise, en bps.
    MinCap(Symbol),
    LpBalance(Address),
    TotalDeposits,
    /// Σ (notionnel en USDC × plafond) des couvertures ouvertes = perte max du pool.
    Reserved,
    NextId,
    Hedge(u64),
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

    // ---- LPs ----

    /// Dépose des USDC dans le pool. Retourne le nouveau solde du LP.
    pub fn deposit_lp(_env: Env, _lp: Address, _amount: i128) -> i128 {
        panic!("TODO")
    }

    /// Retire des USDC, limité au capital non réservé (TotalDeposits - Reserved).
    pub fn withdraw(_env: Env, _lp: Address, _amount: i128) -> i128 {
        panic!("TODO")
    }

    // ---- Utilisateurs ----

    /// Spot et F en USD par unité de devise : F = Spot × (1 + r_USD·t) / (1 + r_devise·t) ± spread.
    pub fn quote(_env: Env, _currency: Symbol, _side: Side, _notional: i128, _duration_secs: u64) -> Quote {
        panic!("TODO")
    }

    /// Ouvre une couverture : transfère la marge (notionnel USDC × cap_bps) et vérifie la capacité.
    /// Retourne l'id de la position.
    pub fn open_hedge(
        _env: Env,
        _owner: Address,
        _currency: Symbol,
        _side: Side,
        _notional: i128,
        _duration_secs: u64,
        _cap_bps: u32,
    ) -> u64 {
        panic!("TODO")
    }

    /// Permissionless après l'échéance. Règle l'écart (taux bloqué vs spot oracle), borné par le plafond,
    /// rend le reste de la marge au propriétaire. Retourne le montant versé au propriétaire.
    pub fn settle(_env: Env, _hedge_id: u64) -> i128 {
        panic!("TODO")
    }

    pub fn get_hedge(env: Env, hedge_id: u64) -> Option<Hedge> {
        env.storage().persistent().get(&DataKey::Hedge(hedge_id))
    }

    // ---- Pool ----

    /// Capital encore disponible pour de nouvelles couvertures.
    pub fn capacity(env: Env) -> i128 {
        let s = env.storage().instance();
        let total: i128 = s.get(&DataKey::TotalDeposits).unwrap_or(0);
        let reserved: i128 = s.get(&DataKey::Reserved).unwrap_or(0);
        total - reserved
    }

    // ---- Admin (MVP) ----

    pub fn set_rate(_env: Env, _currency: Symbol, _rate_bps: u32) {
        panic!("TODO")
    }

    pub fn set_min_cap(_env: Env, _currency: Symbol, _cap_bps: u32) {
        panic!("TODO")
    }
}

mod test;
