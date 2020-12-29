### View Calls
```shell
near view stake.oysterpack.testnet  balance --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

near view stake.oysterpack.testnet total_supply

near view stake.oysterpack.testnet metadata
```

### Stateful Calls
```shell
near call stake.oysterpack.testnet transfer --accountId alfio-zappala-oysterpack.testnet --args '{"receiver_id":"oysterpack.testnet", "amount":"1000000000000"}'
```

1000000000000000000000000
1 000 000 000 000