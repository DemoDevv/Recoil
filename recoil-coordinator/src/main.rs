use core::future::Future;
use std::{fmt::Debug, sync::Arc, time::Duration};

use futures::{StreamExt, stream::FuturesUnordered};
use tokio::{sync::Semaphore, time::error::Elapsed};
use tracing::{info, instrument, warn};

use recoil_client::participant::TxParticipant;

use crate::{
    remote_client::RemoteClient,
    timeout::{DEFAULT_TIMEOUT_DURATION, TimeoutDuration},
};

mod metrics;
mod remote_client;
mod timeout;

/// Defines the maximum number of concurrent operations allowed.
const MAX_CONCURRENT_OPS: usize = 10;

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

    /// Apply a timeout to an operation
    /// If DEFAULT_TIMEOUT_DURATION is give to the coordinator method, the default timeout will be set to 30 seconds
    async fn timeout<F: Future>(
        &self,
        operation: F,
        duration: TimeoutDuration,
    ) -> Result<F::Output, Elapsed> {
        tokio::time::timeout(duration.unwrap_or(Duration::from_secs(30)), operation).await
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
    async fn prepare_clients<T: TxParticipant>(
        &self,
        tx: &mut Transaction<T>,
        timeout_duration: Option<Duration>,
    ) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| {
                self.timeout(
                    self.execute_with_semaphore(move || c.prepare()),
                    timeout_duration,
                )
            })
            .collect::<FuturesUnordered<_>>();

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            match success {
                Ok(Ok(_)) => (),
                Ok(Err(err)) => {
                    warn!("[{}] Client failed to prepare: {}", tx.id, err);
                    all_success = false;
                }
                Err(timeout_err) => {
                    warn!("[{}] Client failed to prepare: {}", tx.id, timeout_err);
                    all_success = false;
                }
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
    async fn commit_clients<T: TxParticipant>(
        &self,
        tx: &mut Transaction<T>,
        timeout_duration: Option<Duration>,
    ) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| {
                self.timeout(
                    self.execute_with_semaphore(move || c.commit()),
                    timeout_duration,
                )
            })
            .collect::<FuturesUnordered<_>>();

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            match success {
                Ok(Ok(_)) => (),
                Ok(Err(err)) => {
                    warn!("[{}] Client failed to commit: {}", tx.id, err);
                    all_success = false;
                }
                Err(timeout_err) => {
                    warn!("[{}] Client failed to commit: {}", tx.id, timeout_err);
                    all_success = false;
                }
            }
        }

        if all_success {
            info!("[{}] All clients committed", tx.id);
            tx.commit();
        } else {
            tx.abort();
        }
    }

    async fn rollback_clients<T: TxParticipant>(
        &self,
        tx: &mut Transaction<T>,
        timeout_duration: Option<Duration>,
    ) {
        // cheap copy of clients because we use Arc
        let clients = tx.clients.clone();

        let mut futures = clients
            .iter()
            .map(|c| {
                self.timeout(
                    self.execute_with_semaphore(move || c.rollback()),
                    timeout_duration,
                )
            })
            .collect::<FuturesUnordered<_>>();

        let mut all_success = true;

        while let Some(success) = futures.next().await {
            match success {
                Ok(Ok(_)) => (),
                Ok(Err(err)) => {
                    warn!("[{}] Client failed to rollback: {}", tx.id, err);
                    all_success = false;
                }
                Err(timeout_err) => {
                    warn!("[{}] Client failed to rollback: {}", tx.id, timeout_err);
                    all_success = false;
                }
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

    let client_a = Arc::new(RemoteClient(1));
    let client_b = Arc::new(RemoteClient(2));

    let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

    let coordinator = Coordinator::new();

    let mut tx = coordinator.start_transaction(clients);
    let _ = coordinator
        .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::FutureExt;

    use recoil_client::participant::ClientResult;
    use recoil_errors::transaction::TxError;

    use crate::remote_client::RemoteClient;

    #[test]
    fn test_start_transaction() {
        let client_a = Arc::new(RemoteClient(1));
        let client_b = Arc::new(RemoteClient(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let tx = coordinator.start_transaction(clients.clone());

        assert_eq!(tx.state, TxState::Created);
        assert_eq!(tx.clients, clients);
    }

    #[test]
    fn test_start_transaction_without_participants() {
        let clients: Vec<Arc<RemoteClient>> = vec![];
        let coordinator = Coordinator::new();

        let tx = coordinator.start_transaction(clients.clone());

        assert!(clients.is_empty());
        assert_eq!(tx.state, TxState::Aborted)
    }

    #[tokio::test]
    async fn test_prepare_clients() {
        let client_a = Arc::new(RemoteClient(1));
        let client_b = Arc::new(RemoteClient(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(clients);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Prepared);
    }

    #[tokio::test]
    async fn test_commit_clients() {
        let client_a = Arc::new(RemoteClient(1));
        let client_b = Arc::new(RemoteClient(2));

        let clients = vec![Arc::clone(&client_a), Arc::clone(&client_b)];

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(clients);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        coordinator
            .commit_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Committed);
    }

    #[tokio::test]
    async fn test_abort_on_prepare_failure() {
        #[derive(Debug)]
        struct FailingClient;

        impl TxParticipant for FailingClient {
            fn prepare(&self) -> ClientResult<'_> {
                async { Err(TxError::PrepareFailed) }.boxed()
            }

            fn commit(&self) -> ClientResult<'_> {
                async { Err(TxError::CommitFailed) }.boxed()
            }

            fn rollback(&self) -> ClientResult<'_> {
                async { Err(TxError::RollbackFailed) }.boxed()
            }
        }

        let client = Arc::new(FailingClient);

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(vec![Arc::clone(&client)]);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_abort_on_commit_failure() {
        #[derive(Debug)]
        struct FailingClient;

        impl TxParticipant for FailingClient {
            fn prepare(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }

            fn commit(&self) -> ClientResult<'_> {
                async { Err(TxError::CommitFailed) }.boxed()
            }

            fn rollback(&self) -> ClientResult<'_> {
                async { Err(TxError::RollbackFailed) }.boxed()
            }
        }

        let client = Arc::new(FailingClient);

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(vec![Arc::clone(&client)]);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        coordinator
            .commit_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_state_after_successful_rollback() {
        #[derive(Debug)]
        struct FailingClient;

        impl TxParticipant for FailingClient {
            fn prepare(&self) -> ClientResult<'_> {
                async { Err(TxError::PrepareFailed) }.boxed()
            }

            fn commit(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }

            fn rollback(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }
        }

        let client_a = Arc::new(FailingClient);
        let client_b = Arc::new(FailingClient);

        let coordinator = Coordinator::new();

        let mut tx =
            coordinator.start_transaction(vec![Arc::clone(&client_a), Arc::clone(&client_b)]);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);

        coordinator
            .rollback_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_state_after_partial_rollback_failure() {
        #[derive(Debug)]
        struct MockClient {
            should_fail: bool,
        }

        impl TxParticipant for MockClient {
            fn prepare(&self) -> ClientResult<'_> {
                async { Err(TxError::PrepareFailed) }.boxed()
            }

            fn commit(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }

            fn rollback(&self) -> ClientResult<'_> {
                if self.should_fail {
                    async { Err(TxError::RollbackFailed) }.boxed()
                } else {
                    async { Ok(true) }.boxed()
                }
            }
        }

        let client_a = Arc::new(MockClient { should_fail: false });
        let client_b = Arc::new(MockClient { should_fail: true });

        let coordinator = Coordinator::new();

        let mut tx =
            coordinator.start_transaction(vec![Arc::clone(&client_a), Arc::clone(&client_b)]);

        coordinator
            .prepare_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);

        coordinator
            .rollback_clients(&mut tx, DEFAULT_TIMEOUT_DURATION)
            .await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    #[tokio::test]
    async fn test_state_after_timeout_on_prepare() {
        #[derive(Debug)]
        struct TimeoutClient;

        impl TxParticipant for TimeoutClient {
            fn prepare(&self) -> ClientResult<'_> {
                async {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    Ok(true)
                }
                .boxed()
            }

            fn commit(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }

            fn rollback(&self) -> ClientResult<'_> {
                async { Ok(true) }.boxed()
            }
        }

        let client = Arc::new(TimeoutClient);

        let coordinator = Coordinator::new();

        let mut tx = coordinator.start_transaction(vec![Arc::clone(&client)]);

        coordinator
            .prepare_clients(&mut tx, TimeoutDuration::Some(Duration::from_secs(1)))
            .await;

        assert_eq!(tx.state, TxState::Aborted);
    }

    // add more tests for timeout with a custom timeout because i can't wait for the default timeout
}
