#!/bin/bash
set -e

cargo build --target wasm32-unknown-unknown --release
wasm-opt ../target/wasm32-unknown-unknown/release/staking_pool_mock.wasm -Oz -o ../res/staking_pool_mock.wasm