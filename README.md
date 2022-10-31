# Sei Transfer Contract
This project was scaffolded by the CosmWasm starter pack. All of the code can be found in `/src`.

## State Management
There are two primary things stored in this contract:
1. The state which tracks the owner of the contract as well as the amount of fees they charge to use the contract.
2. A map which tracks the user -> the balances of different denominations of coins.

## Execution Messages
### Send {account1: String, account2: String}
Sends funds and distributes them evenly between two account while adding up fees for the owner.

### Withdraw {amount : Uint128, denom : String}
Allows users to withdraw funds given an amount and a denom.

### Send {account1: String, account2: String}
Allows users to withdraw the maximum balance for a given denom.

## Query Messages
### GetOwner {}
Returns a human-readable representation of the owner of the smart contract.

### GetFees {}
Returns a human-readable representation of the fees accumulating for an owner.

### GetBalance {account : String, denom: String}
Returns a human-readable representation of the balance of the user 
for a given denom.

## Fee Management
Fees are calcuated by a percentage basis such if fees == 1 on initialization, the owner will take 1% of all sends. There is error handling to ensure that fees is never greater than 100 as that would incorrectly distribute fees. `initialization_basic` and `initialization_fail` test the creation of a new contract.
