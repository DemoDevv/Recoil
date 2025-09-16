# recoil-coordinator

The Transaction Coordinator for Recoil, will be a microservice for managing distributed transactions.

## How it works

The Transaction Coordinator will be responsible to create global transactions and coordinate them across a distributed system.
He can handle a lot of registered Ressource Managers and coordinate them by sending messages like prepare, commit or rollback. The coordinator will also be responsible for handling any failures or errors that may occur during the transaction process. if an error occurs, the coordinator will rollback the transaction and notify all registered resource managers. For that he needs to follow the status of each TM.

For a good communication, the coordonator use the 2PC (two-phase commit):
- Phase 1: Prepare
- Phase 2: Commit or Rollback

### Transaction

Each Transaction will be identified by a unique ID and will have a status that can be one of the following:
- Pending: The transaction is waiting for the coordinator to start the transaction.
- Prepared: The transaction is prepared and waiting for the coordinator to commit or rollback the transaction.
- Committed: The transaction has been committed successfully.
- Rolledback: The transaction has been rolledback successfully.
