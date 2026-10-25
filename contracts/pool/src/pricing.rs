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

/// Année de 365 jours (convention ACT/365 simplifiée) : t = durée / SECS_PER_YEAR.
/// C'est aussi la durée maximale d'une couverture.
pub const SECS_PER_YEAR: u64 = 31_536_000;

/// Taux d'intérêt annuel maximum accepté par `set_rate` : 100 %/an (ARS peut être haut).
const MAX_RATE_BPS: u32 = 10_000;
/// Spread maximum accepté par `set_spread` : 5 %.
const MAX_SPREAD_BPS: u32 = 500;
/// Plafond maximum, pour `set_min_cap` comme pour une cotation : 50 %. Au-delà, ce n'est plus une couverture.
pub const MAX_CAP_BPS: u32 = 5_000;

/// TTL du storage instance (Rate, MinCap, SpreadBps, Admin, Oracle…), en ledgers (~5 s chacun).
/// Prolongé à chaque `quote` quand il reste moins d'≈ 1 jour, jusqu'à ≈ 30 jours.
const INSTANCE_TTL_THRESHOLD: u32 = 17_280;
const INSTANCE_TTL_EXTEND_TO: u32 = 518_400;

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

/// Devis complet. `check_cap` passe avant tout appel à l'oracle : une devise non ouverte ou un plafond
/// hors bornes est refusé sans dépendre de l'état de l'oracle.
pub(crate) fn quote(
    env: &Env,
    currency: &Symbol,
    side: Side,
    notional: i128,
    duration_secs: u64,
    cap_bps: u32,
) -> Quote {
    if notional <= 0 {
        panic!("invalid notional");
    }
    check_cap(env, currency, cap_bps);
    env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
    let spot = spot(env, currency);
    let forward_rate = forward_rate(env, currency, side, duration_secs);
    let usdc_cost = to_usdc(notional, forward_rate);
    let margin = margin_for(usdc_cost, cap_bps);
    Quote { spot, forward_rate, usdc_cost, margin }
}

/// Pose le taux annuel d'une devise, en bps. Le taux USD se pose aussi ici, avec le symbole `USD`.
pub(crate) fn set_rate(env: &Env, currency: &Symbol, rate_bps: u32) {
    crate::admin(env).require_auth();
    if rate_bps > MAX_RATE_BPS {
        panic!("rate too high");
    }
    env.storage().instance().set(&DataKey::Rate(currency.clone()), &rate_bps);
}

/// Ouvre une devise avec son plafond minimum, en bps (liste blanche : sans minimum, pas de cotation).
pub(crate) fn set_min_cap(env: &Env, currency: &Symbol, cap_bps: u32) {
    crate::admin(env).require_auth();
    if cap_bps == 0 || cap_bps > MAX_CAP_BPS {
        panic!("cap out of range");
    }
    env.storage().instance().set(&DataKey::MinCap(currency.clone()), &cap_bps);
}

pub(crate) fn set_spread(env: &Env, spread_bps: u32) {
    crate::admin(env).require_auth();
    if spread_bps > MAX_SPREAD_BPS {
        panic!("spread too high");
    }
    env.storage().instance().set(&DataKey::SpreadBps, &spread_bps);
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

/// Taux bloqué à l'ouverture, spread inclus (USD par unité, 14 décimales).
///
/// Parité des taux en entiers, t = duration / SECS_PER_YEAR, taux en bps :
///   num = BPS × SECS_PER_YEAR + r_usd × duration
///   den = BPS × SECS_PER_YEAR + r_dev × duration
///   F   = S × num / den
/// Overflow : S ≈ 5,5e12 × num ≈ 3,2e11 → 1,8e24. Pire cas (S = 1e14, durée 1 an, taux 100 %) :
/// S × num = 1e14 × 6,3e11 = 6,3e25. Le spread ensuite : F ≤ 2 × S = 2e14, × (BPS + spread) ≤ 10_500
/// → 2,1e18. Tout reste très loin de i128::MAX ≈ 1,7e38.
///
/// Arrondis : toujours du côté du pool. F sans spread est tronqué (floor) ; le spread arrondit
/// `BuyCurrency` vers le haut (l'utilisateur paie un peu plus) et `SellCurrency` vers le bas
/// (il reçoit un peu moins). Arrondir dans l'autre sens laisserait une fraction d'unité par
/// couverture à l'utilisateur, exploitable en répétant de petites couvertures.
pub(crate) fn forward_rate(env: &Env, currency: &Symbol, side: Side, duration_secs: u64) -> i128 {
    if duration_secs > SECS_PER_YEAR {
        panic!("duration too long");
    }
    let s = spot(env, currency);
    let r_usd = rate(env, &Symbol::new(env, "USD"));
    let r_dev = rate(env, currency);
    let spread: u32 = env.storage().instance().get(&DataKey::SpreadBps).unwrap();

    let year = crate::BPS * SECS_PER_YEAR as i128;
    let duration = duration_secs as i128;
    let num = year + r_usd * duration;
    let den = year + r_dev * duration;
    let f = mul_div_floor(s, num, den);

    let spread = spread as i128;
    match side {
        Side::BuyCurrency => mul_div_ceil(f, crate::BPS + spread, crate::BPS),
        Side::SellCurrency => mul_div_floor(f, crate::BPS - spread, crate::BPS),
    }
}

/// Taux annuel en bps. Panique si absent : un taux manquant = prix faux = arbitrage, jamais de défaut.
fn rate(env: &Env, currency: &Symbol) -> i128 {
    let bps: u32 = env
        .storage()
        .instance()
        .get(&DataKey::Rate(currency.clone()))
        .unwrap_or_else(|| panic!("rate not set"));
    bps as i128
}

/// a × b / c arrondi vers le bas. Valeurs positives ; multiplier avant de diviser.
fn mul_div_floor(a: i128, b: i128, c: i128) -> i128 {
    a * b / c
}

/// a × b / c arrondi vers le haut. Valeurs positives.
fn mul_div_ceil(a: i128, b: i128, c: i128) -> i128 {
    (a * b + c - 1) / c
}

/// Panique si la devise n'est pas ouverte (pas de plafond minimum = hors liste blanche), si `cap_bps`
/// est sous le minimum de la devise ou au-dessus de `MAX_CAP_BPS`.
pub(crate) fn check_cap(env: &Env, currency: &Symbol, cap_bps: u32) {
    let min: u32 = env
        .storage()
        .instance()
        .get(&DataKey::MinCap(currency.clone()))
        .unwrap_or_else(|| panic!("currency not enabled"));
    if cap_bps < min {
        panic!("cap below minimum");
    }
    if cap_bps > MAX_CAP_BPS {
        panic!("cap too high");
    }
}

/// Marge USDC (7 décimales) pour un plafond : usdc_cost × cap_bps / BPS, arrondi vers le haut (côté pool).
/// `hedge::open` doit appeler cette fonction : la marge déposée est alors identique à celle affichée par `quote`.
pub(crate) fn margin_for(usdc_cost: i128, cap_bps: u32) -> i128 {
    mul_div_ceil(usdc_cost, cap_bps as i128, crate::BPS)
}

/// Conversion devise → USDC : notional (7 déc.) × rate (14 déc.) → USDC (7 déc.).
pub(crate) fn to_usdc(notional: i128, rate: i128) -> i128 {
    notional * rate / 10i128.pow(crate::PRICE_DECIMALS)
}
