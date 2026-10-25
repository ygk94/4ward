//! PAIEMENT — ouverture et règlement des couvertures.
//!
//! open : prix bloqué (pricing::forward_rate), contrôle du plafond (pricing::check_cap),
//!        marge = to_usdc(notional, locked_rate) × cap_bps / BPS, transférée owner → contrat,
//!        vault::reserve(marge), stockage de la position (+ extend_ttl), événement.
//! settle : après l'échéance, spot = pricing::spot ; écart = to_usdc(notional, spot) - to_usdc(notional, locked) ;
//!        signe selon Side ; borné à ±marge ; vault::release(marge) puis book_gain / book_loss ;
//!        transfert au propriétaire de marge ± P&L ; settled = true ; événement.
//! Ensuite, côté front : path payment USDC → devise vers `supplier` pour exactement `notional`.
use crate::{Hedge, Side};
use soroban_sdk::{Address, Env, Symbol};

pub(crate) fn open(
    _env: &Env,
    _owner: &Address,
    _supplier: &Address,
    _currency: &Symbol,
    _side: Side,
    _notional: i128,
    _duration_secs: u64,
    _cap_bps: u32,
) -> u64 {
    panic!("TODO paiement")
}

pub(crate) fn settle(_env: &Env, _hedge_id: u64) -> i128 {
    panic!("TODO paiement")
}

pub(crate) fn get(env: &Env, hedge_id: u64) -> Option<Hedge> {
    env.storage().persistent().get(&crate::DataKey::Hedge(hedge_id))
}
