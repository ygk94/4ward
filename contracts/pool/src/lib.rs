// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
#![no_std]
//! Contrat principal : pool USDC des LPs + couvertures de change plafonnées.
//!
//! Découpage par personne (voir TODO.md) :
//! - `vault.rs`   — Vault : dépôts / retraits LP, capital réservé, flux USDC du pool
//! - `pricing.rs` — Pricing : spot oracle, taux forward, spread, plafonds minimum
//! - `hedge.rs`   — Paiement : ouverture et règlement des couvertures (marge, P&L, restitution)
//!
//! Ce fichier ne contient que les points d'entrée publics et les types partagés.
//! Les fonctions `pub(crate)` entre modules sont le contrat d'interface entre vous :
//! ne pas changer leur signature sans prévenir les deux autres.
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, Symbol};

mod admin;
mod hedge;
mod pricing;
mod vault;

/// Décimales de l'oracle (Reflector FX) : prix en USD par unité de devise.
pub const PRICE_DECIMALS: u32 = 14;
/// Décimales des montants (USDC et notionnels en devise, comme les actifs Stellar).
pub const AMOUNT_DECIMALS: u32 = 7;
pub const BPS: i128 = 10_000;

/// Sens de la couverture, vu par l'utilisateur.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// Doit PAYER la devise (importateur) : protégé si la devise monte.
    BuyCurrency,
    /// Va RECEVOIR la devise (exportateur) : protégé si la devise baisse.
    SellCurrency,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quote {
    /// Spot de l'oracle (USD par unité, 14 décimales).
    pub spot: i128,
    /// Taux forward spread inclus (USD par unité, 14 décimales) = taux bloqué.
    pub forward_rate: i128,
    /// Coût total en USDC (7 décimales) au taux bloqué : notional × forward_rate.
    pub usdc_cost: i128,
    /// Marge à déposer en USDC (7 décimales) pour le plafond demandé.
    pub margin: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hedge {
    pub owner: Address,
    /// Fournisseur à payer en devise à l'échéance (utilisé par le front pour le path payment).
    pub supplier: Address,
    pub currency: Symbol,
    pub side: Side,
    /// Montant en devise, 7 décimales (1 000 000 MXN = 10_000_000_000_000).
    pub notional: i128,
    /// Taux bloqué (USD par unité, 14 décimales).
    pub locked_rate: i128,
    /// Plafond = marge, en points de base (1000 = 10 %).
    pub cap_bps: u32,
    /// Marge déposée en USDC (7 décimales). Égale au capital réservé dans le vault.
    pub margin: i128,
    /// Échéance, en secondes Unix (`env.ledger().timestamp()`).
    pub maturity: u64,
    pub settled: bool,
}

/// Liste blanche concernée par `set_allowed` / `is_allowed` / `set_allowlist_enabled` (B9).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Apporteur de capital (dépôts junior et senior) : vault.rs.
    Lp,
    /// PME qui ouvre des couvertures : hedge.rs (moitié de Clément, pas encore branchée).
    Sme,
}

/// Réglages et compteurs du pilote (B9), en lecture pour le front.
/// Les champs côté couvertures (liste PME, plafonds par couverture et de durée, nombre maximal
/// de couvertures ouvertes) seront ajoutés par la moitié hedge de B9.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PilotLimits {
    /// Liste blanche des LP active (true par défaut : pilote fermé).
    pub lp_allowlist_on: bool,
    /// Plafond du notionnel total ouvert, USDC 7 décimales ; `None` = sans plafond.
    pub notional_cap: Option<i128>,
    /// Σ notionnels des couvertures ouvertes, USDC 7 décimales au taux bloqué.
    pub total_notional: i128,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Usdc,
    Oracle,
    // -- pricing
    SpreadBps,
    /// Taux d'intérêt annuel par devise, en bps. `USD` inclus.
    Rate(Symbol),
    /// Plafond minimum par devise, en bps.
    MinCap(Symbol),
    // -- vault
    LpBalance(Address),
    TotalDeposits,
    /// Σ des marges des couvertures ouvertes = perte max du pool.
    Reserved,
    // -- hedge
    NextId,
    Hedge(u64),
}

