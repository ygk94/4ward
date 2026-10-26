//! PAIEMENT — ouverture et règlement des couvertures.
//!
//! open : prix bloqué (pricing::forward_rate), contrôle du plafond (pricing::check_cap),
//!        marge = to_usdc(notional, locked_rate) × cap_bps / BPS, transférée owner → contrat,
//!        vault::open_position(&hedge) (réserve la marge, exposition par devise), stockage (+ extend_ttl), événement.
//! settle : après l'échéance, spot = pricing::fixing(maturity) (cours en vigueur à l'échéance) ; P&L = pnl(side, notional, locked, spot, marge)
//!        (écart notional × (spot − locked), signe selon Side, borné à ±marge) ; vault::close_position(&hedge) puis book_gain / book_loss ;
//!        transfert au propriétaire de marge ± P&L ; settled = true ; événement.
//! close_early : avant l'échéance, le propriétaire seul. Clôture au prix du marché : taux de sortie = forward du jour
//!        pour la durée restante, dans le sens inverse (spread contre l'utilisateur, comme à l'ouverture : c'est le
//!        frais de sortie) ; même P&L borné, même versement que settle. Le perdant paie, le pool libère sa réserve.
//! Ensuite, côté front : path payment USDC → devise vers `supplier` pour exactement `notional`.
//! Mode démo : `time_factor` (admin, 1 par défaut) raccourcit l'échéance réelle (`durée / facteur`)
//! sans changer le prix, calculé sur la durée annoncée. Ex. 25 920 : 90 jours → 300 s.
//! Erreurs : `HedgeError`, codes 200+ (Vault 100+). Les textes de panic! sont effacés dans le WASM :
//! seul un code d'erreur permet au front d'afficher la bonne raison d'un refus.
use crate::{pricing, vault, DataKey, Hedge, Side};
use soroban_sdk::{contracterror, contractevent, contracttype, panic_with_error, token, Address, Env, Symbol};

/// Erreurs de Paiement. Codes 200+ pour ne pas entrer en collision avec Vault (100+) / Pricing.
/// Ne jamais réutiliser un numéro : ajouter à la suite.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum HedgeError {
    /// `settle` / `get_hedge` sur un id qui n'existe pas.
    UnknownHedge = 200,
    /// `settle` sur une couverture déjà réglée.
    AlreadySettled = 201,
    /// `settle` avant l'échéance.
    NotMatured = 202,
    /// `open` avec un notionnel nul ou négatif.
    InvalidNotional = 203,
    /// `open` avec une durée nulle.
    InvalidDuration = 204,
    /// `set_time_factor` à 0.
    InvalidTimeFactor = 205,
    /// `close_early` à l'échéance ou après : utiliser `settle`.
    AlreadyMatured = 206,
}

/// Clés de stockage de Paiement (à part de `DataKey`, partagé).
#[contracttype]
enum HedgeKey {
    /// Facteur d'accélération du temps (mode démo), 1 par défaut.
    TimeFactor,
}

/// Réglage du mode démo, publié pour que tout le monde voie que les échéances sont accélérées.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeFactorSet {
    pub factor: u32,
}

/// Couverture ouverte (visible sur l'explorateur, lisible par le front).
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HedgeOpened {
    #[topic]
    pub hedge_id: u64,
    #[topic]
    pub owner: Address,
    pub supplier: Address,
    pub currency: Symbol,
    pub side: Side,
    pub notional: i128,
    pub locked_rate: i128,
    pub cap_bps: u32,
    pub margin: i128,
    pub maturity: u64,
}

/// Couverture réglée : cours de fixing, P&L de l'utilisateur et montant qui lui a été versé.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HedgeSettled {
    #[topic]
    pub hedge_id: u64,
    #[topic]
    pub owner: Address,
    pub spot: i128,
    pub pnl: i128,
    pub payout: i128,
}

