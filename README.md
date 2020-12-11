# OysterPack NEAR STAKE Token
The OysterPack STAKE token is backed by staked NEAR tokens. 
It enables you to trade your staked NEAR, i.e., you can stake your NEAR and use it as money.

When you stake your NEAR, it will get locked up within the staking pool contract.
OysterPack will issue STAKE token for staked NEAR. 

STAKE token value is pegged to NEAR token value and stake earnings. As staking rewards are earned, the STAKE token value 
increases. In other words, STAKE tokens appreciate in NEAR token value over time.

There is one STAKE token contract per staking pool. 

### How is the STAKE token valued
STAKE token value in NEAR = `total staked NEAR balance / total STAKE token supply`

## Account Registration and Storage Fees
Customers must first register their accounts to be able to use the contract. The account is responsible to pay for its account storage.
As part of the account registration process, customers are required to attach a deposit to pay for account storage fees.
Storage fee deposits are escrowed and refunded when the customer unregisters their account.

# How staking works
1. The customer deposits NEAR into the STAKE token contract
2. The STAKE token contract batches together deposits from multiple customers.
3. Staking requests are serialized, i.e., the contract is locked while a staking batch request is being processed. 
4. If the contract is locked, then the request is put into the next batch that runs.

# How redeeming STAKE tokens for NEAR works
There is a limitation in the staking pool contracts that needs to be worked around. Unstaked NEAR is not available for
withdrawal for 4 epochs. However, if another unstaking transaction is submitted, then the total unstaked NEAR balance
is locked for another 4 epochs. For example, 50 NEAR are unstaked at epoch 100, which means the 50 NEAR is available
for withdrawal at epoch 104. However, if a user submits a transaction to unstake another 50 NEAR at epoch 103, then
the entire 100 unstaked NEAR will be available to be withdrawn at epoch 107. In this example, in order to be able to 
withdraw the 50 NEAR at epoch 104, the 2nd unstaking request must be submitted after the NEAR is withdrawn.

To work around this staking pool limitation, the scheduling algorithm needs to take this into consideration. When a redeem 
STAKE batch is run, the epoch height is recorded. A batch will not be run unless it has been at least 4 epochs since the 
last batch run and after all available funds have been withdrawn from the staking pool.

### Workflow
1. Customer submits request to redeem STAKE
2. The STAKE contract batches together requests from multiple customers
3. Batches are processed serially, i.e., the batch transaction must acquire the contract lock to run the batch.
4. In addition, batches to redeem STAKE can only be run if there are no pending withdrawals from a prior batch.

# How funds are claimed from processed batches
When a batch is processed, the STAKE token value at that point in time is computed and recorded in the contract state on
the blockchain. Each time an account performs an action, the account checks if there are any batch receipts to apply and
updates account balances accordingly. Explicit contract function will be exposed to process the account's batch receipts.

## Notes
- while the contract is locked, customer requests to stake NEAR and redeem STAKE will be scheduled into the next batch
  - the contract is locked in order to compute STAKE token value which requires balances to be static
- clients can query the contract to check if it is locked
- anyone can kickoff batches to run