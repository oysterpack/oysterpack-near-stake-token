### View Calls
```shell
near view stake.oysterpack.testnet contract_state

near view stake.oysterpack.testnet config
```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet reset_config_default --accountId oysterpack.testnet

near call stake.oysterpack.testnet update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"callbacks":{"on_run_stake_batch":125000000000000}}}}'

near call stake.oysterpack.testnet force_update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"staking_pool":{"get_account":4500000000000}}}}'

near call stake.oysterpack.testnet force_update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"callbacks":{"on_deposit+and_stake":4500000000000}}}}'
```

16 500 000 000 000 000 000 000 000

4500000000000