pub(crate) fn admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

#[contract]
pub struct Pool;

#[contractimpl]
impl Pool {
    pub fn __constructor(env: Env, admin: Address, usdc: Address, oracle: Address, spread_bps: u32) {
        admin::check_oracle_decimals(&env, &oracle);
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Usdc, &usdc);
        s.set(&DataKey::Oracle, &oracle);
        s.set(&DataKey::SpreadBps, &spread_bps);
        s.set(&DataKey::TotalDeposits, &0i128);
        s.set(&DataKey::Reserved, &0i128);
        s.set(&DataKey::NextId, &0u64);
    }

    // ---- Vault ----

    /// Dépose `amount` USDC (7 décimales) dans le pool contre des parts au prix plein.
    /// Retourne le nombre de parts du LP après le dépôt (7 décimales).
    pub fn deposit_lp(env: Env, lp: Address, amount: i128) -> i128 {
        admin::require_not_paused(&env);
        vault::deposit(&env, &lp, amount)
    }

    /// Brûle `amount` PARTS du LP et lui verse leur valeur au prix de sortie
    /// ((valeur junior - Reserved) / TotalShares), donc jamais plus que le capital libre.
    /// Retourne l'USDC versé (7 décimales).
    pub fn withdraw(env: Env, lp: Address, amount: i128) -> i128 {
        vault::withdraw(&env, &lp, amount)
    }

    /// Capital encore disponible pour de nouvelles couvertures.
    pub fn capacity(env: Env) -> i128 {
        vault::capacity(&env)
    }

    /// Valeur en USDC des parts du LP au prix de sortie (ce qu'il recevrait en retirant tout).
    pub fn lp_balance(env: Env, lp: Address) -> i128 {
        vault::balance(&env, &lp)
    }

    /// Parts détenues par le LP (7 décimales).
    pub fn lp_shares(env: Env, lp: Address) -> i128 {
        vault::lp_shares(&env, &lp)
    }

    /// Prix plein d'une part junior en USDC (7 décimales) : valeur junior / TotalShares.
    pub fn share_price(env: Env) -> i128 {
        vault::share_price(&env)
    }

    /// Prix de sortie d'une part junior en USDC (7 décimales) : (valeur junior - Reserved) / TotalShares.
    pub fn exit_price(env: Env) -> i128 {
        vault::exit_price(&env)
    }

    /// Capital des LPs, senior + junior (TotalDeposits), en USDC.
    pub fn total_assets(env: Env) -> i128 {
        vault::total_assets(&env)
    }

    /// Dépose `amount` USDC dans la tranche senior (rendement fixe, protégée par le junior).
    /// Retourne le nombre de parts senior du LP après le dépôt (7 décimales).
    pub fn deposit_senior(env: Env, lp: Address, amount: i128) -> i128 {
        admin::require_not_paused(&env);
        vault::deposit_senior(&env, &lp, amount)
    }

    /// Brûle `shares` PARTS senior du LP et lui verse leur valeur au prix courant.
    /// Retourne l'USDC versé (7 décimales).
    pub fn withdraw_senior(env: Env, lp: Address, shares: i128) -> i128 {
        vault::withdraw_senior(&env, &lp, shares)
    }

    /// Valeur en USDC des parts senior du LP au prix courant.
    pub fn senior_balance(env: Env, lp: Address) -> i128 {
        vault::senior_balance(&env, &lp)
    }

    /// Valeur des deux tranches (senior + junior = total_assets) et prix d'une part senior.
    /// `senior` est la créance senior (ce qui est dû) ; ce qui peut sortir aujourd'hui est
    /// `senior` − `senior_arrears()`.
    pub fn tranche_values(env: Env) -> vault::TrancheValues {
        vault::tranche_values(&env)
    }

    /// Intérêts senior en retard de service (USDC, 7 décimales) : la part de la créance senior que
    /// le junior ne peut pas payer sans entamer le capital réservé. 0 en temps normal.
    pub fn senior_arrears(env: Env) -> i128 {
        vault::senior_arrears(&env)
    }

    /// Dépôts senior ouverts ? Fermés par défaut.
    pub fn senior_deposits_open(env: Env) -> bool {
        vault::senior_deposits_open(&env)
    }

    /// Admin : ouvre ou ferme les dépôts senior. Les retraits senior restent toujours possibles.
    pub fn set_senior_deposits_open(env: Env, open: bool) {
        vault::set_senior_deposits_open(&env, open)
    }

    /// Exposition du pool sur une devise (couvertures ouvertes) : volumes acheteur / vendeur
    /// en USDC au taux bloqué, net = acheteur − vendeur, et marges de chaque côté.
    pub fn exposure(env: Env, currency: Symbol) -> vault::Exposure {
        vault::exposure(&env, &currency)
    }

    /// Limite de la devise en bps du capital (Σ marges de la devise), `None` = sans limite.
    pub fn currency_limit(env: Env, currency: Symbol) -> Option<u32> {
        vault::currency_limit(&env, &currency)
    }

    /// Admin : Σ marges ouvertes d'une devise ≤ `limit_bps` × capital / 10 000.
    /// 0 = plus de nouvelles couvertures sur cette devise.
    pub fn set_currency_limit(env: Env, currency: Symbol, limit_bps: u32) {
        vault::set_currency_limit(&env, &currency, limit_bps)
    }

    // ---- Pilote (B9) ----

    /// Admin : ajoute (`true`) ou retire (`false`) `who` de la liste blanche du rôle.
    /// Un LP retiré garde ses parts et peut toujours retirer.
    pub fn set_allowed(env: Env, role: Role, who: Address, allowed: bool) {
        match role {
            Role::Lp => vault::set_lp_allowed(&env, &who, allowed),
            Role::Sme => admin::sme_not_implemented(&env),
        }
    }

    /// `who` peut-il agir pour ce rôle ? Toujours vrai quand la liste du rôle est désactivée.
    pub fn is_allowed(env: Env, role: Role, who: Address) -> bool {
        match role {
            Role::Lp => vault::is_lp_allowed(&env, &who),
            Role::Sme => admin::sme_not_implemented(&env),
        }
    }

    /// Admin : active (pilote fermé, par défaut) ou désactive la liste blanche du rôle.
    pub fn set_allowlist_enabled(env: Env, role: Role, enabled: bool) {
        match role {
            Role::Lp => vault::set_lp_allowlist_enabled(&env, enabled),
            Role::Sme => admin::sme_not_implemented(&env),
        }
    }

    /// Admin : plafond du notionnel total ouvert (USDC, 7 décimales), `None` = sans plafond.
    /// Contrôlé à l'ouverture seulement : un règlement n'est jamais bloqué.
    pub fn set_notional_cap(env: Env, cap: Option<i128>) {
        vault::set_notional_cap(&env, cap)
    }

    /// Admin : plafond du capital réservé (Σ marges des couvertures ouvertes) par propriétaire,
    /// USDC 7 décimales, `None` = sans plafond. Contrôlé à l'ouverture seulement (erreur 120).
    pub fn set_owner_reserve_cap(env: Env, cap: Option<i128>) {
        vault::set_owner_reserve_cap(&env, cap)
    }

    /// Plafond du capital réservé par propriétaire ; `None` = sans plafond.
    pub fn owner_reserve_cap(env: Env) -> Option<i128> {
        vault::owner_reserve_cap(&env)
    }

    /// Capital réservé (Σ marges) des couvertures ouvertes de `owner`, USDC 7 décimales.
    pub fn owner_reserved(env: Env, owner: Address) -> i128 {
        vault::owner_reserved(&env, &owner)
    }

    /// Σ notionnels des couvertures ouvertes, USDC 7 décimales au taux bloqué.
    pub fn total_notional(env: Env) -> i128 {
        vault::total_notional(&env)
    }

    pub fn pilot_limits(env: Env) -> PilotLimits {
        PilotLimits {
            lp_allowlist_on: vault::lp_allowlist_on(&env),
            notional_cap: vault::notional_cap(&env),
            total_notional: vault::total_notional(&env),
        }
    }

    // ---- Pricing ----

    pub fn quote(env: Env, currency: Symbol, side: Side, notional: i128, duration_secs: u64, cap_bps: u32) -> Quote {
        pricing::quote(&env, &currency, side, notional, duration_secs, cap_bps)
    }

    pub fn set_rate(env: Env, currency: Symbol, rate_bps: u32) {
        pricing::set_rate(&env, &currency, rate_bps)
    }

    pub fn set_min_cap(env: Env, currency: Symbol, cap_bps: u32) {
        pricing::set_min_cap(&env, &currency, cap_bps)
    }

    pub fn set_spread(env: Env, spread_bps: u32) {
        pricing::set_spread(&env, spread_bps)
    }

    // ---- Couvertures / paiement ----

    /// Ouvre une couverture et retourne son id.
    pub fn open_hedge(
        env: Env,
        owner: Address,
        supplier: Address,
        currency: Symbol,
        side: Side,
        notional: i128,
        duration_secs: u64,
        cap_bps: u32,
    ) -> u64 {
        admin::require_not_paused(&env);
        hedge::open(&env, &owner, &supplier, &currency, side, notional, duration_secs, cap_bps)
    }

    /// Permissionless après l'échéance. Retourne le montant USDC versé au propriétaire.
    pub fn settle(env: Env, hedge_id: u64) -> i128 {
        hedge::settle(&env, hedge_id)
    }

    /// Propriétaire seul, avant l'échéance : clôture au prix du marché (forward du jour pour la durée restante,
    /// spread inclus). Le perdant paie. Retourne le montant USDC versé au propriétaire.
    pub fn close_early(env: Env, hedge_id: u64) -> i128 {
        hedge::close_early(&env, hedge_id)
    }

    pub fn get_hedge(env: Env, hedge_id: u64) -> Option<Hedge> {
        hedge::get(&env, hedge_id)
    }

    /// Mode démo : échéance réelle = durée annoncée / `time_factor` (le prix reste calculé sur la
    /// durée annoncée). 1 par défaut = temps réel.
    pub fn time_factor(env: Env) -> u32 {
        hedge::time_factor(&env)
    }

    /// Admin, testnet / démo uniquement : ex. 25 920 → une couverture à 90 jours échoit en 300 s.
    pub fn set_time_factor(env: Env, factor: u32) {
        hedge::set_time_factor(&env, factor)
    }

    // ---- Admin ----

    /// Admin : propose un nouvel admin, qui devra accepter (deux temps).
    pub fn propose_admin(env: Env, new_admin: Address) {
        admin::propose_admin(&env, &new_admin)
    }

    /// Signé par l'admin proposé : il reçoit le rôle, l'ancien perd ses droits.
    pub fn accept_admin(env: Env) {
        admin::accept_admin(&env)
    }

    /// Admin : annule la proposition en cours.
    pub fn cancel_admin_proposal(env: Env) {
        admin::cancel_admin_proposal(&env)
    }

    pub fn get_admin(env: Env) -> Address {
        admin(&env)
    }

    pub fn pending_admin(env: Env) -> Option<Address> {
        admin::pending_admin(&env)
    }

    /// Admin : pose la clé de garde (distincte de l'admin), qui peut mettre en pause.
    pub fn set_guardian(env: Env, guardian: Address) {
        admin::set_guardian(&env, &guardian)
    }

    pub fn guardian(env: Env) -> Option<Address> {
        admin::guardian(&env)
    }

    /// Garde ou admin (signature de `caller`) : bloque dépôts et ouvertures de couverture.
    /// Retraits, règlement, clôture anticipée et lectures restent possibles.
    pub fn pause(env: Env, caller: Address) {
        admin::pause(&env, &caller)
    }

    /// Admin seul.
    pub fn unpause(env: Env) {
        admin::unpause(&env)
    }

    pub fn paused(env: Env) -> bool {
        admin::paused(&env)
    }

    /// Admin : remplace le code WASM (hash déjà téléversé), adresse et données inchangées.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        admin::upgrade(&env, &new_wasm_hash)
    }
}

mod test;
