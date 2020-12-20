oysterpack-near-stake-token Smart Contract
==================

Exploring The Code
==================
- `lib.rs` 
   - defines the contract and initialization code
- `interface` module
   - defines the contract interface using rust traits
-  `interface::model` module
   - defines the model for the interface
- `contract` - module
   - contains the interface implementations
   - mirrors the `interface` in structure
   - also contains the `settings` module - used to initialize the contract
- `domain` module
   - defines the contract domain model used - also used for storage data model
- `core` module
   - contains core code used throughout the project
- `config`
   - defines the contract config data model
- `near` module
   - contains NEAR utilities
- `test-utils` module
   - contains code to support unit testing
  

## NOTES
- when `near_bindgen` macro is used outside of lib.rs, the following import is necessary in order to compile:
```rust
use crate::*;
```
