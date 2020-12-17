## TODO
1. create separate crates for contract external interfaces and publish on crates.io
2. contract config management
   - should be stored in state storage
   - should be managed via TOML based config, i.e., contract operator can upload new TOML
     - this enables config to be version controlled
   - contract operator role is required
3. Simulation testing
   - https://github.com/near-examples/simulation-testing
4. rust WASM frontend - https://www.webassemblyman.com/rustwasm/frontendframeworksrustwebassembly.html 