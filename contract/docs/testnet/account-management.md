### View Func Calls
```shell
near view $CONTRACT account_storage_fee

near view $CONTRACT total_registered_accounts

near view $CONTRACT lookup_account --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'
near view $CONTRACT lookup_account --args '{"account_id":"oysterpack.testnet"}'
near view $CONTRACT lookup_account --args '{"account_id":"dev-1611907846758-1343432"}'

near view $CONTRACT account_registered --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

```

### Stateful Func Calls
```shell
near call $CONTRACT register_account --accountId oysterpack.testnet --amount 1
near call $CONTRACT register_account --accountId alfio-zappala-oysterpack.testnet --amount 1
near call $CONTRACT register_account --accountId 1.alfio-zappala-oysterpack.testnet --amount 0.0681

near call $CONTRACT unregister_account --accountId alfio-zappala-oysterpack.testnet

near call $CONTRACT withdraw --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"200000000000000000000000"}'

near call $CONTRACT withdraw_all --accountId alfio-zappala-oysterpack.testnet
near call $CONTRACT withdraw_all --accountId oysterpack.testnet
```