## Staking Pool on TestNet
- stakin.pool.f863973.m0
- staked.pool.f863973.m0
- lunanova.pool.f863973.m0
- dokia.pool.f863973.m0

## deploying contract to testnet
```shell
export NEAR_ENV=testnet

# recreating stake.oysterpack.testnet account
near delete stake.oysterpack.testnet oysterpack.testnet
near create-account stake.oysterpack.testnet --masterAccount oysterpack.testnet

near deploy --accountId stake.oysterpack.testnet \
  --wasmFile res/oysterpack_near_stake_token.wasm \
  --initFunction new \
  --initArgs '{"settings":{"staking_pool_id":"staked.pool.f863973.m0", "operator_id":"oysterpack.testnet"}}'
  
# redeploy - with no breaking state schema changes
near deploy --accountId stake.oysterpack.testnet --wasmFile res/oysterpack_near_stake_token.wasm 
```