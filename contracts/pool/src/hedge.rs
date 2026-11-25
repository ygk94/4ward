// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! PAIEMENT — ouverture et règlement des couvertures.
//!
//! open : prix bloqué (pricing::forward_rate), contrôle du plafond (pricing::check_cap),
//!        marge = to_usdc(notional, locked_rate) × cap_bps / BPS, transférée owner → contrat,
//!        vault::open_position(&hedge) (réserve la marge, exposition par devise), stockage (+ extend_ttl), événement.
//! settle : après l'échéance, spot = pricing::settlement_fixing(maturity) (cours en vigueur à l'échéance) ; P&L = pnl(side, notional, locked, spot, marge)
//!        (écart notional × (spot − locked), signe selon Side, borné à ±marge) ; vault::close_position(&hedge) puis book_gain / book_loss ;
//!        marge ± P&L **crédités** au solde réclamable du propriétaire (rien ne lui est envoyé) ; settled = true ; événement.
//! claim / claim_for : versent au propriétaire tout son solde réclamable (claim : signé par lui ; claim_for : sans
//!        signature, pour le robot). Le règlement ne dépend ainsi jamais du compte de la PME : une ligne USDC supprimée
//!        ou un compte bloqué n'empêche pas de régler, et ne retarde que son propre versement.
//! close_early : avant l'échéance, le propriétaire seul. Clôture au prix du marché : taux de sortie = forward du jour
//!        pour la durée restante, dans le sens inverse (spread contre l'utilisateur, comme à l'ouverture : c'est le
//!        frais de sortie) ; même P&L borné, même versement que settle. Le perdant paie, le pool libère sa réserve.
//! record_fixing : après l'échéance, sans signature, fige le cours d'échéance (pricing::record_fixing) pour que la
//!        couverture reste réglable après l'oubli de l'historique de l'oracle (~24 h sur Reflector). Pour le robot.
//! settle_fallback : après l'échéance, sans signature, si aucun cours n'existe ni ne peut plus arriver
//!        (pricing::fixing_missing) : clôture à P&L nul, marge rendue. Sinon refus 209.
//! Ensuite, côté front : path payment USDC → devise vers `supplier` pour exactement `notional`.
//! Mode démo : `time_factor` (admin, 1 par défaut) raccourcit l'échéance réelle (`durée / facteur`)
//! sans changer le prix, calculé sur la durée annoncée. Ex. 25 920 : 90 jours → 300 s.
//! Seulement dans la version compilée avec la feature `demo` (`stellar contract build --package pool --features demo`) :
//! sans elle, `set_time_factor` est refusé (208) et `time_factor` vaut toujours 1.
//! Erreurs : `HedgeError`, codes 200+ (Vault 100+, Pricing 300+, Admin 400+). Les textes de panic! sont effacés dans le WASM :
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
    /// Calcul du P&L hors de toute plage réaliste (notionnel × écart de cours dépasse i128) : refus avec un code
    /// au lieu d'un piège de la VM sans explication.
    Overflow = 207,
    /// `set_time_factor` sur une version compilée sans la feature `demo` : le mode démo n'existe pas en production.
    DemoDisabled = 208,
    /// `settle_fallback` alors qu'un cours d'échéance existe (figé ou lisible) ou peut encore être publié
    /// (fenêtre d'une heure après l'échéance) : régler avec `settle`, ou attendre.
    FixingAvailable = 209,
}

/// Clés de stockage de Paiement (à part de `DataKey`, partagé).
#[contracttype]
enum HedgeKey {
    /// Facteur d'accélération du temps (mode démo), 1 par défaut.
    TimeFactor,
    /// Solde réclamable d'un propriétaire en USDC (7 décimales) : versements de règlement pas encore réclamés.
    Claimable(Address),
    /// Σ des soldes réclamables : USDC du contrat qui n'appartient ni aux LPs ni aux marges ouvertes.
    TotalClaimable,
}

/// Solde réclamable versé à son propriétaire par `claim` ou `claim_for`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub owner: Address,
    pub amount: i128,
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

