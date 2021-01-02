### View Calls
```shell
near view stake.oysterpack.testnet  balance --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

near view stake.oysterpack.testnet total_supply

near view stake.oysterpack.testnet metadata
```

### Stateful Calls
```shell
near call stake.oysterpack.testnet transfer --accountId alfio-zappala-oysterpack.testnet --args '{"recipient":"oysterpack.testnet", "amount":"1000000000000"}'

near call stake.oysterpack.testnet transfer --accountId oysterpack.testnet --args '{"recipient":"alfio-zappala-oysterpack.testnet", "amount":"1000000000000", "headers":{"msg":"merry christmas"}}'
```

1000000000000000000000000
 1 000 000 000 000
40 000 000 000 000