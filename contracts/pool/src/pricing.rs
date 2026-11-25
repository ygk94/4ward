// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! PRICING — spot oracle, taux forward, spread, plafonds minimum.
//!
//! Convention : prix en USD par unité de devise, 14 décimales (format Reflector FX).
//! Forward (parité des taux) : F = S × (1 + r_USD·t) / (1 + r_dev·t), t = durée / 1 an.
//! Spread : appliqué contre l'utilisateur (BuyCurrency → F plus haut, SellCurrency → F plus bas).
//! Tout en i128, pas de flottants ; arrondir du côté du pool.
//! Erreurs : `PricingError`, codes 300+ (Vault 100+, Paiement 200+). Les textes de panic! sont effacés dans
//! le WASM : seul un code d'erreur permet au front d'afficher la bonne raison d'un refus.
use crate::{DataKey, Quote, Side};
use soroban_sdk::{
    contractclient, contracterror, contractevent, contracttype, panic_with_error, Address, Env, Symbol,
};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateSet {
    #[topic]
    pub currency: Symbol,
    pub rate_bps: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinCapSet {
    #[topic]
    pub currency: Symbol,
    pub cap_bps: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpreadSet {
    pub spread_bps: u32,
}

/// Erreurs de Pricing. Codes 300+ pour ne pas entrer en collision avec Vault (100+) / Paiement (200+).
/// Ne jamais réutiliser un numéro : ajouter à la suite.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PricingError {
    /// L'oracle n'a aucun prix pour la devise.
    NoPrice = 300,
    /// Prix de l'oracle nul ou négatif.
    InvalidPrice = 301,
    /// Prix plus vieux que `MAX_PRICE_AGE_SECS` : il faut le rafraîchir (mock) ou attendre Reflector.
    StalePrice = 302,
    /// Taux d'intérêt de la devise ou de l'USD non posé par `set_rate`.
    RateNotSet = 303,
    /// Durée de couverture supérieure à un an.
    DurationTooLong = 304,
    /// `quote` avec un notionnel nul ou négatif.
    InvalidNotional = 305,
    /// Devise non ouverte : pas de plafond minimum posé par `set_min_cap` (liste blanche).
    CurrencyNotEnabled = 306,
    /// Plafond demandé sous le minimum de la devise.
    CapBelowMinimum = 307,
    /// Plafond demandé au-dessus de `MAX_CAP_BPS`.
    CapTooHigh = 308,
    /// `set_rate` au-delà de 100 %/an.
    RateTooHigh = 309,
    /// `set_min_cap` à 0 ou au-delà de `MAX_CAP_BPS`.
    CapOutOfRange = 310,
    /// `set_spread` au-delà de 5 %.
    SpreadTooHigh = 311,
    /// Pas de cours de fixing à l'échéance : aucun prix à cette date (ou échéance trop ancienne
    /// pour l'historique de l'oracle, ~12 h sur Reflector testnet).
    NoFixing = 312,
    /// Dépassement de capacité dans un calcul de prix (`to_usdc`, `margin_for`, taux forward) :
    /// montant ou cours hors de toute plage réaliste. Un code plutôt qu'une panique sans numéro.
    Overflow = 313,
}

// ---- client SEP-40 (Reflector FX et mock-oracle) ----

/// Âge maximum accepté pour le dernier prix (`spot` : devis, ouverture, clôture anticipée), en secondes.
/// Reflector FX publie toutes les 300 s (`resolution`) : un prix a normalement moins de 330 s environ.
/// 600 s tolère une période manquée. Au-delà, le prix est jugé périmé et toute cotation est refusée
/// (un prix en retard sur le marché = arbitrage contre le pool).
pub const MAX_PRICE_AGE_SECS: u64 = 600;

/// Âge maximum, à l'échéance, du cours en vigueur à l'échéance (`read_fixing`) : 3 périodes Reflector.
/// Au-delà, on prend le premier cours publié après l'échéance, dans `FIXING_GRACE_SECS`.
pub const MAX_FIXING_AGE_SECS: u64 = 900;

/// Fenêtre de rattrapage après l'échéance : passé ce délai sans cours utilisable, le cours est constaté absent.
pub const FIXING_GRACE_SECS: u64 = 3_600;

/// Pas de recherche du premier cours après l'échéance : la résolution de Reflector.
const FIXING_STEP_SECS: u64 = 300;

/// Fixing en moyenne (dossier Instaward) : nombre de relevés, espacés de `FIXING_STEP_SECS`, qui se
/// terminent à l'échéance (maturity − 25 min … maturity, soit une fenêtre d'environ 30 min).
pub const TWAP_SAMPLES: usize = 6;
/// Nombre minimum de relevés valides, après bornes, pour calculer la moyenne.
pub const TWAP_MIN_SAMPLES: usize = 4;
/// Borne : un relevé à plus de 2 % de la médiane de la fenêtre est écarté.
pub const TWAP_BAND_BPS: i128 = 200;

/// TTL d'un cours figé : comme une couverture (30 jours / 120 jours, en ledgers de ~5 s).
const FIXING_TTL_THRESHOLD: u32 = 30 * 17_280;
const FIXING_TTL_EXTEND_TO: u32 = 120 * 17_280;

/// Clés de stockage de Pricing, à part de `DataKey` (lib.rs inchangé).
#[contracttype]
enum PricingKey {
    /// Cours de règlement figé d'une couverture (persistent, écrit une seule fois).
    Fixing(u64),
}

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
/// `decimals()` doit valoir 14 : vérifié au constructeur (`admin::check_oracle_decimals`).
#[contractclient(name = "OracleClient")]
#[allow(dead_code)] // le trait n'existe que pour engendrer `OracleClient`
pub trait Sep40Oracle {
    fn lastprice(env: Env, asset: Asset) -> Option<PriceData>;
    /// Prix en vigueur à `timestamp` (Reflector : cours de la période de 300 s qui le contient ; `None` si futur).
    fn price(env: Env, asset: Asset, timestamp: u64) -> Option<PriceData>;
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
        panic_with_error!(env, PricingError::InvalidNotional);
    }
    check_cap(env, currency, cap_bps);
    env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
    let spot = spot(env, currency);
    let forward_rate = forward_rate(env, currency, side, duration_secs);
    let usdc_cost = to_usdc(env, notional, forward_rate);
    let margin = margin_for(env, usdc_cost, cap_bps);
    Quote { spot, forward_rate, usdc_cost, margin }
}

