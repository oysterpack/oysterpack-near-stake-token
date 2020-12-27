### View Func Calls
```shell
near view stake.oysterpack.testnet account_storage_fee

near view stake.oysterpack.testnet total_registered_accounts

near view stake.oysterpack.testnet lookup_account --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'
near view stake.oysterpack.testnet lookup_account --args '{"account_id":"oysterpack.testnet"}'

near view stake.oysterpack.testnet account_registered --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet register_account --accountId oysterpack.testnet --amount 1
near call stake.oysterpack.testnet register_account --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet register_account --accountId 1.alfio-zappala-oysterpack.testnet --amount 1

near call stake.oysterpack.testnet unregister_account --accountId alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet withdraw --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"200000000000000000000000"}'

near call stake.oysterpack.testnet withdraw_all --accountId alfio-zappala-oysterpack.testnet
```