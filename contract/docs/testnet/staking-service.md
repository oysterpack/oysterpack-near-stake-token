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

near call stake.oysterpack.testnet stake --accountId alfio-zappala-oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet deposit_and_stake --accountId alfio-zappala-oysterpack.testnet --amount 1 --gas 150000000000000
near call stake.oysterpack.testnet deposit_and_stake --accountId oysterpack.testnet --amount 1 --gas 150000000000000

near call stake.oysterpack.testnet redeem --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}'
near call stake.oysterpack.testnet redeem --accountId 1.alfio-zappala-oysterpack.testnet --args '{"amount":"600000000000000000000000"}'

near call stake.oysterpack.testnet redeem_all --accountId 1.alfio-zappala-oysterpack.testnet
near call stake.oysterpack.testnet redeem_all --accountId oysterpack.testnet

near call stake.oysterpack.testnet unstake --accountId alfio-zappala-oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet redeem_and_unstake --accountId alfio-zappala-oysterpack.testnet --args '{"amount":"500000000000000000000000"}' --gas 150000000000000
near call stake.oysterpack.testnet redeem_all_and_unstake --accountId alfio-zappala-oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet redeem_all_and_unstake --accountId oysterpack.testnet --gas 150000000000000

near call stake.oysterpack.testnet cancel_uncommitted_redeem_stake_batch --accountId alfio-zappala-oysterpack.testnet

```

## Staking Pool
```shell
export STAKING_POOL=staked.pool.f863973.m0
export STAKING_POOL=stakin.pool.f863973.m0
export STAKING_POOL=lunanova.pool.f863973.m0

near view $STAKING_POOL get_account --args '{"account_id":"stake.oysterpack.testnet"}'

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"100"}' --gas 300000000000000

near call $STAKING_POOL unstake --accountId stake.oysterpack.testnet --args '{"amount":"10000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL unstake_all --accountId stake.oysterpack.testnet

near call $STAKING_POOL stake --accountId stake.oysterpack.testnet --args '{"amount":"1000000000000000000000000"}' --gas 300000000000000

near call $STAKING_POOL withdraw_all --accountId stake.oysterpack.testnet --gas 300000000000000
```

# 1000000000000000000000000     = 1 NEAR
#  500000000000000000000000     = 0.5 NEAR

   500000000000000000000000
    68100000000000000000000
   2504182481837150800772062
   2497219431892304397903632

# deposit_and_stake transaction (alfio-zappala-oysterpack.testnet:G7ahkVZGrcJZPJWghawh73qtNVDzPnYf6LZPbAaVxHoy)
  2 427 963 482 746        StakingService::deposit_and_stake         G7ahkVZGrcJZPJWghawh73qtNVDzPnYf6LZPbAaVxHoy
 32 328 258 227 662        StakingService::stake                     FkSZkV1pe25JMK9sLNs9MBJbnAcmLwMo9yuzfHLgEFQY
  4 306 264 748 859        staking-pool::get_account_staked_balance  CwYK77QvyLf8r8oZP5ZuxzwGyLRVcw6t2M3hDv1ZW6na 
 18 244 770 167 486        StakingService:on_run_stake_batch         BKAwaxs7cyZQU8XDrGdyVugNipNidzQJYpGgEnzYGe3E
 22 721 733 350 873        staking-pool::deposit_and_stake           7ziED2YhMbDcYVYkapcWpHNgWdeQ6iQtqPq5D4sdcy7p
  4 220 515 569 698        StakingService::on_deposit_and_stake      2eLh9Ds5MWSCF6kVVcGQHELvzDSX2KxFNzMm5pgx3BhB
  3 790 483 214 956        StakingService::release_run_stake_batch   DXNKjJQW8GRZmzySiiKv7ZYypzGTjMZNC5mgK4B685wx

100 000 000 000 000 
 29 985 580 262 929
 18 244 770 167 486
 32 327 841 739 594
    210 277 125 000
  3 591 923 837 853
110 000 000 000 000

  2 427 974 662 416
  4 307 213 416 125 'C5ZcCt4ytQ6u5BNVC4G7HkFSiFfdd5J89mkK2axuyQwC',
 18 335 095 321 900 'B9cSUpFonwGyPKqQtZSegmPBXtsz7JrL2mpos6pFJezk',
 18 678 677 812 567
    210 277 125 000
     99 607 375 000
  3 790 483 214 956
  '9AwVWP78wwD1JUewJuxTphbTLKmewHkvKB1xnYCvpWNY',
  'Bh2aqj9tGn8uAwHH3QMrhimiepPRPhJzcGCh9ki4mRkt',