/// Pose le taux annuel d'une devise, en bps. Le taux USD se pose aussi ici, avec le symbole `USD`.
pub(crate) fn set_rate(env: &Env, currency: &Symbol, rate_bps: u32) {
    crate::admin(env).require_auth();
    if rate_bps > MAX_RATE_BPS {
        panic_with_error!(env, PricingError::RateTooHigh);
    }
    env.storage().instance().set(&DataKey::Rate(currency.clone()), &rate_bps);
    RateSet { currency: currency.clone(), rate_bps }.publish(env);
}

/// Ouvre une devise avec son plafond minimum, en bps (liste blanche : sans minimum, pas de cotation).
pub(crate) fn set_min_cap(env: &Env, currency: &Symbol, cap_bps: u32) {
    crate::admin(env).require_auth();
    if cap_bps == 0 || cap_bps > MAX_CAP_BPS {
        panic_with_error!(env, PricingError::CapOutOfRange);
    }
    env.storage().instance().set(&DataKey::MinCap(currency.clone()), &cap_bps);
    MinCapSet { currency: currency.clone(), cap_bps }.publish(env);
}

pub(crate) fn set_spread(env: &Env, spread_bps: u32) {
    crate::admin(env).require_auth();
    if spread_bps > MAX_SPREAD_BPS {
        panic_with_error!(env, PricingError::SpreadTooHigh);
    }
    env.storage().instance().set(&DataKey::SpreadBps, &spread_bps);
    SpreadSet { spread_bps }.publish(env);
}

