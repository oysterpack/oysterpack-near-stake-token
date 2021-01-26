### View Calls
```shell
near view $CONTRACT contract_state

near view $CONTRACT config
```

### Stateful Func Calls
```shell
near call $CONTRACT clear_stake_batch_lock --accountId oysterpack.testnet

near call $CONTRACT clear_redeem_stake_batch_lock --accountId oysterpack.testnet

near call $CONTRACT reset_config_default --accountId oysterpack.testnet

near call $CONTRACT update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"callbacks":{"on_run_stake_batch":125000000000000}}}}'

near call $CONTRACT force_update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"staking_pool":{"get_account":4500000000000}}}}'

near call $CONTRACT force_update_config --accountId oysterpack.testnet --args \
'{"config":{"gas_config":{"callbacks":{"on_run_stake_batch":125000000000000,"on_deposit_and_stake":5000000000000,"on_unstake":5000000000000,"on_run_redeem_stake_batch":85000000000000,"on_redeeming_stake_pending_withdrawal":85000000000000,"unlock":5000000000000,"on_redeeming_stake_post_withdrawal":5000000000000},"staking_pool":{"deposit_and_stake":50000000000000,"unstake":50000000000000,"withdraw":50000000000000,"get_account":5000000000000},"vault_ft":{"min_gas_for_receiver":10000000000000,"transfer_with_vault":25000000000000,"resolve_vault":5000000000000},"transfer_call_ft":{"min_gas_for_receiver":5000000000000,"transfer_call":25000000000000,"finalize_ft_transfer":5000000000000}}}}'

near call $CONTRACT force_update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"callbacks":{"on_deposit_and_stake":"15000000000000"}}}}'
near call $CONTRACT force_update_config --accountId oysterpack.testnet --args '{"config":{"gas_config":{"staking_pool":{"stake":5000000000000}}}}'
```

near tx-status --accountId oysterpack.testnet AFZieZSG9aymGnQNpw3mdUiFeTDE2cEkc4yrJWFwRZWi > temp/txn.txt

1000000000000   1 TGas
2427936651538
6167250439669
4506577528637
6219296404217
4506577528637