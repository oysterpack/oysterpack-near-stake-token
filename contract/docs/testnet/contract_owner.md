### View Calls
```shell
near view stake.oysterpack.testnet owner_id

near view stake.oysterpack.testnet owner_balance
```

### Stateful fun calls
```shell
near call stake.oysterpack.testnet transfer_ownership --accountId oysterpack.testnet --args '{"new_owner":"unknown.oysterpack.testnet"}'
```