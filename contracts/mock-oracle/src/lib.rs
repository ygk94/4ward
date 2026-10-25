#![no_std]
//! Oracle de démo, même interface de lecture que Reflector (SEP-40) :
//! `base`, `decimals`, `resolution`, `assets`, `lastprice`, `price`.
//! L'historique des prix sert au fixing à l'échéance (`price(asset, ts)`).
//! L'admin peut fixer un prix à la main (bouton « crash du peso »).
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

/// Même forme que l'`Asset` de SEP-40 / Reflector : les devises fiat sont `Other(symbole)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
enum DataKey {
    Admin,
    Assets,
    Price(Asset),
    /// Prix successifs d'un actif, du plus ancien au plus récent, `MAX_HISTORY` au plus.
    History(Asset),
}

/// Reflector FX : 14 décimales, base USD, résolution 300 s.
const DECIMALS: u32 = 14;
const RESOLUTION: u32 = 300;
/// Taille de l'historique par actif : largement assez pour une démo (un prix toutes les quelques minutes).
const MAX_HISTORY: u32 = 64;

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Assets, &Vec::<Asset>::new(&env));
    }

    /// Prix en USD par unité de devise, avec 14 décimales (ex. 1 MXN = 0.055 USD → 5_500_000_000_000).
    pub fn set_price(env: Env, asset: Asset, price: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        if price <= 0 {
            panic!("price must be positive");
        }
        let mut assets: Vec<Asset> = env.storage().instance().get(&DataKey::Assets).unwrap();
        if !assets.contains(&asset) {
            assets.push_back(asset.clone());
            env.storage().instance().set(&DataKey::Assets, &assets);
        }
        let data = PriceData {
            price,
            timestamp: env.ledger().timestamp(),
        };
        let key = DataKey::History(asset.clone());
        let mut history: Vec<PriceData> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));
        if history.len() >= MAX_HISTORY {
            history.pop_front();
        }
        history.push_back(data.clone());
        env.storage().persistent().set(&key, &history);
        env.storage().persistent().set(&DataKey::Price(asset), &data);
    }

    pub fn base(_env: Env) -> Asset {
        Asset::Other(Symbol::new(&_env, "USD"))
    }

    pub fn decimals(_env: Env) -> u32 {
        DECIMALS
    }

    pub fn resolution(_env: Env) -> u32 {
        RESOLUTION
    }

    pub fn assets(env: Env) -> Vec<Asset> {
        env.storage().instance().get(&DataKey::Assets).unwrap()
    }

    pub fn lastprice(env: Env, asset: Asset) -> Option<PriceData> {
        env.storage().persistent().get(&DataKey::Price(asset))
    }

    /// Prix en vigueur à `timestamp` : le dernier posé à cet instant ou avant (SEP-40 `price`).
    /// `None` si `timestamp` est dans le futur ou avant le premier prix, comme Reflector
    /// (qui renvoie le cours de la période de 300 s contenant `timestamp`).
    pub fn price(env: Env, asset: Asset, timestamp: u64) -> Option<PriceData> {
        if timestamp > env.ledger().timestamp() {
            return None;
        }
        let history: Vec<PriceData> = env.storage().persistent().get(&DataKey::History(asset))?;
        history.iter().rev().find(|p| p.timestamp <= timestamp)
    }
}

mod test;
