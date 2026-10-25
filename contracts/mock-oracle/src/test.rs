#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

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
