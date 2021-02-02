* Structure of the contract
    * What happens to the fields of a struct decorated with #[near_bindgen];
    * When someone should use std::collections::* vs near_sdk::collections::*;
    * Comparison table of the near_sdk::collections::* data structures with advises and use cases when to use one over another (there are different nuances in terms of their footprint on the state);
    * Nesting of the collections;
    * How to prevent different users of the contract abuse each other through hashing of the collection keys: https://github.com/near/near-sdk-rs/blob/master/examples/fungible-token/src/lib.rs#L240
    * How to leverage registers to minimize the costs;
    * Default vs non-default initialization of the contract. Why would someone choose one over another;
* Contract interface
    * Different ways of having a public method (e.g. through `pub fn` or by implementing a trait);
    * How near-sdk-rs automatically derives mutability of the method based on which one is used: &self, &mut self, or self. When to use pure methods that do not have `self` argument. Mutable methods, vs view methods vs pure methods;
    * Different flavors of a private method, e.g. what is `#[private]`
    * Payable vs non-payable methods;
    * Comparison of JSON vs Borsh interfaces, with pros and cons, and advises on when to use which. What is `U128` and why do we have it;
* Cross-contract calls
    * Why do we need to write traits, like this: https://github.com/near/near-sdk-rs/blob/master/examples/cross-contract-high-level/src/lib.rs#L26
    * What do #[callback] decorators do and why do we need them both on the arguments of the trait method: https://github.com/near/near-sdk-rs/blob/master/examples/cross-contract-high-level/src/lib.rs#L30 and the arguments of the implementation: https://github.com/near/near-sdk-rs/blob/master/examples/cross-contract-high-level/src/lib.rs#L106
    * What is `#[serializer(*)]`, and why we cannot have some argument serialized with JSON while other arguments serialized with Borsh
    * A comparison of high-level vs low-level API that we can extract from examples here: https://github.com/near/near-sdk-rs/tree/master/examples/cross-contract-low-level and https://github.com/near/near-sdk-rs/tree/master/examples/cross-contract-high-level
    * How much gas to attach to the cross contract call. In what case should we use fixed number vs fraction of the remaining gas;
* Creating transactions
    * Sending tokens, creating accounts and deploying contracts based on the examples here: https://github.com/near/near-sdk-rs/blob/master/examples/cross-contract-high-level/src/lib.rs
* Important nuances
    * What keys have special meaning in the state API and why we wouldn't want them to be tampered with;
* Building contracts
    * Basic building of the contract using cargo, and explanation on why do we need RUSTFLAGS https://github.com/near/near-sdk-rs/blob/master/examples/flags.sh#L4
    * Reproducible builds using https://github.com/near/near-sdk-rs/tree/master/contact-builder and why it is very important;
    * Different tools, like wasm-opt, wasm-gc, etc used here: https://github.com/near/near-sdk-rs/blob/master/minifier/minify.sh
* Running and debugging contracts
    * How to run a contract using local node or on testnet. E.g. we can repurpose some outdated instructions from: https://github.com/near/near-sdk-rs/tree/master/examples/cross-contract-high-level
    * Debugging using CLion;
    * How to use `log!` and where to see it in near-cli response or explorer;
    * Deploying on Mainnet;
* Testing (we don't need to go too deep with simulation tests, until they are polished):
    * Unit tests with explanations on what they can do and what they cannot do;
        * In what cases we want to reset testing environment using testing_env! more than once in a single unit test: https://github.com/near/near-sdk-rs/blob/master/examples/lockable-fungible-token/src/lib.rs#L402
    * Simulation tests, very high-level overview based on https://github.com/near/near-sdk-rs/blob/master/examples/fungible-token/tests/general.rs 