use futures::future::BoxFuture;
use std::fmt::Debug;

/// Trait for a transaction participant.
/// Can be used to participate in a transaction.
/// Useful for mocking or testing purposes.
pub trait TxParticipant: Debug + Send + Sync {
    fn prepare<'a>(&'a self) -> BoxFuture<'a, bool>;
    fn commit<'a>(&'a self) -> BoxFuture<'a, bool>;
}
