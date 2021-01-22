### View Calls
```shell
near view $CONTRACT  ft_balance_of --args '{"account_id":"oysterpack.testnet"}'

near view $CONTRACT ft_total_supply

```

### Stateful Calls
```shell
near call $CONTRACT transfer --accountId alfio-zappala-oysterpack.testnet --args '{"recipient":"oysterpack.testnet", "amount":"1000000000000"}'

near call $CONTRACT transfer --accountId oysterpack.testnet --args '{"recipient":"alfio-zappala-oysterpack.testnet", "amount":"1000000000000", "headers":{"msg":"merry christmas"}}'
```

1000000000000000000000000
 1 000 000 000 000
40 000 000 000 000