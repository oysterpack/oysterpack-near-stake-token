### View Calls
```shell
near view $CONTRACT  ft_balance_of --args '{"account_id":"oysterpack.testnet"}'

near view $CONTRACT ft_total_supply

```

### Stateful Calls
```shell
near call $CONTRACT ft_transfer --accountId alfio-zappala-oysterpack.testnet --args '{"receiver_id":"oysterpack.testnet", "amount":"1000000000000000000000000"}' --amount 0.000000000000000000000001

near call $CONTRACT ft_transfer --accountId oysterpack.testnet --args '{"receiver_id":"alfio-zappala-oysterpack.testnet", "amount":"1000000000000000000000000", "memo":"merry christmas"}' --amount 0.000000000000000000000001

near call $CONTRACT ft_transfer_call --accountId oysterpack.testnet --args '{"receiver_id":"dev-1611907846758-1343432", "amount":"100000000000000000000000", "memo":"merry christmas", "msg":"{\"Accept\":{\"refund_percent\":0,\"transfer_relay_percent\":0}}"}' --amount 0.000000000000000000000001
```

1000000000000000000000000