/// Couverture clôturée avant l'échéance par son propriétaire : taux de sortie (forward du jour pour la durée
/// restante, spread inclus), P&L de l'utilisateur et montant qui lui a été versé.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HedgeClosedEarly {
    #[topic]
    pub hedge_id: u64,
    #[topic]
    pub owner: Address,
    pub close_rate: i128,
    pub pnl: i128,
    pub payout: i128,
}

/// Durée de conservation d'une couverture sur la chaîne, en ledgers (~5 s) : prolongée à 120 jours
/// dès qu'elle passe sous 30 jours.
const DAY_IN_LEDGERS: u32 = 17_280;
const HEDGE_TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const HEDGE_TTL_EXTEND_TO: u32 = 120 * DAY_IN_LEDGERS;

pub(crate) fn open(
    env: &Env,
    owner: &Address,
    supplier: &Address,
    currency: &Symbol,
    side: Side,
    notional: i128,
    duration_secs: u64,
    cap_bps: u32,
) -> u64 {
    owner.require_auth();
    if notional <= 0 {
        panic_with_error!(env, HedgeError::InvalidNotional);
    }
    if duration_secs == 0 {
        panic_with_error!(env, HedgeError::InvalidDuration);
    }
    pricing::check_cap(env, currency, cap_bps);

    let locked_rate = pricing::forward_rate(env, currency, side, duration_secs);
    // même calcul que `quote` : la marge déposée est celle affichée (arrondie vers le haut)
    let margin = pricing::margin_for(pricing::to_usdc(notional, locked_rate), cap_bps);

    let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    token::TokenClient::new(env, &usdc).transfer(owner, &env.current_contract_address(), &margin);

    let hedge = Hedge {
        owner: owner.clone(),
        supplier: supplier.clone(),
        currency: currency.clone(),
        side,
        notional,
        locked_rate,
        cap_bps,
        margin,
        // prix calculé sur `duration_secs` ; échéance réelle raccourcie en mode démo, au moins 1 s
        maturity: env.ledger().timestamp() + (duration_secs / time_factor(env) as u64).max(1),
        settled: false,
    };
    vault::open_position(env, &hedge);

    let s = env.storage().instance();
    let id: u64 = s.get(&DataKey::NextId).unwrap();
    s.set(&DataKey::NextId, &(id + 1));
    let key = DataKey::Hedge(id);
    env.storage().persistent().set(&key, &hedge);
    env.storage().persistent().extend_ttl(&key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);

    HedgeOpened {
        hedge_id: id,
        owner: hedge.owner,
        supplier: hedge.supplier,
        currency: hedge.currency,
        side,
        notional,
        locked_rate,
        cap_bps,
        margin,
        maturity: hedge.maturity,
    }
    .publish(env);
    id
}

pub(crate) fn settle(env: &Env, hedge_id: u64) -> i128 {
    let key = DataKey::Hedge(hedge_id);
    let mut hedge = load_open(env, &key);
    if env.ledger().timestamp() < hedge.maturity {
        panic_with_error!(env, HedgeError::NotMatured);
    }

    // cours en vigueur à l'échéance réelle, pas au moment de l'appel : personne ne choisit son moment
    let spot = pricing::fixing(env, &hedge.currency, hedge.maturity);
    let (pnl, payout) = close(env, &key, &mut hedge, spot);
    HedgeSettled { hedge_id, owner: hedge.owner, spot, pnl, payout }.publish(env);
    payout
}

