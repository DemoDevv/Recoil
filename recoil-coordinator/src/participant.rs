use futures::future::BoxFuture;
use std::fmt::Debug;

use crate::errors::TxError;

pub type ClientResult<'a> = BoxFuture<'a, Result<bool, TxError>>;

/// Trait for a transaction participant.
/// Can be used to participate in a transaction.
/// Useful for mocking or testing purposes.
pub trait TxParticipant: Debug + Send + Sync {
    fn prepare<'a>(&'a self) -> ClientResult<'a>;
    fn commit<'a>(&'a self) -> ClientResult<'a>;
    fn rollback<'a>(&'a self) -> ClientResult<'a>;
}
