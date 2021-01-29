```shell
near dev-deploy ../res/ft_transfer_receiver_mock.wasm 

export CONTRACT=dev-1611907846758-1343432

near call $CONTRACT register_account --accountId oysterpack.testnet --args '{"contract_id":"stake-1.oysterpack.testnet"}'

near call $CONTRACT ft_on_transfer --accountId oysterpack.testnet --args '{"sender_id":"oysterpack.testnet","amount":"100","msg":"{\"Accept\":{\"refund_percent\":0}}"}'
```