/// Couverture close par la voie de secours, faute de cours d'échéance : P&L nul, marge rendue telle quelle.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HedgeFallbackSettled {
    #[topic]
    pub hedge_id: u64,
    #[topic]
    pub owner: Address,
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
    let margin = pricing::margin_for(env, pricing::to_usdc(env, notional, locked_rate), cap_bps);

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
    // cours figé, sinon lu puis figé ; aucun cours encore : 312, la couverture reste ouverte
    let spot = pricing::settlement_fixing(env, hedge_id, &hedge.currency, hedge.maturity);
    let (pnl, payout) = close(env, &key, &mut hedge, spot, Payout::Credit);
    HedgeSettled { hedge_id, owner: hedge.owner, spot, pnl, payout }.publish(env);
    payout
}

/// Fige le cours d'échéance d'une couverture échue, sans la régler, et le renvoie. Sans signature (robot).
/// Reflector oublie ses cours au bout d'environ 24 h : appelé peu après l'échéance, il garantit que la couverture
/// reste réglable au bon cours même si `settle` passe plus tard. Répétable (renvoie le cours déjà figé).
/// Erreurs : `UnknownHedge`, `AlreadySettled`, `NotMatured`, et `NoFixing` (312) tant qu'aucun cours n'est lisible.
pub(crate) fn record_fixing(env: &Env, hedge_id: u64) -> i128 {
    let hedge = load_open(env, &DataKey::Hedge(hedge_id));
    if env.ledger().timestamp() < hedge.maturity {
        panic_with_error!(env, HedgeError::NotMatured);
    }
    pricing::record_fixing(env, hedge_id, &hedge.currency, hedge.maturity)
}

