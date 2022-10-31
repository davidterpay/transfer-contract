use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub fees: u8,
}

/// State tracks the owner of the contract as well as the fees that are removed per send tx. Fees must
/// be a number less than 100. fees is the percentage of each transaction that will go to the owner.
pub const STATE: Item<State> = Item::new("state");

/// Balances tracks the amount of each coin each registered address is permitted to withdraw.
pub const BALANCES: Map<(&Addr, String), Uint128> = Map::new("balances");
