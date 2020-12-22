### View Calls
```bash
near view stake.oysterpack.testnet staking_pool_id

near view stake.oysterpack.testnet stake_token_value
```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet deposit --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet deposit --accountId 1.alfio-zappala-oysterpack.testnet --amount 2

near call stake.oysterpack.testnet run_stake_batch --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet refresh_stake_token_value --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet claim_all_batch_receipt_funds --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet redeem --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}'

near call stake.oysterpack.testnet redeem --accountId 1.alfio-zappala-oysterpack.testnet --args '{"amount":"600000000000000000000000"}'

near call stake.oysterpack.testnet redeem_all --accountId 1.alfio-zappala-oysterpack.testnet

near call stake.oysterpack.testnet run_redeem_stake_batch --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

```

## Staking Pool
```shell
near view stakin.pool.f863973.m0 get_account --args '{"account_id":"stake.oysterpack.testnet"}'

near call stakin.pool.f863973.m0 unstake --accountId stake.oysterpack.testnet --args '{"amount":"100"}' --gas 300000000000000

near call stakin.pool.f863973.m0 unstake --accountId stake.oysterpack.testnet --args '{"amount":"10000000000000000000000000"}' --gas 300000000000000

near call stakin.pool.f863973.m0 unstake_all --accountId stake.oysterpack.testnet --gas 300000000000000

near call stakin.pool.f863973.m0 stake --accountId stake.oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 300000000000000
```

100 000 000 000 000
300 000 000 000 000
27 746 674 478 365

2 166 093 679 397 045 264 218 189

9 020 698 702 263 739 326 176 171

1000000000000000000000000
13782080610593614417005060

1000000000000000000000000
100000000000000000000000000
500000000000000000000000
500000000000000000000000
1163454750719037791377183
9020698702263739326176171

2832187358794090528436378

17146719750067146888600308

12082080610593614417004917
1995642109513128319813561

