## Staking Pool on TestNet
- stakin.pool.f863973.m0
- staked.pool.f863973.m0

## deploying contract to testnet
```shell
export NEAR_ENV=testnet

# recreating stake.oysterpack.testnet account
near delete stake.oysterpack.testnet oysterpack.testnet
near create-account stake.oysterpack.testnet --masterAccount oysterpack.testnet

near deploy --accountId stake.oysterpack.testnet \
  --wasmFile res/oysterpack_near_stake_token.wasm \
  --initFunction new \
  --initArgs '{"settings":{"staking_pool_id":"stakin.pool.f863973.m0", "operator_id":"oysterpack.testnet"}}'
  
# redeploy
near deploy --accountId stake.oysterpack.testnet --wasmFile res/oysterpack_near_stake_token.wasm 
```

# NEAR CLI Usage Examples

## Account Management

### View Func Calls
```shell
near view stake.oysterpack.testnet account_storage_fee

near view stake.oysterpack.testnet staking_pool_id

near view stake.oysterpack.testnet stake_token_value

near view stake.oysterpack.testnet total_registered_accounts

near view stake.oysterpack.testnet lookup_account --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

near view stake.oysterpack.testnet account_registered --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

```

### Stateful Func Calls
```shell
near call stake.oysterpack.testnet register_account --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet register_account --accountId 1.alfio-zappala-oysterpack.testnet --amount 1

near call stake.oysterpack.testnet unregister_account --accountId alfio-zappala-oysterpack.testnet
```

## Staking Service

## STAKE Token - NEP-122 

### View Calls
```shell
near view stake.oysterpack.testnet  get_balance --args '{"account_id":"alfio-zappala-oysterpack.testnet"}'

near view stake.oysterpack.testnet get_total_supply
```
### ### Stateful Func Calls
```shell
near call stake.oysterpack.testnet deposit --accountId alfio-zappala-oysterpack.testnet --amount 1
near call stake.oysterpack.testnet deposit --accountId 1.alfio-zappala-oysterpack.testnet --amount 2

near call stake.oysterpack.testnet run_stake_batch --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet refresh_stake_token_value --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000

near call stake.oysterpack.testnet claim_all_batch_receipt_funds --accountId alfio-zappala-oysterpack.testnet --gas 300000000000000
```

## Operator

### View Calls
```shell
near view stake.oysterpack.testnet contract_state
```

1 000 000 000 000 000 000 000 000

1 332 187 358 794 090 528 436 378

8020698702263739326176175
1332187358794090528436378