/// Voie de secours, sans signature : clôt une couverture échue dont le cours d'échéance est définitivement
/// absent (rien de figé, aucun cours dans l'heure après l'échéance, ou historique de l'oracle oublié).
/// P&L nul : la marge est rendue au propriétaire, la réserve du pool libérée, personne ne gagne ni ne perd.
/// Refus `FixingAvailable` (209) tant qu'un cours existe ou peut encore arriver : il faut alors `settle`.
/// Retourne le montant USDC versé (la marge).
pub(crate) fn settle_fallback(env: &Env, hedge_id: u64) -> i128 {
    let key = DataKey::Hedge(hedge_id);
    let mut hedge = load_open(env, &key);
    if env.ledger().timestamp() < hedge.maturity {
        panic_with_error!(env, HedgeError::NotMatured);
    }
    if !pricing::fixing_missing(env, hedge_id, &hedge.currency, hedge.maturity) {
        panic_with_error!(env, HedgeError::FixingAvailable);
    }
    // au taux bloqué, le P&L est nul par construction
    let rate = hedge.locked_rate;
    let (_, payout) = close(env, &key, &mut hedge, rate, Payout::Credit);
    HedgeFallbackSettled { hedge_id, owner: hedge.owner, payout }.publish(env);
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
    // signée par le propriétaire : il ne peut bloquer que sa propre transaction, on lui verse directement
    let (pnl, payout) = close(env, &key, &mut hedge, close_rate, Payout::Transfer);
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

/// Où va le versement d'une couverture close.
enum Payout {
    /// Crédité au solde réclamable (règlements sans signature : ne dépend pas du compte du propriétaire).
    Credit,
    /// Envoyé au propriétaire (clôture qu'il a lui-même signée).
    Transfer,
}

/// Ferme la couverture au taux `rate` : P&L borné, libération de la réserve, gain ou perte du pool, versement
/// de marge ± P&L au propriétaire (crédité ou envoyé selon `payout_to`), `settled = true`. Retourne (P&L, versement).
fn close(env: &Env, key: &DataKey, hedge: &mut Hedge, rate: i128, payout_to: Payout) -> (i128, i128) {
    let pnl = pnl(env, hedge.side, hedge.notional, hedge.locked_rate, rate, hedge.margin);

    vault::close_position(env, hedge);
    if pnl > 0 {
        vault::book_loss(env, pnl);
    } else if pnl < 0 {
        vault::book_gain(env, -pnl);
    }

    // marge rendue ± P&L, entre 0 et 2 × marge
    let payout = hedge.margin + pnl;
    if payout > 0 {
        match payout_to {
            Payout::Credit => credit(env, &hedge.owner, payout),
            Payout::Transfer => transfer_out(env, &hedge.owner, payout),
        }
    }

    hedge.settled = true;
    env.storage().persistent().set(key, hedge);
    (pnl, payout)
}

// ---- solde réclamable (B4) ----

fn transfer_out(env: &Env, to: &Address, amount: i128) {
    let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    token::TokenClient::new(env, &usdc).transfer(&env.current_contract_address(), to, &amount);
}

fn checked_add(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_add(b).unwrap_or_else(|| panic_with_error!(env, HedgeError::Overflow))
}

/// Ajoute `amount` au solde réclamable de `owner` et au total.
fn credit(env: &Env, owner: &Address, amount: i128) {
    let key = HedgeKey::Claimable(owner.clone());
    let p = env.storage().persistent();
    let balance = checked_add(env, p.get(&key).unwrap_or(0), amount);
    p.set(&key, &balance);
    p.extend_ttl(&key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);
    let total = checked_add(env, total_claimable(env), amount);
    env.storage().instance().set(&HedgeKey::TotalClaimable, &total);
}

/// Solde réclamable de `owner` (0 s'il n'a rien).
pub(crate) fn claimable(env: &Env, owner: &Address) -> i128 {
    env.storage().persistent().get(&HedgeKey::Claimable(owner.clone())).unwrap_or(0)
}

/// Σ des soldes réclamables.
pub(crate) fn total_claimable(env: &Env) -> i128 {
    env.storage().instance().get(&HedgeKey::TotalClaimable).unwrap_or(0)
}

/// Verse à `owner` tout son solde réclamable et le remet à zéro. Rien à verser : 0, sans transfert.
/// Si le transfert échoue (ligne USDC supprimée, compte bloqué), toute la transaction est annulée et le solde reste dû.
fn pay_out(env: &Env, owner: &Address) -> i128 {
    let amount = claimable(env, owner);
    if amount <= 0 {
        return 0;
    }
    env.storage().persistent().remove(&HedgeKey::Claimable(owner.clone()));
    env.storage().instance().set(&HedgeKey::TotalClaimable, &(total_claimable(env) - amount));
    transfer_out(env, owner, amount);
    Claimed { owner: owner.clone(), amount }.publish(env);
    amount
}

/// Le propriétaire réclame son solde (signé par lui). Retourne l'USDC versé.
pub(crate) fn claim(env: &Env, owner: &Address) -> i128 {
    owner.require_auth();
    pay_out(env, owner)
}

/// N'importe qui pousse le solde de `owner` vers `owner` (robot), sans signature. Retourne l'USDC versé.
pub(crate) fn claim_for(env: &Env, owner: &Address) -> i128 {
    pay_out(env, owner)
}

/// P&L de l'utilisateur en USDC (7 décimales) : > 0 le pool lui doit, < 0 il doit au pool.
/// Arrondi vers le bas (en faveur du pool), borné à ±marge (le plafond). `Overflow` (207) si le calcul dépasse i128.
pub(crate) fn pnl(env: &Env, side: Side, notional: i128, locked_rate: i128, spot: i128, margin: i128) -> i128 {
    let diff = match side {
        Side::BuyCurrency => spot.checked_sub(locked_rate),
        Side::SellCurrency => locked_rate.checked_sub(spot),
    };
    let raw = diff
        .and_then(|d| notional.checked_mul(d))
        .unwrap_or_else(|| panic_with_error!(env, HedgeError::Overflow))
        .div_euclid(10i128.pow(crate::PRICE_DECIMALS));
    raw.clamp(-margin, margin)
}

/// Facteur d'accélération du temps : échéance réelle = durée annoncée / facteur.
/// Toujours 1 sans la feature `demo` : une version de production a des échéances réelles, quoi qu'il y ait
/// dans le stockage (par exemple après une mise à jour du code depuis une version de démo).
pub(crate) fn time_factor(env: &Env) -> u32 {
    if !cfg!(feature = "demo") {
        return 1;
    }
    env.storage().instance().get(&HedgeKey::TimeFactor).unwrap_or(1)
}

/// Admin : règle le facteur (1 = temps réel). N'existe que dans la version de démo (feature `demo`) :
/// ailleurs, refus `DemoDisabled` (208).
pub(crate) fn set_time_factor(env: &Env, factor: u32) {
    if !cfg!(feature = "demo") {
        panic_with_error!(env, HedgeError::DemoDisabled);
    }
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