// ---- appelé par hedge.rs (interface Paiement → Pricing) ----

/// Dernier prix de l'oracle (Reflector ou mock, même interface SEP-40 : `lastprice(Asset::Other(sym))`).
/// USD par unité de devise, 14 décimales. Panique si pas de prix, prix non positif ou prix plus vieux
/// que `MAX_PRICE_AGE_SECS`.
pub(crate) fn spot(env: &Env, currency: &Symbol) -> i128 {
    let oracle: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
    let data = OracleClient::new(env, &oracle)
        .lastprice(&Asset::Other(currency.clone()))
        .unwrap_or_else(|| panic_with_error!(env, PricingError::NoPrice));
    if data.price <= 0 {
        panic_with_error!(env, PricingError::InvalidPrice);
    }
    // saturating : un oracle légèrement en avance sur le ledger n'est pas « périmé ».
    let age = env.ledger().timestamp().saturating_sub(data.timestamp);
    if age > MAX_PRICE_AGE_SECS {
        panic_with_error!(env, PricingError::StalePrice);
    }
    data.price
}

// ---- cours d'échéance (lot B1) ----

/// Lecture du cours d'échéance : trois issues.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FixingRead {
    /// Cours de règlement trouvé (USD par unité, 14 décimales).
    Found(i128),
    /// Aucun cours utilisable pour l'instant, mais la fenêtre `FIXING_GRACE_SECS` n'est pas écoulée.
    Pending,
    /// Aucun cours utilisable et la fenêtre est écoulée : constat d'absence (futur `settle_fallback`).
    Missing,
}

/// Résultat de la fenêtre de moyenne.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Twap {
    /// Moyenne des relevés retenus.
    Price(i128),
    /// Moins de `TWAP_MIN_SAMPLES` relevés dans la fenêtre (historique trop court, oracle muet) :
    /// on se replie sur la lecture d'un cours unique.
    Thin,
    /// Assez de relevés, mais moins de `TWAP_MIN_SAMPLES` dans la borne de la médiane : cours
    /// incohérents, aucun fixing (pas de repli sur un cours unique, qui serait manipulable).
    Inconsistent,
}

/// Moyenne des cours de la fenêtre qui se termine à l'échéance (lecture seule).
/// Relevé k (k = 0…`TWAP_SAMPLES` − 1) : cours en vigueur à `maturity − k × FIXING_STEP_SECS`, valide
/// s'il est positif et a au plus `MAX_FIXING_AGE_SECS` à cet instant. Les relevés à plus de
/// `TWAP_BAND_BPS` de la médiane sont écartés ; la moyenne (arrondie vers le bas) porte sur le reste.
/// Le même cours peut compter pour plusieurs relevés : c'est le cours en vigueur, comme une moyenne
/// pondérée par le temps.
fn read_twap(env: &Env, client: &OracleClient, asset: &Asset, maturity: u64) -> Twap {
    let mut prices = [0i128; TWAP_SAMPLES];
    let mut n = 0usize;
    for k in 0..TWAP_SAMPLES {
        let Some(ts) = maturity.checked_sub(k as u64 * FIXING_STEP_SECS) else { break };
        if let Some(d) = client.price(asset, &ts) {
            if d.price > 0 && d.timestamp <= ts && ts - d.timestamp <= MAX_FIXING_AGE_SECS {
                prices[n] = d.price;
                n += 1;
            }
        }
    }
    if n < TWAP_MIN_SAMPLES {
        return Twap::Thin;
    }
    let p = &mut prices[..n];
    p.sort_unstable();
    // Médiane basse : pour un nombre pair, la plus petite des deux valeurs centrales.
    let median = p[(n - 1) / 2];
    let band = median
        .checked_mul(TWAP_BAND_BPS)
        .unwrap_or_else(|| panic_with_error!(env, PricingError::Overflow))
        / 10_000;
    let (mut sum, mut kept) = (0i128, 0i128);
    for &x in p.iter() {
        if (x - median).abs() <= band {
            sum = sum.checked_add(x).unwrap_or_else(|| panic_with_error!(env, PricingError::Overflow));
            kept += 1;
        }
    }
    if (kept as usize) < TWAP_MIN_SAMPLES {
        return Twap::Inconsistent;
    }
    Twap::Price(sum / kept)
}

