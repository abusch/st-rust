use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn, debug};

use crate::protocol::TypedMessage;

use super::ConnectionState;


/// Handles messages from the reader.
///
/// Can optionally send out messages to the writer.
pub struct ConnectionDispatcher {
    inbox: mpsc::Receiver<TypedMessage>,
    state: ConnectionState,
}

impl ConnectionDispatcher {
    pub fn new(inbox: mpsc::Receiver<TypedMessage>) -> Self {
        Self {
            inbox,
            state: ConnectionState::Initial,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection message dispatcher...");
        loop {
            let msg = if let Some(frame) = self.inbox.recv().await {
                frame
            } else {
                info!("Shutting down");
                break;
            };
            if let Err(e) = self.dispatch(&msg).await {
                warn!(%e, ?msg, "Failed to dispatch message");
            }
        }
    }

    async fn dispatch(&mut self, message: &TypedMessage) -> Result<()> {
        match message {
            TypedMessage::ClusterConfig(_) => {
                debug!("Got ClusterConfig from peer: switching connection state to Ready");
                self.state = ConnectionState::Ready;
            }
            TypedMessage::Index(_) => {
                debug!("Got Index message from peer.");
            }
            TypedMessage::IndexUpdate(_) => {}
            TypedMessage::Request(_) => {}
            TypedMessage::Response(_) => {}
            TypedMessage::DownloadProgress(_) => {}
            TypedMessage::Ping(_) => {}
            TypedMessage::Close(_) => {}
        }

        Ok(())
    }
}


