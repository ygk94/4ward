//! PRICING — spot oracle, taux forward, spread, plafonds minimum.
//!
//! Convention : prix en USD par unité de devise, 14 décimales (format Reflector FX).
//! Forward (parité des taux) : F = S × (1 + r_USD·t) / (1 + r_dev·t), t = durée / 1 an.
//! Spread : appliqué contre l'utilisateur (BuyCurrency → F plus haut, SellCurrency → F plus bas).
//! Tout en i128, pas de flottants ; arrondir du côté du pool.
use crate::{Quote, Side};
use soroban_sdk::{Env, Symbol};

// ---- appelé par lib.rs ----

pub(crate) fn quote(
    _env: &Env,
    _currency: &Symbol,
    _side: Side,
    _notional: i128,
    _duration_secs: u64,
    _cap_bps: u32,
) -> Quote {
    panic!("TODO pricing")
}

pub(crate) fn set_rate(_env: &Env, _currency: &Symbol, _rate_bps: u32) {
    panic!("TODO pricing")
}

pub(crate) fn set_min_cap(_env: &Env, _currency: &Symbol, _cap_bps: u32) {
    panic!("TODO pricing")
}

pub(crate) fn set_spread(_env: &Env, _spread_bps: u32) {
    panic!("TODO pricing")
}

// ---- appelé par hedge.rs (interface Paiement → Pricing) ----

/// Dernier prix de l'oracle (Reflector ou mock, même interface SEP-40 : `lastprice(Asset::Other(sym))`).
/// Panique si pas de prix ou prix trop ancien.
pub(crate) fn spot(_env: &Env, _currency: &Symbol) -> i128 {
    panic!("TODO pricing")
}

/// Taux bloqué à l'ouverture, spread inclus.
pub(crate) fn forward_rate(_env: &Env, _currency: &Symbol, _side: Side, _duration_secs: u64) -> i128 {
    panic!("TODO pricing")
}

/// Panique si `cap_bps` est sous le plafond minimum de la devise.
pub(crate) fn check_cap(_env: &Env, _currency: &Symbol, _cap_bps: u32) {
    panic!("TODO pricing")
}

/// Conversion devise → USDC : notional (7 déc.) × rate (14 déc.) → USDC (7 déc.).
pub(crate) fn to_usdc(notional: i128, rate: i128) -> i128 {
    notional * rate / 10i128.pow(crate::PRICE_DECIMALS)
}