/// Clôture anticipée par le propriétaire, au prix du marché. Le taux de sortie est le forward du jour pour la
/// durée restante, côté inverse (l'acheteur revend, le vendeur rachète) : le spread joue contre l'utilisateur
/// une deuxième fois, c'est le frais de sortie. Sans ce frais, sortir quand le marché tourne contre soi et garder
/// la couverture sinon serait une option gratuite payée par les LPs. Retourne le montant USDC versé.
pub(crate) fn close_early(env: &Env, hedge_id: u64) -> i128 {
    let key = DataKey::Hedge(hedge_id);
    let mut hedge = load_open(env, &key);
    hedge.owner.require_auth();
    let now = env.ledger().timestamp();
    if now >= hedge.maturity {
        panic_with_error!(env, HedgeError::AlreadyMatured);
    }

    // durée restante en durée « annoncée » (mode démo : × time_factor), bornée à la durée maximale du pricing
    let remaining = (hedge.maturity - now).saturating_mul(time_factor(env) as u64).min(pricing::SECS_PER_YEAR);
    let exit_side = match hedge.side {
        Side::BuyCurrency => Side::SellCurrency,
        Side::SellCurrency => Side::BuyCurrency,
    };
    let close_rate = pricing::forward_rate(env, &hedge.currency, exit_side, remaining);
    let (pnl, payout) = close(env, &key, &mut hedge, close_rate);
    HedgeClosedEarly { hedge_id, owner: hedge.owner, close_rate, pnl, payout }.publish(env);
    payout
}

/// Couverture existante et non réglée, sinon `UnknownHedge` / `AlreadySettled`.
fn load_open(env: &Env, key: &DataKey) -> Hedge {
    let hedge: Hedge = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| panic_with_error!(env, HedgeError::UnknownHedge));
    if hedge.settled {
        panic_with_error!(env, HedgeError::AlreadySettled);
    }
    hedge
}

/// Ferme la couverture au taux `rate` : P&L borné, libération de la réserve, gain ou perte du pool, versement
/// de marge ± P&L au propriétaire, `settled = true`. Retourne (P&L, versement).
fn close(env: &Env, key: &DataKey, hedge: &mut Hedge, rate: i128) -> (i128, i128) {
    let pnl = pnl(hedge.side, hedge.notional, hedge.locked_rate, rate, hedge.margin);

    vault::close_position(env, hedge);
    if pnl > 0 {
        vault::book_loss(env, pnl);
    } else if pnl < 0 {
        vault::book_gain(env, -pnl);
    }

    // marge rendue ± P&L, entre 0 et 2 × marge
    let payout = hedge.margin + pnl;
    if payout > 0 {
        let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
        token::TokenClient::new(env, &usdc).transfer(&env.current_contract_address(), &hedge.owner, &payout);
    }

    hedge.settled = true;
    env.storage().persistent().set(key, hedge);
    (pnl, payout)
}

/// P&L de l'utilisateur en USDC (7 décimales) : > 0 le pool lui doit, < 0 il doit au pool.
/// Arrondi vers le bas (en faveur du pool), borné à ±marge (le plafond).
pub(crate) fn pnl(side: Side, notional: i128, locked_rate: i128, spot: i128, margin: i128) -> i128 {
    let diff = match side {
        Side::BuyCurrency => spot - locked_rate,
        Side::SellCurrency => locked_rate - spot,
    };
    let raw = (notional * diff).div_euclid(10i128.pow(crate::PRICE_DECIMALS));
    raw.clamp(-margin, margin)
}

/// Facteur d'accélération du temps : échéance réelle = durée annoncée / facteur.
pub(crate) fn time_factor(env: &Env) -> u32 {
    env.storage().instance().get(&HedgeKey::TimeFactor).unwrap_or(1)
}

/// Admin : règle le facteur (1 = temps réel). Réservé au testnet / à la démo.
pub(crate) fn set_time_factor(env: &Env, factor: u32) {
    crate::admin(env).require_auth();
    if factor == 0 {
        panic_with_error!(env, HedgeError::InvalidTimeFactor);
    }
    env.storage().instance().set(&HedgeKey::TimeFactor, &factor);
    TimeFactorSet { factor }.publish(env);
}

pub(crate) fn get(env: &Env, hedge_id: u64) -> Option<Hedge> {
    env.storage().persistent().get(&crate::DataKey::Hedge(hedge_id))
}
