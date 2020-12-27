### View Calls
```shell
near view stake.oysterpack.testnet  get_balance --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

near view stake.oysterpack.testnet get_total_supply
```

### Stateful Calls
```shell
near call stake.oysterpack.testnet transfer --accountId alfio-zappala-oysterpack.testnet --args '{"receiver_id":"oysterpack.testnet", "amount":"1000000000000"}'
```