//! This module contains code to handle a connection to a peer.
use std::net::SocketAddr;

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use tracing::Instrument;

use crate::protos::ClusterConfig;

use super::{AsyncTypedMessage, TypedMessage};

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
    outbox: mpsc::Sender<AsyncTypedMessage>,
}

impl ConnectionHandle {
    /// Creates a new connection handle for the given TLS connection.
    ///
    /// Note that it will spawn several tasks to manage the connection, so this
    /// should be called from within an async context.
    pub fn new<T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static>(peer_addr: SocketAddr, conn: T, shutdown_rx: watch::Receiver<bool>) -> Self {
        let (reader, writer) = tokio::io::split(conn);
        let (inbox_tx, inbox_rx) = mpsc::channel(1024);
        let (outbox_tx, outbox_rx) = mpsc::channel(1024);
        let (last_msg_received_tx, last_msg_received_rx) = watch::channel(Instant::now());
        let (last_msg_sent_tx, last_msg_sent_rx) = watch::channel(Instant::now());

        let mut connection_reader = ConnectionReader::new(reader, inbox_tx, last_msg_received_tx, shutdown_rx.clone());
        let mut connection_writer = ConnectionWriter::new(writer, outbox_rx, last_msg_sent_tx, shutdown_rx.clone());
        let mut connection_dispatcher = ConnectionDispatcher::new(inbox_rx, shutdown_rx.clone());
        let connection_ping_receiver: ConnectionPingReceiver =
            ConnectionPingReceiver::new(last_msg_received_rx, shutdown_rx.clone());
        let connection_ping_sender: ConnectionPingSender =
            ConnectionPingSender::new(outbox_tx.clone(), last_msg_sent_rx, shutdown_rx);

        tokio::spawn(async move {
            connection_reader
                .run()
                .instrument(tracing::info_span!("connection_reader", %peer_addr))
                .await
        });
        tokio::spawn(async move {
            connection_writer
                .run()
                .instrument(tracing::info_span!("connection_writer", %peer_addr))
                .await
        });
        tokio::spawn(async move {
            connection_dispatcher
                .run()
                .instrument(tracing::info_span!("connection_dispatcher", %peer_addr))
                .await
        });
        tokio::spawn(async move {
            connection_ping_receiver
                .run()
                .instrument(tracing::info_span!("connection_ping_receiver", %peer_addr))
                .await
        });
        tokio::spawn(async move {
            connection_ping_sender
                .run()
                .instrument(tracing::info_span!("connection_ping_sender", %peer_addr))
                .await
        });

        Self { outbox: outbox_tx }
    }

    // pub async fn close(&mut self) -> Result<()> {
    //     self.outbox.send(Frame::Close(Close::new())).await
    // }

    pub async fn config_cluster(&mut self) -> Result<()> {
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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Initial,
    Ready,
}