/// Lecture seule (rien n'est écrit), dans l'ordre :
/// 0. la moyenne de la fenêtre de ~30 min qui se termine à l'échéance (`read_twap`), si au moins
///    `TWAP_MIN_SAMPLES` relevés y sont valides. Si la fenêtre a assez de relevés mais trop sont hors de
///    la borne, aucun fixing : `Pending`, puis `Missing` (règlement de secours) ;
/// Sinon (fenêtre trop maigre), repli sur un cours unique :
/// 1. le cours en vigueur à l'échéance, s'il est positif et a au plus `MAX_FIXING_AGE_SECS` à l'échéance ;
/// 2. sinon le premier cours publié dans ]maturity, maturity + `FIXING_GRACE_SECS`], cherché par pas de
///    `FIXING_STEP_SECS` (exact sur Reflector, aligné sur 300 s ; sur le mock, deux prix à moins de 300 s
///    d'écart peuvent faire sauter le plus ancien, défaut accepté pour la démo) ;
/// 3. sinon `Pending` tant que `now < maturity + FIXING_GRACE_SECS`, puis `Missing`.
/// Un cours ≤ 0 est traité comme inutilisable (on continue de chercher) : un mauvais cours ne bloque pas le règlement.
pub(crate) fn read_fixing(env: &Env, currency: &Symbol, maturity: u64) -> FixingRead {
    let oracle: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
    let client = OracleClient::new(env, &oracle);
    let asset = Asset::Other(currency.clone());
    let now = env.ledger().timestamp();
    let deadline = maturity.saturating_add(FIXING_GRACE_SECS);

    if maturity <= now {
        match read_twap(env, &client, &asset, maturity) {
            Twap::Price(p) => return FixingRead::Found(p),
            Twap::Inconsistent => {
                return if now < deadline { FixingRead::Pending } else { FixingRead::Missing };
            }
            Twap::Thin => {}
        }
        if let Some(d) = client.price(&asset, &maturity) {
            if d.price > 0 && d.timestamp <= maturity && maturity - d.timestamp <= MAX_FIXING_AGE_SECS {
                return FixingRead::Found(d.price);
            }
        }
    }
    let mut ts = maturity;
    while ts < deadline {
        ts = ts.saturating_add(FIXING_STEP_SECS).min(deadline);
        if ts > now {
            break;
        }
        if let Some(d) = client.price(&asset, &ts) {
            if d.price > 0 && d.timestamp > maturity && d.timestamp <= deadline {
                return FixingRead::Found(d.price);
            }
        }
    }
    if now < deadline {
        FixingRead::Pending
    } else {
        FixingRead::Missing
    }
}

/// Cours figé d'une couverture, s'il existe.
pub(crate) fn locked_fixing(env: &Env, hedge_id: u64) -> Option<i128> {
    env.storage().persistent().get(&PricingKey::Fixing(hedge_id))
}

/// Fige le cours d'une couverture. Écrit une seule fois : sans effet si un cours est déjà figé.
fn lock_fixing(env: &Env, hedge_id: u64, price: i128) {
    let key = PricingKey::Fixing(hedge_id);
    let s = env.storage().persistent();
    if s.has(&key) {
        return;
    }
    s.set(&key, &price);
    s.extend_ttl(&key, FIXING_TTL_THRESHOLD, FIXING_TTL_EXTEND_TO);
}

