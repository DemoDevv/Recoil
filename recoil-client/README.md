This crate define a custom wrapper for client (Ressources Manager).
The client can communicate with the coordinator with gRPC or HTTP protocol (TCP).
This crate will be use by other backend application for handle distributed transactions.

The SDK has the following architecture:

- RecoilClient: A client that communicates with the coordinator using gRPC or HTTP protocol (TCP) and registers itself.
- RecoilServer/sdk-server: A server for handling incoming requests from the coordinator.
- TxParticipant: A trait defined by the user to implement the participant logic. Can be implemented with Postgres or any other database.
