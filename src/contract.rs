use std::str::from_utf8;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:transfer-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // State.fees is the percentage of transaction funds that will be sent to the over in send transactions.
    // As such, it must be less than 100 because in send the logic does msg.fees / 100 when distributing funds.
    if msg.fees > 100 {
        return Err(ContractError::InvalidFeePercentageError { fees: msg.fees });
    }

    let state = State {
        owner: info.sender.clone(),
        fees: msg.fees,
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    STATE.save(deps.storage, &state)?;

    let res = Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("fees", from_utf8(&[msg.fees]).unwrap());

    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Send { account1, account2 } => execute::send(deps, info, account1, account2),
        ExecuteMsg::Withdraw { amount, denom } => execute::withdraw(deps, info, amount, denom),
        ExecuteMsg::WithdrawAll { denom } => execute::withdraw_all(deps, info, denom),
    }   
}

pub mod execute {
    use std::ops::Shr;

    use cosmwasm_std::{coins, Addr, BankMsg, Uint128};

    use crate::state::BALANCES;

    use super::*;

    pub fn send(
        deps: DepsMut,
        info: MessageInfo,
        account1: String,
        account2: String,
    ) -> Result<Response, ContractError> {
        // Validating the two addresses that will have an allowance
        let address1: Addr = deps.api.addr_validate(&account1)?;
        let address2: Addr = deps.api.addr_validate(&account2)?;

        let state: State = STATE.load(deps.storage)?;
        let fees: Uint128 = Uint128::from(state.fees);

        // Iterating through all of the coins for distribution
        for coin in info.funds.iter() {
            // Updating the owners balance
            let owner_fees: Uint128 = coin.amount.multiply_ratio(fees, Uint128::new(100));
            BALANCES.update(
                deps.storage,
                (&state.owner, coin.denom.clone()),
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() + owner_fees)
                },
            )?;

            // Updating the remaining balances
            let left_over: Uint128 = coin.amount - owner_fees;
            let split_amount: Uint128 = left_over.shr(1);
            let left_over: Uint128 = left_over - split_amount;
            BALANCES.update(
                deps.storage,
                (&address1, coin.denom.clone()),
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() + split_amount)
                },
            )?;
            BALANCES.update(
                deps.storage,
                (&address2, coin.denom.clone()),
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() + left_over)
                },
            )?;
        }

        let res = Response::new()
            .add_attribute("method", "send")
            .add_attribute("sender", &info.sender)
            .add_attribute("address_1", &address1)
            .add_attribute("address_2", &address2);

        Ok(res)
    }

    pub fn withdraw(
        deps: DepsMut,
        info: MessageInfo,
        amount: Uint128,
        denom: String,
    ) -> Result<Response, ContractError> {
        let balance = BALANCES
            .may_load(deps.storage, (&info.sender, denom.clone()))?
            .unwrap_or_default();

        if amount > balance {
            return Err(ContractError::InsufficientBalanceError {
                balance: balance,
                requested: amount,
            });
        }

        BALANCES.update(
            deps.storage,
            (&info.sender, denom.clone()),
            |balance: Option<Uint128>| -> StdResult<_> {
                Ok(balance.unwrap_or_default().checked_sub(amount)?)
            },
        )?;

        let res = Response::new()
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(amount.u128(), denom),
            })
            .add_attribute("withdraw", &info.sender)
            .add_attribute("amount", amount);

        Ok(res)
    }

    pub fn withdraw_all(
        deps: DepsMut,
        info: MessageInfo,
        denom: String,
    ) -> Result<Response, ContractError> {
        let balance = BALANCES
            .may_load(deps.storage, (&info.sender, denom.clone()))?
            .unwrap_or_default();

        withdraw(deps, info, balance, denom)
    }

}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner {} => to_binary(&query::owner(deps)?),
        QueryMsg::GetFees {} => to_binary(&query::fees(deps)?),
        QueryMsg::GetBalance { account, denom } => {
            to_binary(&query::balance(deps, account, denom)?)
        }
    }
}

pub mod query {
    use cosmwasm_std::Addr;

    use crate::{
        msg::{GetBalanceResponse, GetFeesResponse, GetOwnerResponse},
        state::BALANCES,
    };

    use super::*;

    pub fn owner(deps: Deps) -> StdResult<GetOwnerResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetOwnerResponse { owner: state.owner })
    }

    pub fn fees(deps: Deps) -> StdResult<GetFeesResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetFeesResponse { fees: state.fees })
    }

    pub fn balance(deps: Deps, account: String, denom: String) -> StdResult<GetBalanceResponse> {
        let address: Addr = deps.api.addr_validate(&account)?;

        let balance = BALANCES
            .may_load(deps.storage, (&address, denom))?
            .unwrap_or_default();

        Ok(GetBalanceResponse { balance: balance })
    }
}

