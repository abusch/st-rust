//! This module contains code to handle a connection to a peer.
use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, watch, Barrier},
    time::Instant,
};
use tracing::{Instrument, debug};

use crate::{protos::ClusterConfig, model::Model};

use super::{AsyncTypedMessage, DeviceId, TypedMessage};

mod dispatcher;
mod ping;
mod reader;
mod writer;

use dispatcher::ConnectionDispatcher;
use ping::{ConnectionPingReceiver, ConnectionPingSender};
use reader::ConnectionReader;
use writer::ConnectionWriter;

/// Handle to a connection to a peer.
pub struct ConnectionHandle {
    model: Arc<Model>,
    id: DeviceId,
    outbox: mpsc::Sender<AsyncTypedMessage>,
    start: Arc<Barrier>,
}

impl ConnectionHandle {
    /// Creates a new connection handle for the given TLS connection.
    ///
    /// Note that it will spawn several tasks to manage the connection, so this
    /// should be called from within an async context.
    pub fn new<T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static>(
        model: Arc<Model>,
        remote_id: DeviceId,
        peer_addr: SocketAddr,
        conn: T,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        // Barrier used to synchronize the start of all the background tasks.
        // There are 5 tasks, so initialize the barrier with 6. The last `ConnectionHandle` itself
        // will be the last one to wait on it when `start()` is called, which will unblock all the
        // tasks.
        let start = Arc::new(Barrier::new(6));
        let (reader, writer) = tokio::io::split(conn);
        let (inbox_tx, inbox_rx) = mpsc::channel(1024);
        let (outbox_tx, outbox_rx) = mpsc::channel(1024);
        let (last_msg_received_tx, last_msg_received_rx) = watch::channel(Instant::now());
        let (last_msg_sent_tx, last_msg_sent_rx) = watch::channel(Instant::now());

        let mut connection_reader =
            ConnectionReader::new(reader, inbox_tx, last_msg_received_tx, shutdown_rx.clone());
        let mut connection_writer =
            ConnectionWriter::new(writer, outbox_rx, last_msg_sent_tx, shutdown_rx.clone());
        let mut connection_dispatcher = ConnectionDispatcher::new(remote_id, model.clone(), inbox_rx, shutdown_rx.clone());
        let connection_ping_receiver: ConnectionPingReceiver =
            ConnectionPingReceiver::new(last_msg_received_rx, shutdown_rx.clone());
        let connection_ping_sender: ConnectionPingSender =
            ConnectionPingSender::new(outbox_tx.clone(), last_msg_sent_rx, shutdown_rx);

        let barrier_clone = start.clone();
        tokio::spawn(async move {
            barrier_clone.wait().await;
            connection_reader
                .run()
                .instrument(tracing::info_span!("connection_reader", %peer_addr))
                .await
        });
        let barrier_clone = start.clone();
        tokio::spawn(async move {
            barrier_clone.wait().await;
            connection_writer
                .run()
                .instrument(tracing::info_span!("connection_writer", %peer_addr))
                .await
        });
        let barrier_clone = start.clone();
        tokio::spawn(async move {
            barrier_clone.wait().await;
            connection_dispatcher
                .run()
                .instrument(tracing::info_span!("connection_dispatcher", %peer_addr))
                .await
        });
        let barrier_clone = start.clone();
        tokio::spawn(async move {
            barrier_clone.wait().await;
            connection_ping_receiver
                .run()
                .instrument(tracing::info_span!("connection_ping_receiver", %peer_addr))
                .await
        });
        let barrier_clone = start.clone();
        tokio::spawn(async move {
            barrier_clone.wait().await;
            connection_ping_sender
                .run()
                .instrument(tracing::info_span!("connection_ping_sender", %peer_addr))
                .await
        });

        Self {
            model,
            id: remote_id,
            outbox: outbox_tx,
            start,
        }
    }

    pub async fn start(&mut self) {
        // Unblock all the background tasks that have been started
        debug!("Starting all the async tasks...");
        self.start.wait().await;
        debug!("...done");
    }

    // pub async fn close(&mut self) -> Result<()> {
    //     self.outbox.send(Frame::Close(Close::new())).await
    // }

    pub async fn config_cluster(&self) -> Result<()> {
        // TODO generate proper config
        let config = ClusterConfig::default();
        let (done_tx, done_rx) = oneshot::channel();
        self.outbox
            .send(AsyncTypedMessage {
                msg: TypedMessage::ClusterConfig(config),
                done: done_tx,
            })
            .await?;

        done_rx.await?;

        Ok(())
    }

    pub async fn send(&self, msg: TypedMessage) -> Result<()> {
        let (done_tx, done_rx) = oneshot::channel();
        self.outbox
            .send(AsyncTypedMessage {
                msg,
                done: done_tx,
            })
            .await?;

        done_rx.await?;

        Ok(())
    }


    /// Get the connection handle's device id.
    pub fn id(&self) -> DeviceId {
        self.id
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Initial,
    Ready,
}
