export CONTRACT_TEST=dev-1609182856595-2170806

near call stake.oysterpack.testnet unregister_account --accountId $CONTRACT_TEST

near view $CONTRACT_TEST ping

near call $CONTRACT_TEST test_account_registration_workflow --accountId oysterpack.testnet --args '{"stake_token_contract":"stake.oysterpack.testnet"}'

1000000000000   1 TGAS
17374630679850
48958982322329