use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub fees: u8,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Sends funds and distributes them evenly between two account while adding up fees for the owner
    Send {account1: String, account2: String},
    /// Allows users to withdraw funds given an amount and a denom
    Withdraw {amount : Uint128, denom : String},
    /// Allows users to withdraw the maximum balance for a given denom
    WithdrawAll {denom : String},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns a human-readable representation of the owner
    #[returns(GetOwnerResponse)]
    GetOwner {},

    /// Returns a human-readable representation of the fees accumulating for an owner
    #[returns(GetFeesResponse)]
    GetFees {},

    /// Returns a human-readable representation of the balance of the user 
    /// for a given denom
    #[returns(GetBalanceResponse)]
    GetBalance {account : String, denom: String}
}


#[cw_serde]
pub struct GetOwnerResponse {
    pub owner: Addr,
}

#[cw_serde]
pub struct GetFeesResponse {
    pub fees: u8,
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetBalanceResponse {
    pub balance: Uint128,
}
