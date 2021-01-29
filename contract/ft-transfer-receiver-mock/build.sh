#!/bin/bash
set -e

cargo build --target wasm32-unknown-unknown --release
wasm-opt ../target/wasm32-unknown-unknown/release/ft_transfer_receiver_mock.wasm -Oz -o ../res/ft_transfer_receiver_mock.wasm