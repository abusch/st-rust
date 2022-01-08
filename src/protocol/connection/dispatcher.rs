use std::sync::Arc;

use anyhow::Result;
use tokio::{
    select,
    sync::{mpsc, watch},
};
use tracing::{debug, info, warn};

use crate::{
    model::Model,
    protocol::{DeviceId, TypedMessage},
};

use super::ConnectionState;

/// Handles messages from the reader.
///
/// Can optionally send out messages to the writer.
pub struct ConnectionDispatcher {
    id: DeviceId,
    model: Arc<Model>,
    inbox: mpsc::Receiver<TypedMessage>,
    state: ConnectionState,
    shutdown_rx: watch::Receiver<bool>,
}

impl ConnectionDispatcher {
    pub fn new(
        remote_id: DeviceId,
        model: Arc<Model>,
        inbox: mpsc::Receiver<TypedMessage>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            id: remote_id,
            model,
            inbox,
            state: ConnectionState::Initial,
            shutdown_rx,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection message dispatcher...");
        loop {
            select! {
            Some(msg) = self.inbox.recv() => {
                if let Err(e) = self.dispatch(msg).await {
                    warn!(%e, "Failed to dispatch message");
                }
            },
            _ = self.shutdown_rx.changed() => {
                info!("Shutting down connection dispatcher");
                break;
            }
            }
        }
    }

    async fn dispatch(&mut self, message: TypedMessage) -> Result<()> {
        match message {
            TypedMessage::ClusterConfig(cm) => {
                debug!("Got ClusterConfig from peer: switching connection state to Ready");
                self.state = ConnectionState::Ready;
                self.model.cluster_config(self.id, cm);
            }
            TypedMessage::Index(idx) => {
                debug!("Got Index message from peer.");
                self.model.index(self.id, idx);
            }
            TypedMessage::IndexUpdate(_idx) => {
                debug!("Got IndexUpdate message from peer.");
            }
            TypedMessage::Request(_) => {}
            TypedMessage::Response(_) => {}
            TypedMessage::DownloadProgress(_) => {}
            TypedMessage::Ping(_) => {}
            TypedMessage::Close(_) => {}
        }

        Ok(())
    }
}
