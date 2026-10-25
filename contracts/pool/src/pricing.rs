//! PRICING — spot oracle, taux forward, spread, plafonds minimum.
//!
//! Convention : prix en USD par unité de devise, 14 décimales (format Reflector FX).
//! Forward (parité des taux) : F = S × (1 + r_USD·t) / (1 + r_dev·t), t = durée / 1 an.
//! Spread : appliqué contre l'utilisateur (BuyCurrency → F plus haut, SellCurrency → F plus bas).
//! Tout en i128, pas de flottants ; arrondir du côté du pool.
use crate::{DataKey, Quote, Side};
use soroban_sdk::{contractclient, contracttype, Address, Env, Symbol};

// ---- client SEP-40 (Reflector FX et mock-oracle) ----

/// Âge maximum accepté pour un prix, en secondes.
/// Reflector FX publie toutes les 300 s (`resolution`) ; on tolère 3 périodes manquées.
/// Au-delà, le prix est jugé périmé et toute cotation est refusée (un prix faux = arbitrage contre le pool).
pub const MAX_PRICE_AGE_SECS: u64 = 900;

/// Même forme que l'`Asset` SEP-40 de Reflector et du mock-oracle : les devises fiat sont `Other(symbole)`.
/// Les noms de type et de variante doivent rester identiques pour que l'encodage XDR coïncide.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

/// Prix SEP-40 : `price` en USD par unité de devise, 14 décimales (`crate::PRICE_DECIMALS`), horodaté en secondes Unix.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// Sous-ensemble de l'interface SEP-40 dont le pool a besoin.
/// Hypothèse documentée, non vérifiée à chaque appel : `decimals()` vaut 14 sur Reflector FX comme sur le mock.
#[contractclient(name = "OracleClient")]
#[allow(dead_code)] // le trait n'existe que pour engendrer `OracleClient`
pub trait Sep40Oracle {
    fn lastprice(env: Env, asset: Asset) -> Option<PriceData>;
    fn decimals(env: Env) -> u32;
}

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
/// USD par unité de devise, 14 décimales. Panique si pas de prix, prix non positif ou prix plus vieux
/// que `MAX_PRICE_AGE_SECS`.
pub(crate) fn spot(env: &Env, currency: &Symbol) -> i128 {
    let oracle: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
    let data = OracleClient::new(env, &oracle)
        .lastprice(&Asset::Other(currency.clone()))
        .unwrap_or_else(|| panic!("no price"));
    if data.price <= 0 {
        panic!("invalid price");
    }
    // saturating : un oracle légèrement en avance sur le ledger n'est pas « périmé ».
    let age = env.ledger().timestamp().saturating_sub(data.timestamp);
    if age > MAX_PRICE_AGE_SECS {
        panic!("stale price");
    }
    data.price
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
