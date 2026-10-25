//! VAULT — dépôts / retraits des LPs, capital réservé, flux USDC du pool.
//!
//! Compta : `TotalDeposits` = capital du pool (bouge avec le P&L des couvertures).
//! `Reserved` = Σ des marges des couvertures ouvertes. Capacité = TotalDeposits - Reserved.
//! MVP : soldes LP en mapping simple (pas de parts SEP-41). Si le P&L doit être réparti
//! entre LPs, passer à des parts (shares) : décision du Vault.
use crate::DataKey;
use soroban_sdk::{Address, Env};

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
    let s = env.storage().instance();
    let total: i128 = s.get(&DataKey::TotalDeposits).unwrap_or(0);
    let reserved: i128 = s.get(&DataKey::Reserved).unwrap_or(0);
    total - reserved
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