/// Fige le cours d'échéance d'une couverture **sans la régler**, et le renvoie.
///
/// Reflector ne garde qu'environ 24 h d'historique : sans appel de `record_fixing` (ou de `settle`) dans les
/// 24 h qui suivent l'échéance, le cours est perdu et la couverture n'est plus réglable que par le futur
/// `settle_fallback` (Clément). Le robot doit donc l'appeler peu après chaque échéance.
///
/// Déjà figé : renvoie le cours figé, sans rien changer ni paniquer (appel répétable).
/// Pas encore de cours (`Pending`) ou plus de cours (`Missing`) : panique `NoFixing` (312).
#[allow(dead_code)] // exposée par Clément dans lib.rs
pub(crate) fn record_fixing(env: &Env, hedge_id: u64, currency: &Symbol, maturity: u64) -> i128 {
    if let Some(price) = locked_fixing(env, hedge_id) {
        return price;
    }
    match read_fixing(env, currency, maturity) {
        FixingRead::Found(price) => {
            lock_fixing(env, hedge_id, price);
            price
        }
        FixingRead::Pending | FixingRead::Missing => panic_with_error!(env, PricingError::NoFixing),
    }
}

/// Cours de règlement : le prix en vigueur **à l'échéance** (ou juste après), pas au moment où `settle` est
/// appelé, sinon celui qui règle choisit son moment.
/// - Cours figé par `record_fixing` : il est consommé (l'entrée est supprimée) et renvoyé.
/// - Sinon `read_fixing`, sans rien écrire : la couverture est réglée dans la même transaction.
/// - `Pending` ou `Missing` : panique `NoFixing` (312) ; la transaction est annulée, la couverture reste ouverte.
///
/// Sans appel de `record_fixing` (ou de `settle`) dans les 24 h après l'échéance, Reflector a oublié
/// l'historique : la couverture n'est alors réglable que par le futur `settle_fallback` (Clément).
/// Appelée par `hedge::settle` avec l'échéance réelle (`hedge.maturity`).
pub(crate) fn settlement_fixing(env: &Env, hedge_id: u64, currency: &Symbol, maturity: u64) -> i128 {
    if let Some(price) = locked_fixing(env, hedge_id) {
        env.storage().persistent().remove(&PricingKey::Fixing(hedge_id));
        return price;
    }
    match read_fixing(env, currency, maturity) {
        FixingRead::Found(price) => price,
        FixingRead::Pending | FixingRead::Missing => panic_with_error!(env, PricingError::NoFixing),
    }
}

/// Constat d'absence pour le futur `settle_fallback` : aucun cours figé et lecture `Missing`.
#[allow(dead_code)] // branché par `settle_fallback` (Clément)
pub(crate) fn fixing_missing(env: &Env, hedge_id: u64, currency: &Symbol, maturity: u64) -> bool {
    locked_fixing(env, hedge_id).is_none() && read_fixing(env, currency, maturity) == FixingRead::Missing
}

/// Taux bloqué à l'ouverture, spread inclus (USD par unité, 14 décimales).
///
/// Parité des taux en entiers, t = duration / SECS_PER_YEAR, taux en bps :
///   num = BPS × SECS_PER_YEAR + r_usd × duration
///   den = BPS × SECS_PER_YEAR + r_dev × duration
///   F   = S × num / den
/// Overflow : S ≈ 5,5e12 × num ≈ 3,2e11 → 1,8e24. Pire cas (S = 1e14, durée 1 an, taux 100 %) :
/// S × num = 1e14 × 6,3e11 = 6,3e25. Le spread ensuite : F ≤ 2 × S = 2e14, × (BPS + spread) ≤ 10_500
/// → 2,1e18. Tout reste très loin de i128::MAX ≈ 1,7e38 avec un cours réaliste ; les produits sont
/// quand même vérifiés (`Overflow`, 313), car le cours vient d'un contrat externe.
///
/// Arrondis : le spread arrondit du côté du pool ; la troncature de F (floor) peut avantager
/// l'acheteur de moins de 1e-14 USD par unité, sans effet mesurable. Le spread arrondit
/// `BuyCurrency` vers le haut (l'utilisateur paie un peu plus) et `SellCurrency` vers le bas
/// (il reçoit un peu moins). Arrondir dans l'autre sens laisserait une fraction d'unité par
/// couverture à l'utilisateur, exploitable en répétant de petites couvertures.
pub(crate) fn forward_rate(env: &Env, currency: &Symbol, side: Side, duration_secs: u64) -> i128 {
    if duration_secs > SECS_PER_YEAR {
        panic_with_error!(env, PricingError::DurationTooLong);
    }
    let s = spot(env, currency);
    let r_usd = rate(env, &Symbol::new(env, "USD"));
    let r_dev = rate(env, currency);
    let spread: u32 = env.storage().instance().get(&DataKey::SpreadBps).unwrap();

    let year = crate::BPS * SECS_PER_YEAR as i128;
    let duration = duration_secs as i128;
    let num = year + r_usd * duration;
    let den = year + r_dev * duration;
    let f = mul_div_floor(env, s, num, den);

    let spread = spread as i128;
    match side {
        Side::BuyCurrency => mul_div_ceil(env, f, crate::BPS + spread, crate::BPS),
        Side::SellCurrency => mul_div_floor(env, f, crate::BPS - spread, crate::BPS),
    }
}

