// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! PAIEMENT — ouverture et règlement des couvertures.
//!
//! open : prix bloqué (pricing::forward_rate), refus 210 s'il est moins bon que le taux limite signé (`worst_rate`),
//!        contrôle du plafond (pricing::check_cap),
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
//! settle_fallback : après l'échéance, admin seul, si aucun cours n'existe ni ne peut plus arriver
//!        (pricing::fixing_missing) : clôture à P&L nul, marge rendue. Sinon refus 209.
//! Ensuite, côté front : path payment USDC → devise vers `supplier` pour exactement `notional`.
//! Mode démo : `time_factor` (admin, 1 par défaut) raccourcit l'échéance réelle (`durée / facteur`)
//! sans changer le prix, calculé sur la durée annoncée. Ex. 25 920 : 90 jours → 300 s.
//! Seulement dans la version compilée avec la feature `demo` (`stellar contract build --package pool --features demo`) :
//! sans elle, `set_time_factor` est refusé (208) et `time_factor` vaut toujours 1.
//! Erreurs : `HedgeError`, codes 200+ (Vault 100+, Pricing 300+, Admin 400+). Les textes de panic! sont effacés dans le WASM :
//! seul un code d'erreur permet au front d'afficher la bonne raison d'un refus.
use crate::{pricing, vault, DataKey, Hedge, Side};
use soroban_sdk::{contractclient, contracterror, contractevent, contracttype, panic_with_error, token, vec, Address, Env, Symbol, Vec};

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
    /// `open` : le taux bloqué est moins bon que la limite signée par la PME (`worst_rate`) : plus haut pour un
    /// importateur (BuyCurrency), plus bas pour un exportateur (SellCurrency). Le cours a bougé depuis le devis.
    RateWorseThanLimit = 210,
    /// `open` par une PME hors de la liste blanche du pilote (active par défaut).
    SmeNotAllowed = 211,
    /// `open` au-delà du montant maximal par couverture du pilote (USDC au taux bloqué).
    HedgeTooLarge = 212,
    /// `open` au-delà de la durée maximale du pilote.
    HedgeTooLong = 213,
    /// `open` alors que la PME a déjà le nombre maximal de couvertures ouvertes du pilote.
    TooManyOpenHedges = 214,
    /// `set_hedge_limits` avec une limite nulle ou négative (`None` pour « sans limite »).
    InvalidLimit = 215,
    /// `settle_and_pay` : aucune route de paiement (jeton de la devise et routeur Soroswap) posée pour la devise.
    NoPaymentRoute = 216,
    /// `settle_and_pay` : l'échange coûterait plus d'USDC que le maximum signé par la PME. Rien n'est fait.
    SwapTooExpensive = 217,
    /// `settle_and_pay` : le fournisseur de cette couverture a déjà été payé.
    SupplierAlreadyPaid = 218,
    /// `settle_and_pay` sur une couverture SellCurrency : l'exportateur reçoit la devise, il n'a rien à payer.
    NotAPayment = 219,
    /// `settle_and_pay` après la date limite signée par la PME (timestamp du ledger, strictement avant). Rien n'est fait.
    DeadlinePassed = 220,
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
    /// Liste blanche des PME active (instance) ; absente = active (pilote fermé par défaut).
    SmeAllowlistOn,
    /// PME autorisée à ouvrir des couvertures (persistent, présente = autorisée).
    SmeAllowed(Address),
    /// Limites du pilote par couverture (instance) ; absentes = sans limite.
    Limits,
    /// Nombre de couvertures ouvertes d'une PME (persistent).
    OpenCount(Address),
    /// Route de paiement du fournisseur pour une devise (persistent).
    Route(Symbol),
    /// Fournisseur de la couverture payé par `settle_and_pay` (persistent, présent = payé).
    SupplierPaid(u64),
}

/// Route de paiement du fournisseur dans une devise : jeton de la devise (SAC) et routeur Soroswap qui l'échange
/// contre de l'USDC. Posée par l'admin, pour qu'une PME ne puisse pas faire payer son fournisseur en faux jeton.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRoute {
    pub token: Address,
    pub router: Address,
}

