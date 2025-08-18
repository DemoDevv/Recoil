use std::{fmt::Debug, sync::Arc};

use futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use tokio::sync::Semaphore;
use tracing::{info, instrument, warn};

use crate::participant::TxParticipant;

mod errors;
mod metrics;
mod participant;

const MAX_CONCURRENT_OPS: usize = 10;

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

    #[instrument(skip(self))]
    fn rollback<'a>(&'a self) -> BoxFuture<'a, bool> {
        async move {
            info!("Rolling back client {}", self.0);
            true
        }
        .boxed()
    }
}

#[derive(Debug)]
struct Coordinator {
    semaphore: Arc<Semaphore>,
}

impl Coordinator {
    fn new() -> Self {
        Coordinator {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_OPS)),
        }
    }

    /// Execute an operation with a semaphore
    async fn execute_with_semaphore<T, F, Fut>(&self, operation: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let _permit = self.semaphore.clone().acquire_owned().await.unwrap();
        operation().await
    }

    #[instrument(skip(self))]
    fn start_transaction<T: TxParticipant>(&self, clients: Vec<Arc<T>>) -> Transaction<T> {
        info!("Starting new transaction");
        if clients.is_empty() {
            warn!("No clients provided in transaction");
            return TransactionBuilder::new(clients)
                .state(TxState::Aborted)
                .build();
        }

        Transaction::new(clients)
    }

    /// Prepare the transaction
    /// Use parallelism to prepare clients
    /// If any client fails to prepare, abort the transaction
    async fn prepare_clients<T: TxParticipant>(&self, tx: &mut Transaction<T>) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| self.execute_with_semaphore(move || c.prepare()))
            .collect::<FuturesUnordered<_>>(); // doesn't have a limit

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            if !success {
                warn!("[{}] Client failed to prepare", tx.id);
                all_success = false;
            }
        }

        if all_success {
            info!("[{}] All clients prepared", tx.id);
            tx.prepare();
        } else {
            tx.abort();
        }
    }

    /// Commit the transaction
    /// Use parallelism to commit clients
    /// If any client fails to commit, abort the transaction
    async fn commit_clients<T: TxParticipant>(&self, tx: &mut Transaction<T>) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| self.execute_with_semaphore(move || c.commit()))
            .collect::<FuturesUnordered<_>>(); // doesn't have a limit

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            if !success {
                warn!("[{}] Client failed to commit", tx.id);
                all_success = false;
            }
        }

        if all_success {
            info!("[{}] All clients committed", tx.id);
            tx.commit();
        } else {
            tx.abort();
        }
    }

    async fn rollback_clients<T: TxParticipant>(&self, tx: &mut Transaction<T>) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| self.execute_with_semaphore(move || c.rollback()))
            .collect::<FuturesUnordered<_>>(); // doesn't have a limit

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            if !success {
                all_success = false;
            }
        }

        if all_success {
            info!("[{}] All rollbacks succeeded", tx.id);
        } else {
            warn!("[{}] Some rollbacks failed", tx.id);
        }
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
    id: uuid::Uuid,
    state: TxState,
    clients: Vec<Arc<T>>,
}

impl<T: TxParticipant> Transaction<T> {
    fn new(clients: Vec<Arc<T>>) -> Self {
        Transaction {
            id: uuid::Uuid::new_v4(),
            state: TxState::Created,
            clients,
        }
    }

    fn is_aborted(&self) -> bool {
        self.state == TxState::Aborted
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

/// Transaction builder
/// used to build a transaction
struct TransactionBuilder<T: TxParticipant> {
    state: Option<TxState>,
    clients: Vec<Arc<T>>,
}

impl<T: TxParticipant> TransactionBuilder<T> {
    fn new(clients: Vec<Arc<T>>) -> Self {
        TransactionBuilder {
            state: None,
            clients,
        }
    }

    /// Set the state of the transaction
    fn state(mut self, state: TxState) -> Self {
        self.state = Some(state);
        self
    }

    /// Build a transaction
    fn build(self) -> Transaction<T> {
        Transaction {
            id: uuid::Uuid::new_v4(),
            state: self.state.unwrap_or(TxState::Created),
            clients: self.clients,
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let client_a = Arc::new(Client(1));
    let client_b = Arc::new(Client(2));

    let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

    let coordinator = Coordinator::new();

    let mut tx = coordinator.start_transaction(clients);
    let _ = coordinator.prepare_clients(&mut tx).await;
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

    #[test]
    fn test_start_transaction_without_participants() {
        let clients: Vec<Arc<Client>> = vec![];
        let coordinator = Coordinator::new();

        let tx = coordinator.start_transaction(clients.clone());

        assert!(clients.is_empty());
        assert_eq!(tx.state, TxState::Aborted)
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
    async fn test_abort_on_prepare_failure() {
        #[derive(Debug)]
        struct FaillingClient;

        impl TxParticipant for FaillingClient {
            fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn rollback<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }
        }

        let client = Arc::new(FaillingClient);

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(vec![Arc::clone(&client)]);

        coordinator.prepare_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_abort_on_commit_failure() {
        #[derive(Debug)]
        struct FaillingClient;

        impl TxParticipant for FaillingClient {
            fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn rollback<'a>(&'a self) -> BoxFuture<'a, bool> {
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

    #[tokio::test]
    async fn test_state_after_successful_rollback() {
        #[derive(Debug)]
        struct FaillingClient;

        impl TxParticipant for FaillingClient {
            fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { true }.boxed()
            }

            fn rollback<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { true }.boxed()
            }
        }

        let client_a = Arc::new(FaillingClient);
        let client_b = Arc::new(FaillingClient);

        let coordinator = Coordinator::new();

        let mut tx =
            coordinator.start_transaction(vec![Arc::clone(&client_a), Arc::clone(&client_b)]);

        coordinator.prepare_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);

        coordinator.rollback_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_state_after_partial_rollback_failure() {
        #[derive(Debug)]
        struct MockClient {
            should_fail: bool,
        }

        impl TxParticipant for MockClient {
            fn prepare<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { false }.boxed()
            }

            fn commit<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { true }.boxed()
            }

            fn rollback<'a>(&'a self) -> BoxFuture<'a, bool> {
                async { !self.should_fail }.boxed()
            }
        }

        let client_a = Arc::new(MockClient { should_fail: false });
        let client_b = Arc::new(MockClient { should_fail: true });

        let coordinator = Coordinator::new();

        let mut tx =
            coordinator.start_transaction(vec![Arc::clone(&client_a), Arc::clone(&client_b)]);

        coordinator.prepare_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);

        coordinator.rollback_clients(&mut tx).await;

        assert_eq!(tx.state, TxState::Aborted);
    }
}
