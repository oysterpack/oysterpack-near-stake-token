#!/bin/bash
set -e

cargo build --target wasm32-unknown-unknown --release
wasm-opt target/wasm32-unknown-unknown/release/oysterpack_near_stake_token_test.wasm -Oz -o res/oysterpack_near_stake_token_test.wasm