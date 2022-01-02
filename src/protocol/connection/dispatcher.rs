use anyhow::Result;
use tokio::{sync::{mpsc, watch}, select};
use tracing::{info, warn, debug};

use crate::protocol::TypedMessage;

use super::ConnectionState;


/// Handles messages from the reader.
///
/// Can optionally send out messages to the writer.
pub struct ConnectionDispatcher {
    inbox: mpsc::Receiver<TypedMessage>,
    state: ConnectionState,
    shutdown_rx: watch::Receiver<bool>,
}

impl ConnectionDispatcher {
    pub fn new(inbox: mpsc::Receiver<TypedMessage>, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            inbox,
            state: ConnectionState::Initial,
            shutdown_rx
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection message dispatcher...");
        loop {
            select! {
            Some(msg) = self.inbox.recv() => {
                if let Err(e) = self.dispatch(&msg).await {
                    warn!(%e, ?msg, "Failed to dispatch message");
                }
            },
            _ = self.shutdown_rx.changed() => {
                info!("Shutting down connection dispatcher");
                break;
            }
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