#[cfg(test)]
mod tests {
    use crate::msg::{GetBalanceResponse, GetFeesResponse, GetOwnerResponse};

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, BankMsg, CosmosMsg, Uint128, Addr};

    #[test]
    fn initialization_basic() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };
        let info = mock_info("creator", &coins(0, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: GetOwnerResponse = from_binary(&res).unwrap();
        assert_eq!("creator", value.owner);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetFees {}).unwrap();
        let value: GetFeesResponse = from_binary(&res).unwrap();
        assert_eq!(10, value.fees);
    }

    #[test]
    fn initialization_fail() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 101 };
        let info = mock_info("creator", &coins(0, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            ContractError::InvalidFeePercentageError { fees: _ } => (),
            e => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn send_basic() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg);

        // ensure initial balance of account1 is 0 before a sent
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(0), value.balance);

        // disburse an initial send to two accounts (both accounts should have 4 after fees in their allowances i.e. balances)
        let info = mock_info("sender", &coins(10, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(4), value.balance);

        // query to check updated balance of account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(5), value.balance);

        // retrieve the owner to check if fees were collected
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: GetOwnerResponse = from_binary(&res).unwrap();  
        let owner: Addr = value.owner;

        // retrieve the balance of the owner to see if fees were collected
        let msg = QueryMsg::GetBalance {
            account: owner.to_string(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(1), value.balance);

    }

    #[test]
    fn send_multiple() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg);

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(51, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(23), value.balance);

        // query to check updated balance of account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(23), value.balance);

        // disburse an another send to two accounts (both accounts should have 5 in their allowances i.e. balances)
        let info = mock_info("sender", &coins(65, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account3".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(52), value.balance);

        // query to check updated balance of account 3
        let msg = QueryMsg::GetBalance {
            account: "account3".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(30), value.balance);
    }

    #[test]
    fn send_multiple_currencies() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg);

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(100, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // query to check updated balance of account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // disburse an another send to two accounts but with a different currency this time
        let info = mock_info("sender", &coins(50, "wei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "wei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(22), value.balance);

        // query to check updated balance of account 3
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "wei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(23), value.balance);
    }

    #[test]
    fn withdraw_basic() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(100, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // account 1 withdraws money from the contract
        let msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(25),
            denom: "usei".to_owned(),
        };
        let info = mock_info("account1", &coins(0, "usei"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "account1".to_owned(),
                amount: coins(25, "usei")
            })
        );

        // query to check updated balance for account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(20), value.balance);

        // query to check updated balance for account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);
    }

    #[test]
    fn withdraw_all() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(100, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // account 1 withdraws money from the contract
        let msg = ExecuteMsg::WithdrawAll {
            denom: "usei".to_owned(),
        };
        let info = mock_info("account1", &coins(0, "usei"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "account1".to_owned(),
                amount: coins(45, "usei")
            })
        );

        // query to check updated balance for account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(0), value.balance);

        // query to check updated balance for account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);
    }

    #[test]
    fn withdraw_fail() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(100, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // query to check updated balance of account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // account 1 over-withdraws money from the contract
        let msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(46),
            denom: "usei".to_owned(),
        };
        let info = mock_info("account1", &coins(0, "usei"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            ContractError::InsufficientBalanceError {
                balance: _,
                requested: _,
            } => (),
            e => panic!("unexpected error: {:?}", e),
        }

        // query to check updated balance for account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);

        // query to check updated balance for account 2
        let msg = QueryMsg::GetBalance {
            account: "account2".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(45), value.balance);
    }

    #[test]
    fn withdraw_multiple() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fees: 10 };

        // instantiate the contract
        let info = mock_info("creator", &coins(0, "usei"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // disburse an initial send to two accounts
        let info = mock_info("sender", &coins(100, "usei"));
        let msg: ExecuteMsg = ExecuteMsg::Send {
            account1: "account1".to_owned(),
            account2: "account2".to_owned(),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);

        // account 1 withdraws money from the contract
        let msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(25),
            denom: "usei".to_owned(),
        };
        let info = mock_info("account1", &coins(0, "usei"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "account1".to_owned(),
                amount: coins(25, "usei")
            })
        );

        // query to check updated balance for account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(20), value.balance);

        // account 1 withdraws money from the contract a second time
        let msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(19),
            denom: "usei".to_owned(),
        };
        let info = mock_info("account1", &coins(0, "usei"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "account1".to_owned(),
                amount: coins(19, "usei")
            })
        );

        // query to check updated balance for account 1
        let msg = QueryMsg::GetBalance {
            account: "account1".to_owned(),
            denom: "usei".to_owned(),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(1), value.balance);
    }
}
