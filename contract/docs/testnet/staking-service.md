### View Calls
```bash
near view $CONTRACT staking_pool_id

near view $CONTRACT pending_withdrawal

near view $CONTRACT stake_batch_receipt --args '{"batch_id":"15"}'

near view $CONTRACT redeem_stake_batch_receipt --args '{"batch_id":"3"}'
```

### Stateful Func Calls
```shell
near call $CONTRACT deposit --accountId alfio-zappala-oysterpack.testnet --amount 1
near call $CONTRACT deposit --accountId oysterpack.testnet --amount 1
near call $CONTRACT deposit --accountId 1.alfio-zappala-oysterpack.testnet --amount 2

near call $CONTRACT withdraw_funds_from_stake_batch --accountId oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call $CONTRACT withdraw_all_funds_from_stake_batch --accountId oysterpack.testnet

near call $CONTRACT stake --accountId alfio-zappala-oysterpack.testnet --gas 200000000000000
near call $CONTRACT stake --accountId oysterpack.testnet --gas 200000000000000

near call $CONTRACT deposit_and_stake --accountId alfio-zappala-oysterpack.testnet --amount 1 --gas 200000000000000
near call $CONTRACT deposit_and_stake --accountId oysterpack.testnet --amount 1 --gas 200000000000000

near call $CONTRACT redeem --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call $CONTRACT redeem --accountId oysterpack.testnet --args '{"amount":"600000000000000000000000"}'

near call $CONTRACT redeem_all --accountId alfio-zappala-oysterpack.testnet
near call $CONTRACT redeem_all --accountId oysterpack.testnet

near call $CONTRACT cancel_uncommitted_redeem_stake_batch --accountId oysterpack.testnet

near call $CONTRACT unstake --accountId oysterpack.testnet --gas 150000000000000

near call $CONTRACT redeem_and_unstake --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 150000000000000
near call $CONTRACT redeem_and_unstake --accountId oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 150000000000000

near call $CONTRACT redeem_all_and_unstake --accountId oysterpack.testnet --gas 150000000000000
near call $CONTRACT redeem_all_and_unstake --accountId alfio-zappala-oysterpack.testnet --gas 150000000000000

near call $CONTRACT cancel_uncommitted_redeem_stake_batch --accountId alfio-zappala-oysterpack.testnet

near call $CONTRACT claim_receipts --accountId oysterpack.testnet 
near call $CONTRACT claim_receipts --accountId alfio-zappala-oysterpack.testnet 


near call $CONTRACT on_deposit_and_stake_2 --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000 \
--args '{"staking_pool_account":{"account_id":"", "unstaked_balance":"38", "staked_balance":"8013327778056927180799196", "can_withdraw":true}}' 

```

## Staking Pool
```shell
export STAKING_POOL=staked.pool.f863973.m0
export STAKING_POOL=stakin.pool.f863973.m0
export STAKING_POOL=lunanova.pool.f863973.m0
export STAKING_POOL=dokia.pool.f863973.m0

near view $STAKING_POOL get_account --args '{"account_id":"stake-1.oysterpack.testnet"}'

near call $STAKING_POOL unstake --accountId $CONTRACT --args '{"amount":"100"}' --gas 300000000000000

near call $STAKING_POOL unstake --accountId $CONTRACT --args '{"amount":"10000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL unstake_all --accountId $CONTRACT

near call $STAKING_POOL stake --accountId $CONTRACT --args '{"amount":"1000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL withdraw_all --accountId $CONTRACT --gas 300000000000000
```

1000000000000000000000000     = 1 NEAR

1000000000000                 = 1 TGas






