#[derive(Debug)]
pub enum TxError {
    CommitFailed,
    RollbackFailed,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TxError::CommitFailed => write!(f, "Commit failed"),
            TxError::RollbackFailed => write!(f, "Rollback failed"),
        }
    }
}

impl std::error::Error for TxError {}
