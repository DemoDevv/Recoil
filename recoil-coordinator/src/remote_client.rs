use futures::FutureExt;
use tracing::{info, instrument};

use recoil_client::participant::{ClientResult, TxParticipant};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RemoteClient(pub(crate) u32);

impl TxParticipant for RemoteClient {
    #[instrument(skip(self))]
    fn prepare(&self) -> ClientResult<'_> {
        async move {
            info!("Sending preparing to client {}", self.0);
            Ok(true)
        }
        .boxed()
    }

    #[instrument(skip(self))]
    fn commit(&self) -> ClientResult<'_> {
        async move {
            info!("Sending committing to client {}", self.0);
            Ok(true)
        }
        .boxed()
    }

    #[instrument(skip(self))]
    fn rollback(&self) -> ClientResult<'_> {
        async move {
            info!("Sending rolling back to client {}", self.0);
            Ok(true)
        }
        .boxed()
    }
}