/// Interface du routeur Soroswap (`soroswap/core`, contracts/router), pour les deux fonctions utilisées.
/// `to` signe et paie l'entrée (`amounts[0]`), et reçoit la sortie. Échéance : timestamp du ledger, strictement après.
#[allow(dead_code)]
#[contractclient(name = "SoroswapRouterClient")]
pub trait SoroswapRouter {
    fn router_get_amounts_in(env: Env, amount_out: i128, path: Vec<Address>) -> Vec<i128>;
    fn swap_tokens_for_exact_tokens(
        env: Env,
        amount_out: i128,
        amount_in_max: i128,
        path: Vec<Address>,
        to: Address,
        deadline: u64,
    ) -> Vec<i128>;
}

/// Fournisseur payé dans la transaction du règlement : `amount` en devise (exactement le notionnel), contre
/// `usdc_spent` USDC échangés sur Soroswap.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplierPaid {
    #[topic]
    pub hedge_id: u64,
    #[topic]
    pub owner: Address,
    pub supplier: Address,
    pub amount: i128,
    pub usdc_spent: i128,
}

/// L'admin a posé la route de paiement d'une devise.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRouteSet {
    #[topic]
    pub currency: Symbol,
    pub token: Address,
    pub router: Address,
}

/// Limites du pilote côté couvertures. `None` = sans limite.
#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HedgeLimits {
    /// Montant maximal d'une couverture, USDC 7 décimales au taux bloqué.
    pub max_hedge_notional: Option<i128>,
    /// Durée maximale d'une couverture (durée annoncée), en secondes.
    pub max_duration_secs: Option<u64>,
    /// Nombre maximal de couvertures ouvertes en même temps par une PME.
    pub max_open_per_sme: Option<u32>,
}

/// L'admin a ajouté (`allowed`) ou retiré une PME de la liste blanche.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmeAllowedSet {
    #[topic]
    pub sme: Address,
    pub allowed: bool,
}

