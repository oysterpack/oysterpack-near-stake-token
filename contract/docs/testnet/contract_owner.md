### View Calls
```shell
near view stake.oysterpack.testnet owner_id

near view stake.oysterpack.testnet owner_balance

near view stake.oysterpack.testnet owner_starting_balance
```

### Stateful fun calls
```shell
near call stake.oysterpack.testnet transfer_ownership --accountId oysterpack.testnet --args '{"new_owner":"unknown.oysterpack.testnet"}'

near call stake.oysterpack.testnet withdraw_owner_balance --args '{"amount":"5426381"}' --accountId alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet withdraw_all_owner_balance --accountId alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet stake_owner_balance --args '{"amount":"5426381"}' --accountId alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet stake_all_owner_balance --accountId alfio-zappala-oysterpack.testnet
```