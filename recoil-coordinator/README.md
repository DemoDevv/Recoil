# recoil-coordinator

The Transaction Coordinator for Recoil, will be a microservice for managing distributed transactions.

## How it works

The Transaction Coordinator will be responsible to create global transactions and coordinate them across a distributed system.
He can handle a lot of registered Ressource Managers and coordinate them by sending messages like commit or rollback. The coordinator will also be responsible for handling any failures or errors that may occur during the transaction process. if an error occurs, the coordinator will rollback the transaction and notify all registered resource managers. For that he needs to follow the status of each TM.