/// L'admin a changé les limites du pilote par couverture.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HedgeLimitsSet {
    pub max_hedge_notional: Option<i128>,
    pub max_duration_secs: Option<u64>,
    pub max_open_per_sme: Option<u32>,
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
    worst_rate: Option<i128>,
) -> u64 {
    owner.require_auth();
    if !is_sme_allowed(env, owner) {
        panic_with_error!(env, HedgeError::SmeNotAllowed);
    }
    let limits = limits(env);
    if notional <= 0 {
        panic_with_error!(env, HedgeError::InvalidNotional);
    }
    if duration_secs == 0 {
        panic_with_error!(env, HedgeError::InvalidDuration);
    }
    pricing::check_cap(env, currency, cap_bps);
    if limits.max_duration_secs.is_some_and(|max| duration_secs > max) {
        panic_with_error!(env, HedgeError::HedgeTooLong);
    }
    let open_count = open_count(env, owner);
    if limits.max_open_per_sme.is_some_and(|max| open_count >= max) {
        panic_with_error!(env, HedgeError::TooManyOpenHedges);
    }

    let locked_rate = pricing::forward_rate(env, currency, side, duration_secs);
    // taux limite signé par la PME : protège contre un mouvement entre le devis affiché et l'exécution
    if let Some(limit) = worst_rate {
        let worse = match side {
            Side::BuyCurrency => locked_rate > limit,
            Side::SellCurrency => locked_rate < limit,
        };
        if worse {
            panic_with_error!(env, HedgeError::RateWorseThanLimit);
        }
    }
    if limits.max_hedge_notional.is_some_and(|max| pricing::to_usdc(env, notional, locked_rate) > max) {
        panic_with_error!(env, HedgeError::HedgeTooLarge);
    }
    set_open_count(env, owner, open_count + 1);
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

/// Voie de secours, **admin seul** : clôt une couverture échue dont le cours d'échéance est définitivement
/// absent (rien de figé, aucun cours dans l'heure après l'échéance, ou historique de l'oracle oublié).
/// P&L nul : la marge est rendue au propriétaire, la réserve du pool libérée, personne ne gagne ni ne perd.
/// Refus `FixingAvailable` (209) tant qu'un cours existe ou peut encore arriver : il faut alors `settle`.
/// Retourne le montant USDC versé (la marge).
pub(crate) fn settle_fallback(env: &Env, hedge_id: u64) -> i128 {
    // réservée à l'admin (relecture de Hugo, 25/11) : ouverte à tous, ce serait une option gratuite pour le perdant,
    // qui n'aurait qu'à attendre que personne ne fige le cours pendant 24 h
    crate::admin(env).require_auth();
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
    set_open_count(env, &hedge.owner, open_count(env, &hedge.owner).saturating_sub(1));
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

// ---- régler et payer le fournisseur en une transaction (dossier Instaward, livrable 2) ----

/// Admin : route de paiement d'une devise (jeton SAC de la devise, routeur Soroswap).
pub(crate) fn set_payment_route(env: &Env, currency: &Symbol, token: &Address, router: &Address) {
    crate::admin(env).require_auth();
    let key = HedgeKey::Route(currency.clone());
    let route = PaymentRoute { token: token.clone(), router: router.clone() };
    env.storage().persistent().set(&key, &route);
    env.storage().persistent().extend_ttl(&key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);
    PaymentRouteSet { currency: currency.clone(), token: token.clone(), router: router.clone() }.publish(env);
}

pub(crate) fn payment_route(env: &Env, currency: &Symbol) -> Option<PaymentRoute> {
    env.storage().persistent().get(&HedgeKey::Route(currency.clone()))
}

/// Règle la couverture (si ce n'est pas déjà fait) et paie au fournisseur **exactement** le montant de la facture
/// en devise, dans une seule transaction signée par la PME :
/// 1. `settle` crédite marge ± P&L au solde réclamable (sauté si le robot a déjà réglé) ;
/// 2. le solde réclamable est versé à la PME ;
/// 3. Soroswap échange ses USDC contre exactement `notional` en devise (`to` = la PME, qui signe et paie l'entrée),
///    sans dépasser `max_usdc` et avant `deadline` (tous deux signés par la PME) : sinon refus 217 / 220, rien n'est fait ;
/// 4. la devise part de la PME au fournisseur.
/// Tout ou rien : si une étape échoue, la transaction est annulée et la couverture reste réglable par `settle`.
/// Le contrat ne signe jamais pour son propre compte. Retourne l'USDC dépensé dans l'échange.
pub(crate) fn settle_and_pay(env: &Env, hedge_id: u64, max_usdc: i128, deadline: u64) -> i128 {
    let key = DataKey::Hedge(hedge_id);
    let hedge: Hedge = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, HedgeError::UnknownHedge));
    hedge.owner.require_auth();
    if env.ledger().timestamp() >= deadline {
        panic_with_error!(env, HedgeError::DeadlinePassed);
    }
    if hedge.side != Side::BuyCurrency {
        panic_with_error!(env, HedgeError::NotAPayment);
    }
    let route = payment_route(env, &hedge.currency).unwrap_or_else(|| panic_with_error!(env, HedgeError::NoPaymentRoute));
    let paid_key = HedgeKey::SupplierPaid(hedge_id);
    if env.storage().persistent().has(&paid_key) {
        panic_with_error!(env, HedgeError::SupplierAlreadyPaid);
    }
    if !hedge.settled {
        settle(env, hedge_id);
    }

    let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    let router = SoroswapRouterClient::new(env, &route.router);
    let path = vec![env, usdc, route.token.clone()];
    let needed = router.router_get_amounts_in(&hedge.notional, &path).get(0).unwrap();
    if needed > max_usdc {
        panic_with_error!(env, HedgeError::SwapTooExpensive);
    }

    pay_out(env, &hedge.owner);
    // date limite **signée par la PME** : elle signe l'appel au routeur avec ses arguments exacts, une date calculée
    // ici à l'exécution ne correspondrait jamais à celle de la simulation (relecture de Hugo)
    let amounts = router.swap_tokens_for_exact_tokens(&hedge.notional, &max_usdc, &path, &hedge.owner, &deadline);
    token::TokenClient::new(env, &route.token).transfer(&hedge.owner, &hedge.supplier, &hedge.notional);

    env.storage().persistent().set(&paid_key, &true);
    env.storage().persistent().extend_ttl(&paid_key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);
    let usdc_spent = amounts.get(0).unwrap();
    SupplierPaid { hedge_id, owner: hedge.owner, supplier: hedge.supplier, amount: hedge.notional, usdc_spent }.publish(env);
    usdc_spent
}

