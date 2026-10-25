//! PAIEMENT — ouverture et règlement des couvertures.
//!
//! open : prix bloqué (pricing::forward_rate), contrôle du plafond (pricing::check_cap),
//!        marge = to_usdc(notional, locked_rate) × cap_bps / BPS, transférée owner → contrat,
//!        vault::reserve(marge), stockage de la position (+ extend_ttl), événement.
//! settle : après l'échéance, spot = pricing::spot ; P&L = pnl(side, notional, locked, spot, marge)
//!        (écart notional × (spot − locked), signe selon Side, borné à ±marge) ; vault::release(marge) puis book_gain / book_loss ;
//!        transfert au propriétaire de marge ± P&L ; settled = true ; événement.
//! Ensuite, côté front : path payment USDC → devise vers `supplier` pour exactement `notional`.
use crate::{pricing, vault, DataKey, Hedge, Side};
use soroban_sdk::{contractevent, token, Address, Env, Symbol};

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
        panic!("notional must be positive");
    }
    if duration_secs == 0 {
        panic!("duration must be positive");
    }
    pricing::check_cap(env, currency, cap_bps);

    let locked_rate = pricing::forward_rate(env, currency, side, duration_secs);
    // marge arrondie vers le haut
    let margin = (pricing::to_usdc(notional, locked_rate) * cap_bps as i128 + crate::BPS - 1) / crate::BPS;

    let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
    token::TokenClient::new(env, &usdc).transfer(owner, &env.current_contract_address(), &margin);
    vault::reserve(env, margin);

    let s = env.storage().instance();
    let id: u64 = s.get(&DataKey::NextId).unwrap();
    s.set(&DataKey::NextId, &(id + 1));

    let hedge = Hedge {
        owner: owner.clone(),
        supplier: supplier.clone(),
        currency: currency.clone(),
        side,
        notional,
        locked_rate,
        cap_bps,
        margin,
        maturity: env.ledger().timestamp() + duration_secs,
        settled: false,
    };
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
    let mut hedge: Hedge = env.storage().persistent().get(&key).expect("unknown hedge");
    if hedge.settled {
        panic!("already settled");
    }
    if env.ledger().timestamp() < hedge.maturity {
        panic!("not matured");
    }

    let spot = pricing::spot(env, &hedge.currency);
    let pnl = pnl(hedge.side, hedge.notional, hedge.locked_rate, spot, hedge.margin);

    vault::release(env, hedge.margin);
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
    env.storage().persistent().set(&key, &hedge);

    HedgeSettled { hedge_id, owner: hedge.owner, spot, pnl, payout }.publish(env);
    payout
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

pub(crate) fn get(env: &Env, hedge_id: u64) -> Option<Hedge> {
    env.storage().persistent().get(&crate::DataKey::Hedge(hedge_id))
}
