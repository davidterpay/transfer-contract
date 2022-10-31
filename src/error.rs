use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Insufficient Balance Error: your balance - {balance:?} - is less than the requested amount - {requested:?}")]
    InsufficientBalanceError { balance: Uint128, requested: Uint128 },

    #[error("Invalid Fee Percentage: the enter fee parameter must be less than 100 - {fees:?}.")]
    InvalidFeePercentageError { fees: u8 },
}
