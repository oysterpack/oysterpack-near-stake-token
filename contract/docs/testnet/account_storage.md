near view $CONTRACT storage_minimum_balance

near view $CONTRACT storage_balance_of --args '{"account_id":"oysterpack.testnet"}'

near call $CONTRACT storage_deposit --accountId oysterpack.testnet --amount 0.0681

near call $CONTRACT storage_withdraw --accountId oysterpack.testnet --args '{"amount":"1000000"}' --amount 0.000000000000000000000001

# withdraw all available balance
near call $CONTRACT storage_withdraw --accountId oysterpack.testnet --amount 0.000000000000000000000001