# OysterPack STAKE Token NEAR Smart Contract
The OysterPack STAKE token is backed by staked NEAR. This contract enables you to delegate your
NEAR to stake, and in return you are issued STAKE tokens. This enables you to trade your STAKE
tokens while your NEAR is staked and earning staking rewards. The STAKE token transforms your
staked NEAR into a tradeable asset.

STAKE token value is pegged to NEAR token value and stake earnings. As staking rewards are earned,
the STAKE token value increases. In other words, STAKE tokens appreciate in NEAR token value over
time.

## STAKE Token Vision
Leverage NEAR as a digital currency beyond being a utility token for the NEAR network to pay for
transaction gas and storage usage. NEAR is designed to be scalable and fast with very low and
predictable transaction costs and pricing. NEAR tokenomics has built in inflation, with a 5%
maximum inflation target. The inflation provides incentive to stake your NEAR, which helps to further
secure the network. Transforming staked NEAR into a tradeable asset via the STAKE token enhances
the value proposition. Since most of the NEAR token supply will be staked, we can get more value
out of the staked NEAR by being able to use it as a tradeable digital asset.

The long term vision is to integrate the STAKE token with the NEAR wallet:
- users would be able to stake their NEAR via this contract
- users would be able to transfer STAKE tokens via the NEAR wallet

## Problem With Current Unstaking Process
With the current staking pool implementations, the problem is that unstaked NEAR is not immediately
available for withdrawal from the staking pool contract. The unstaked NEAR is locked for 4 epoch
time periods, which translates to ~48 hours in NEAR time. This makes it more difficult and complex
to utilize NEAR as a digital asset, i.e., as a fungible token.

## STAKE Token Benefits
1. NEAR token asset value is maximized through staking.
2. Transforms staked NEAR into tradeable digital asset, i.e., into a fungible token.
3. Provides more incentive to stake NEAR, which helps to further strengthen and secure the network
   by providing more economic incentive to validators.

# Contract Key Features and High Level Design
- Contract users must register with the account in order to use it. Users must pay an upfront
  account storage usage fee because long term storage is not "free" on NEAR. When an account
  unregisters, the storage usage fee will be refunded.
- STAKE token contract is linked to a single staking pool contract that is specified as part of
  contract deployment and becomes permanent for contract's lifetime. A STAKE token contract will
  be deployed per staking pool contract.
- Implements [NEP-122 vault based fungible token standard](https://github.com/near/NEPs/issues/122)
  - NEAR community is currently trying to standardize fungible token interface. STAKE token implements
    NEP-122 Vault Based Fungible Token (WIP), but waiting for NEP-122 standard to be finalized.
- Has concept of contract ownership. The contract owner earns the contract rewards from transaction
  fees.
  - contract ownership can be transferred
  - contract earning can be staked into the contract owner's account
- Contract has an operator role which provides functions to support the contract, e.g., releasing
  locks, config management, etc

## How is the STAKE token value computed?
STAKE token value in NEAR = `total staked NEAR balance / total STAKE token supply`
