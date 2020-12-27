### View Calls
```shell
near view stake.oysterpack.testnet contract_state
```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet reset_config_default --accountId oysterpack.testnet

near call stake.oysterpack.testnet update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"callbacks":{"finalize_ownership_transfer":5000000000000}}}}'
```