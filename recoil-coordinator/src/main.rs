use std::{fmt::Debug, sync::Arc};

use futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use tracing::{info, instrument};

use crate::participant::TxParticipant;

mod participant;

#[derive(Debug, PartialEq, Eq)]
struct Client(u32);

impl TxParticipant for Client {
    #[instrument(skip(self))]
    fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
        async move {
            info!("Preparing client {}", self.0);
            true
        }
        .boxed()
    }

    #[instrument(skip(self))]
    fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
        async move {
            info!("Committing client {}", self.0);
            true
        }
        .boxed()
    }
}

#[derive(Debug)]
struct Coordinator;

impl Coordinator {
    fn new() -> Self {
        Coordinator
    }

    #[instrument(skip(self))]
    fn start_transaction<T: TxParticipant>(&self, clients: Vec<Arc<T>>) -> Transaction<T> {
        info!("Starting transaction");
        Transaction::new(clients)
    }

    /// Prepare the transaction
    /// Use parallelism to prepare clients
    /// If any client fails to prepare, abort the transaction
    async fn prepare_clients<T: TxParticipant>(&self, tx: &mut Transaction<T>) {
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| c.prepare())
            .collect::<FuturesUnordered<_>>();

        while let Some(success) = futures.next().await {
            if !success {
                tx.abort();
                return;
            }
        }

        tx.prepare();
    }

    /// Commit the transaction
    /// Use parallelism to commit clients
    /// If any client fails to commit, abort the transaction
    async fn commit_clients<T: TxParticipant>(&self, tx: &mut Transaction<T>) {
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| c.commit())
            .collect::<FuturesUnordered<_>>();

        while let Some(success) = futures.next().await {
            if !success {
                tx.abort();
                return;
            }
        }

        tx.commit();
    }
}

/// Transaction state
/// used to track the state of a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxState {
    Created,
    Prepared,
    Committed,
    Aborted,
}

struct Transaction<T: TxParticipant> {
    state: TxState,
    clients: Vec<Arc<T>>,
}

impl<T: TxParticipant> Transaction<T> {
    fn new(clients: Vec<Arc<T>>) -> Self {
        Transaction {
            state: TxState::Created,
            clients,
        }
    }

    fn abort(&mut self) {
        self.state = TxState::Aborted;
    }

    fn prepare(&mut self) {
        self.state = TxState::Prepared;
    }

    fn commit(&mut self) {
        self.state = TxState::Committed;
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let client_a = Arc::new(Client(1));
    let client_b = Arc::new(Client(2));

    let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

    let coordinator = Coordinator::new();

    let _ = coordinator.start_transaction(clients);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_transaction() {
        let client_a = Arc::new(Client(1));
        let client_b = Arc::new(Client(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let tx = coordinator.start_transaction(clients.clone());

        assert_eq!(tx.state, TxState::Created);
        assert_eq!(tx.clients, clients);
    }

    #[tokio::test]
    async fn test_prepare_clients() {
        let client_a = Arc::new(Client(1));
        let client_b = Arc::new(Client(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(clients);

        coordinator.prepare_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Prepared);
    }

    #[tokio::test]
    async fn test_commit_clients() {
        let client_a = Arc::new(Client(1));
        let client_b = Arc::new(Client(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(clients);

        coordinator.prepare_clients(&mut tx).await;

        coordinator.commit_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Committed);
    }

    #[tokio::test]
    async fn test_failling_client() {
        #[derive(Debug)]
        struct FaillingClient;

        impl TxParticipant for FaillingClient {
            fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }
        }

        let client = Arc::new(FaillingClient);

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(vec![Arc::clone(&client)]);

        coordinator.prepare_clients(&mut tx).await;

        coordinator.commit_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);
    }
}
