### View Calls
```bash
near view stake.oysterpack.testnet staking_pool_id

near view stake.oysterpack.testnet pending_withdrawal

near view stake.oysterpack.testnet stake_batch_receipt --args '{"batch_id":"15"}'

near view stake.oysterpack.testnet redeem_stake_batch_receipt --args '{"batch_id":"3"}'
```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet deposit --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet deposit --accountId oysterpack.testnet --amount 1
near call stake.oysterpack.testnet deposit --accountId 1.alfio-zappala-oysterpack.testnet --amount 2

near call stake.oysterpack.testnet withdraw_funds_from_stake_batch --accountId oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call stake.oysterpack.testnet withdraw_all_funds_from_stake_batch --accountId oysterpack.testnet

near call stake.oysterpack.testnet stake --accountId alfio-zappala-oysterpack.testnet --gas 250000000000000
near call stake.oysterpack.testnet stake --accountId oysterpack.testnet --gas 250000000000000

near call stake.oysterpack.testnet deposit_and_stake --accountId alfio-zappala-oysterpack.testnet --amount 1 --gas 200000000000000
near call stake.oysterpack.testnet deposit_and_stake --accountId oysterpack.testnet --amount 1 --gas 150000000000000

near call stake.oysterpack.testnet redeem --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call stake.oysterpack.testnet redeem --accountId oysterpack.testnet --args '{"amount":"600000000000000000000000"}'

near call stake.oysterpack.testnet redeem_all --accountId alfio-zappala-oysterpack.testnet
near call stake.oysterpack.testnet redeem_all --accountId oysterpack.testnet

near call stake.oysterpack.testnet cancel_uncommitted_redeem_stake_batch --accountId oysterpack.testnet

near call stake.oysterpack.testnet unstake --accountId oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet redeem_and_unstake --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 150000000000000
near call stake.oysterpack.testnet redeem_and_unstake --accountId oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 150000000000000

near call stake.oysterpack.testnet redeem_all_and_unstake --accountId oysterpack.testnet --gas 150000000000000
near call stake.oysterpack.testnet redeem_all_and_unstake --accountId alfio-zappala-oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet cancel_uncommitted_redeem_stake_batch --accountId alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet claim_receipts --accountId oysterpack.testnet 
near call stake.oysterpack.testnet claim_receipts --accountId alfio-zappala-oysterpack.testnet 

```

## Staking Pool
```shell
export STAKING_POOL=staked.pool.f863973.m0
export STAKING_POOL=stakin.pool.f863973.m0
export STAKING_POOL=lunanova.pool.f863973.m0
export STAKING_POOL=dokia.pool.f863973.m0

near view $STAKING_POOL get_account --args '{"account_id":"stake.oysterpack.testnet"}'

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"100"}' --gas 300000000000000

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"10000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL unstake_all --accountId stake.oysterpack.testnet

near call $STAKING_POOL stake --accountId stake.oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL withdraw_all --accountId stake.oysterpack.testnet --gas 300000000000000
```

1000000000000000000000000     = 1 NEAR
1999999999999999999999999

1000000000000                 = 1 TGas
22372533024614





