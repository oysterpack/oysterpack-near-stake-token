# OysterPack STAKE Token NEAR Smart Contract

> With the OysterPack NEAR STAKE token "You can have your STAKE and TRADE it too"

The OysterPack NEAR STAKE token contract enables you to stake your NEAR tokens and still be able to trade your staked
tokens. The contract will stake your NEAR for you and in return you are issued a fungible token named **STAKE**. 

The STAKE token transforms your staked NEAR into a tradeable asset. STAKE token value is pegged to NEAR token value and 
stake earnings. As staking rewards are earned, the STAKE token value increases. STAKE tokens appreciate in NEAR token 
value over time.

STAKE token value is pegged to NEAR token value and stake earnings. As staking rewards are earned,
the STAKE token value increases. In other words, STAKE tokens appreciate in NEAR token value over
time. In addition, the contract provides the following yield boosting levers:
1. the contract owner can share a percentage of the contract's gas rewards with STAKE user accounts
   to boost yield. When funds are staked, contract gas earning will be distributed to STAKE users
   by staking the NEAR funds into the staking pool, which increases the staked NEAR balance, which
   increases the STAKE token value.
2. the contract supports collecting earnings from other contracts into the STAKE token contract.
   The collected earnings are pooled with the STAKE Token contract gas earnings and distributed
   to the contract owner and user accounts.

When redeeming STAKE tokens for NEAR, the STAKE token contract also helps to add liquidity for withdrawing your unstaked 
NEAR tokens (see below for more details)

## Problems with NEAR's core staking pool contract that the STAKE token solves
Today, users can delegate the NEAR to be staked with validator pools through NEAR's [core staking pool contract](https://github.com/near/core-contracts/tree/master/staking-pool).
When NEAR is transferred to the staking pool contract, the NEAR is effectively locked while being staked. When the user
wants their NEAR tokens back, they need to unstake their tokens with the staking pool contract. However, the user's 
NEAR tokens are not immediately available for withdrawal. The unstaked tokens are locked in the contract and not made
available for withdrawal until 4 epochs later in NEAR blockchain time, which translates to ~48 hours.

Thus, the staked NEAR token value is effectively locked in the staking pool contract and the user cannot leverage the
value until 2 days after the NEAR tokens are unstaked. 

The STAKE token is the solution to the problem:
1. It enables the token owner to leverage the staked NEAR value while it is locked by the staking pool contract.
2. It adds liquidity to the unstaking and withdrawal process - staking provides the liquidity for unstaking

# STAKE Token Vision
> harness the Internet of value - everything on the internet can take on the proerties of money

The above quote is cited from [NEAR's website](https://near.org/). This is the STAKE token vision by leveraging NEAR as a 
digital currency beyond being a utility token for the NEAR network to pay for transaction gas and storage usage. 
NEAR is designed to be scalable and fast with very low and predictable transaction costs and pricing. NEAR tokenomics has 
built in inflation, with a 5% maximum inflation target. The inflation provides incentive to stake your NEAR, which helps 
to further secure the network. Transforming staked NEAR into a tradeable asset via the STAKE token enhances the value proposition. 
Since most of the NEAR token supply will be staked, we can get more value out of the staked NEAR by being able to use it as a tradeable digital asset.

## How to make this vision a reality
To make this vision a reality, we need to develop robust token standards to pave the way.
[NEP-21 Fungible Token Standard](https://github.com/near/NEPs/issues/21) was NEAR's first attempt to introduce a fungible
token standard modeled after ethereum ERC-20 tokens. NEP-21 is not robust enough to support the long term strategic vision.

Alternative fungible token standards have been proposed through the [NEP](https://github.com/near/NEPs) process:
- [NEP-122 Proposal: Allowance-free vault-based token standard](https://github.com/near/NEPs/issues/122)
- [NEP-110 Advanced Fungible Token Standard](https://github.com/near/NEPs/issues/110)
- [NEP-136 Interactive Fungible Token](https://github.com/near/NEPs/issues/136)
- [NEP-102 Native Fungible Token](https://github.com/near/NEPs/issues/102)

One of the goals of this project is to bring this all together and help drive the community forward towards a robust
and scalable fungible token standard that can support and harness the Internet of value.