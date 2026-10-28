// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Env,
};

#[test]
fn set_and_read_price() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(MockOracle, (&admin,));
    let client = MockOracleClient::new(&env, &id);

    let mxn = Asset::Other(Symbol::new(&env, "MXN"));
    assert_eq!(client.lastprice(&mxn), None);

    client.set_price(&mxn, &5_500_000_000_000);
    assert_eq!(client.lastprice(&mxn).unwrap().price, 5_500_000_000_000);
    assert_eq!(client.assets().len(), 1);

    // crash du peso : on écrase le prix
    client.set_price(&mxn, &6_160_000_000_000);
    assert_eq!(client.lastprice(&mxn).unwrap().price, 6_160_000_000_000);
    assert_eq!(client.assets().len(), 1);
    assert_eq!(client.decimals(), 14);
}

#[test]
fn price_at_timestamp_is_the_one_in_force() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = MockOracleClient::new(&env, &env.register(MockOracle, (&admin,)));
    let mxn = Asset::Other(Symbol::new(&env, "MXN"));

    env.ledger().set_timestamp(1_000);
    assert_eq!(client.price(&mxn, &1_000), None);
    client.set_price(&mxn, &5_500_000_000_000);
    env.ledger().set_timestamp(1_200);
    client.set_price(&mxn, &6_160_000_000_000); // crash
    env.ledger().set_timestamp(1_500);

    assert_eq!(client.price(&mxn, &999), None); // avant le premier prix
    assert_eq!(client.price(&mxn, &1_000).unwrap().price, 5_500_000_000_000);
    assert_eq!(client.price(&mxn, &1_199).unwrap().price, 5_500_000_000_000);
    let at_crash = client.price(&mxn, &1_200).unwrap();
    assert_eq!((at_crash.price, at_crash.timestamp), (6_160_000_000_000, 1_200));
    assert_eq!(client.price(&mxn, &1_500).unwrap().price, 6_160_000_000_000);
    assert_eq!(client.price(&mxn, &1_501), None); // futur
    // lastprice inchangé
    assert_eq!(client.lastprice(&mxn).unwrap().price, 6_160_000_000_000);
}

#[test]
fn history_is_capped() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = MockOracleClient::new(&env, &env.register(MockOracle, (&admin,)));
    let mxn = Asset::Other(Symbol::new(&env, "MXN"));

    for i in 0..(MAX_HISTORY as u64 + 6) {
        env.ledger().set_timestamp(1_000 + i * 60);
        client.set_price(&mxn, &(5_500_000_000_000 + i as i128));
    }
    // Les 6 plus anciens sont sortis de l'historique.
    assert_eq!(client.price(&mxn, &(1_000 + 5 * 60)), None);
    assert_eq!(client.price(&mxn, &(1_000 + 6 * 60)).unwrap().price, 5_500_000_000_006);
    let now = env.ledger().timestamp();
    assert_eq!(client.price(&mxn, &now).unwrap().price, 5_500_000_000_000 + MAX_HISTORY as i128 + 5);
}