// ---- pilote côté couvertures (B9) ----

/// Liste blanche des PME active ? Vrai tant que l'admin ne l'a pas désactivée.
pub(crate) fn sme_allowlist_on(env: &Env) -> bool {
    env.storage().instance().get(&HedgeKey::SmeAllowlistOn).unwrap_or(true)
}

/// Admin : active ou désactive la liste blanche des PME.
pub(crate) fn set_sme_allowlist_enabled(env: &Env, enabled: bool) {
    crate::admin(env).require_auth();
    env.storage().instance().set(&HedgeKey::SmeAllowlistOn, &enabled);
    vault::AllowlistToggled { role: crate::Role::Sme, enabled }.publish(env);
}

/// La PME peut-elle ouvrir ? Toujours vrai quand la liste est désactivée.
pub(crate) fn is_sme_allowed(env: &Env, sme: &Address) -> bool {
    !sme_allowlist_on(env) || env.storage().persistent().has(&HedgeKey::SmeAllowed(sme.clone()))
}

/// Admin : ajoute ou retire une PME. Le retrait ne touche pas à ses couvertures ouvertes, qui se règlent normalement.
pub(crate) fn set_sme_allowed(env: &Env, sme: &Address, allowed: bool) {
    crate::admin(env).require_auth();
    let key = HedgeKey::SmeAllowed(sme.clone());
    if allowed {
        env.storage().persistent().set(&key, &true);
        env.storage().persistent().extend_ttl(&key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);
    } else {
        env.storage().persistent().remove(&key);
    }
    SmeAllowedSet { sme: sme.clone(), allowed }.publish(env);
}

/// Limites du pilote par couverture (toutes à `None` par défaut).
pub(crate) fn limits(env: &Env) -> HedgeLimits {
    env.storage().instance().get(&HedgeKey::Limits).unwrap_or_default()
}

/// Admin : pose les limites du pilote par couverture. `None` = sans limite ; une limite à 0 est refusée (215).
/// Contrôlées à l'ouverture seulement : une couverture ouverte se règle toujours.
pub(crate) fn set_limits(
    env: &Env,
    max_hedge_notional: Option<i128>,
    max_duration_secs: Option<u64>,
    max_open_per_sme: Option<u32>,
) {
    crate::admin(env).require_auth();
    if max_hedge_notional.is_some_and(|v| v <= 0)
        || max_duration_secs == Some(0)
        || max_open_per_sme == Some(0)
    {
        panic_with_error!(env, HedgeError::InvalidLimit);
    }
    let limits = HedgeLimits { max_hedge_notional, max_duration_secs, max_open_per_sme };
    env.storage().instance().set(&HedgeKey::Limits, &limits);
    HedgeLimitsSet { max_hedge_notional, max_duration_secs, max_open_per_sme }.publish(env);
}

fn open_count(env: &Env, owner: &Address) -> u32 {
    env.storage().persistent().get(&HedgeKey::OpenCount(owner.clone())).unwrap_or(0)
}

fn set_open_count(env: &Env, owner: &Address, count: u32) {
    let key = HedgeKey::OpenCount(owner.clone());
    if count == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &count);
        env.storage().persistent().extend_ttl(&key, HEDGE_TTL_THRESHOLD, HEDGE_TTL_EXTEND_TO);
    }
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
