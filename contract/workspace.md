# Workspace Setup
```shell script
sudo apt update
sudo apt install libssl-dev cmake pkg-config build-essential musl-tools llvm clang
# used to format test coverage reports into HTML
sudo pip3 install pycobertura

cargo install --force cargo-make
cargo install --force cargo-tarpaulin
```

# How to ... 

## run tests with code coverage
`cargo tarpaulin --ignore-tests --output-dir target/tarpaulin --out Html`
- generates target/tarpaulin/tarpaulin-report.html

## generate docs
`cargo doc --no-deps --open`

