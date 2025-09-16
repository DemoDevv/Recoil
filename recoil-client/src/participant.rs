use futures::future::BoxFuture;
use std::fmt::Debug;

use recoil_errors::transaction::TxError;

pub type ClientResult<'a> = BoxFuture<'a, Result<bool, TxError>>;

/// Trait for a transaction participant.
/// Can be used to participate in a transaction.
/// Useful for mocking or testing purposes.
pub trait TxParticipant: Debug + Send + Sync {
    fn prepare(&self) -> ClientResult<'_>;
    fn commit(&self) -> ClientResult<'_>;
    fn rollback(&self) -> ClientResult<'_>;
}
