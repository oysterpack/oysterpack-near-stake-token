export DATAHUB_APIKEY=<DATAHUB_APIKEY>
export NEAR_ACCOUNT=oysterpack.testnet

export CONTRACT=stake-demo.oysterpack.testnet
export NEAR_NODE_URL=https://near-testnet--rpc.datahub.figment.io/apikey/$DATAHUB_APIKEY
export NEAR_ENV=testnet

# register account
near call $CONTRACT register_account --node_url $NEAR_NODE_URL --accountId $NEAR_ACCOUNT --amount 1

# deposit and stake some NEAR to get some STAKE tokens
near call $CONTRACT deposit_and_stake --node_url $NEAR_NODE_URL --accountId $NEAR_ACCOUNT --amount 1 --gas 200000000000000

# check balance
near view $CONTRACT ft_balance_of --node_url $NEAR_NODE_URL --args "{\"account_id\":\"$NEAR_ACCOUNT\"}" 

# check total supply
near view $CONTRACT ft_total_supply --node_url $NEAR_NODE_URL

# check balance for receiver contract - before transfer call
near view $CONTRACT ft_balance_of --node_url $NEAR_NODE_URL --args '{"account_id":"dev-1611907846758-1343432"}'

# transfer STAKE via a simple transfer
near call $CONTRACT ft_transfer --node_url $NEAR_NODE_URL --accountId $NEAR_ACCOUNT  --args '{"receiver_id":"dev-1611907846758-1343432", "amount":"10000000"}' --amount 0.000000000000000000000001

# check balance for transfer receiver contract - before transfer call
near view $CONTRACT ft_balance_of --node_url $NEAR_NODE_URL--args "{\"account_id\":\"dev-1611907846758-1343432\"}"

# transfer STAKE via a transfer call to another contract
near call $CONTRACT ft_transfer_call --node_url $NEAR_NODE_URL --accountId $NEAR_ACCOUNT  --args '{"receiver_id":"dev-1611907846758-1343432", "amount":"1000000", "memo":"merry christmas", "msg":"{\"Accept\":{\"refund_percent\":50}}"}' --amount 0.000000000000000000000001

# check balance for transfer receiver contract - after transfer call
near view $CONTRACT ft_balance_of --node_url $NEAR_NODE_URL --args "{\"account_id\":\"dev-1611907846758-1343432\"}"