/// Taux annuel en bps. Panique si absent : un taux manquant = prix faux = arbitrage, jamais de défaut.
fn rate(env: &Env, currency: &Symbol) -> i128 {
    let bps: u32 = env
        .storage()
        .instance()
        .get(&DataKey::Rate(currency.clone()))
        .unwrap_or_else(|| panic_with_error!(env, PricingError::RateNotSet));
    bps as i128
}

/// a × b / c arrondi vers le bas. Valeurs positives ; multiplier avant de diviser.
/// `Overflow` (313) si a × b dépasse i128, au lieu d'une panique sans code.
fn mul_div_floor(env: &Env, a: i128, b: i128, c: i128) -> i128 {
    a.checked_mul(b)
        .and_then(|p| p.checked_div(c))
        .unwrap_or_else(|| panic_with_error!(env, PricingError::Overflow))
}

/// a × b / c arrondi vers le haut. Valeurs positives. `Overflow` (313) comme `mul_div_floor`.
fn mul_div_ceil(env: &Env, a: i128, b: i128, c: i128) -> i128 {
    a.checked_mul(b)
        .and_then(|p| p.checked_add(c - 1))
        .and_then(|p| p.checked_div(c))
        .unwrap_or_else(|| panic_with_error!(env, PricingError::Overflow))
}

/// Panique si la devise n'est pas ouverte (pas de plafond minimum = hors liste blanche), si `cap_bps`
/// est sous le minimum de la devise ou au-dessus de `MAX_CAP_BPS`.
pub(crate) fn check_cap(env: &Env, currency: &Symbol, cap_bps: u32) {
    let min: u32 = env
        .storage()
        .instance()
        .get(&DataKey::MinCap(currency.clone()))
        .unwrap_or_else(|| panic_with_error!(env, PricingError::CurrencyNotEnabled));
    if cap_bps < min {
        panic_with_error!(env, PricingError::CapBelowMinimum);
    }
    if cap_bps > MAX_CAP_BPS {
        panic_with_error!(env, PricingError::CapTooHigh);
    }
}

/// Marge USDC (7 décimales) pour un plafond : usdc_cost × cap_bps / BPS, arrondi vers le haut (côté pool).
/// `hedge::open` doit appeler cette fonction : la marge déposée est alors identique à celle affichée par `quote`.
/// `Overflow` (313) si le produit dépasse i128.
pub(crate) fn margin_for(env: &Env, usdc_cost: i128, cap_bps: u32) -> i128 {
    mul_div_ceil(env, usdc_cost, cap_bps as i128, crate::BPS)
}

/// Conversion devise → USDC : notional (7 déc.) × rate (14 déc.) → USDC (7 déc.), arrondi vers le bas.
/// `Overflow` (313) si notional × rate dépasse i128.
pub(crate) fn to_usdc(env: &Env, notional: i128, rate: i128) -> i128 {
    mul_div_floor(env, notional, rate, 10i128.pow(crate::PRICE_DECIMALS))
}
