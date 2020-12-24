### View Calls
```bash
near view stake.oysterpack.testnet staking_pool_id

near view stake.oysterpack.testnet stake_token_value

near view stake.oysterpack.testnet pending_withdrawal

near view stake.oysterpack.testnet redeem_stake_batch_receipt --args '{"batch_id":"3"}'
```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet deposit --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet deposit --accountId 1.alfio-zappala-oysterpack.testnet --amount 2

near call stake.oysterpack.testnet run_stake_batch --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet redeem --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call stake.oysterpack.testnet redeem --accountId 1.alfio-zappala-oysterpack.testnet --args '{"amount":"600000000000000000000000"}'

near call stake.oysterpack.testnet redeem_all --accountId 1.alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet run_redeem_stake_batch --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000
```

## Staking Pool
```shell
export STAKING_POOL=staked.pool.f863973.m0
#export STAKING_POOL=stakin.pool.f863973.m0

near view $STAKING_POOL get_account --args '{"account_id":"stake.oysterpack.testnet"}'

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"100"}' --gas 300000000000000

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"10000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL unstake_all --accountId stake.oysterpack.testnet --gas 300000000000000

near call $STAKING_POOL stake --accountId stake.oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL withdraw_all --accountId stake.oysterpack.testnet --gas 300000000000000
```

# 1000000000000000000000000     = 1 NEAR
#  500000000000000000000000     = 0.5 NEAR

300000000000000
  4420049914538
 27909492989128
  2427974662416
 18588240766627
 18356815353739
  3591923837853

475611913405595162900000000
475611868769082662900000000