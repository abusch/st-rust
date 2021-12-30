//! This module contains code to handle a connection to a peer.
use std::{io::Cursor, time::Duration};

use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use tracing::{debug, error, info, trace, warn};
use tracing_futures::Instrument;

use crate::protos::{ClusterConfig, Ping};

use super::{AsyncTypedMessage, TypedMessage};

/// Handle to a connection to a peer.
pub struct ConnectionHandle {
    outbox: mpsc::Sender<AsyncTypedMessage>,
}

impl ConnectionHandle {
    /// Creates a new connection handle for the given TLS connection.
    ///
    /// Note that it will spawn several tasks to manage the connection, so this
    /// should be called from within an async context.
    pub fn new<T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static>(conn: T) -> Self {
        let (reader, writer) = tokio::io::split(conn);
        let (inbox_tx, inbox_rx) = mpsc::channel(1024);
        let (outbox_tx, outbox_rx) = mpsc::channel(1024);
        let (last_msg_received_tx, last_msg_received_rx) = watch::channel(Instant::now());
        let (last_msg_sent_tx, last_msg_sent_rx) = watch::channel(Instant::now());

        let mut connection_reader = ConnectionReader::new(reader, inbox_tx, last_msg_received_tx);
        let mut connection_writer = ConnectionWriter::new(writer, outbox_rx, last_msg_sent_tx);
        let mut connection_dispatcher = ConnectionDispatcher::new(inbox_rx);
        let connection_ping_receiver: ConnectionPingReceiver =
            ConnectionPingReceiver::new(last_msg_received_rx);
        let connection_ping_sender: ConnectionPingSender =
            ConnectionPingSender::new(outbox_tx.clone(), last_msg_sent_rx);

        tokio::spawn(async move {
            connection_reader
                .run()
                .instrument(tracing::info_span!("connection_reader"))
                .await
        });
        tokio::spawn(async move {
            connection_writer
                .run()
                .instrument(tracing::info_span!("connection_writer"))
                .await
        });
        tokio::spawn(async move {
            connection_dispatcher
                .run()
                .instrument(tracing::info_span!("connection_dispatcher"))
                .await
        });
        tokio::spawn(async move {
            connection_ping_receiver
                .run()
                .instrument(tracing::info_span!("connection_ping_receiver"))
                .await
        });
        tokio::spawn(async move {
            connection_ping_sender
                .run()
                .instrument(tracing::info_span!("connection_ping_sender"))
                .await
        });

        Self {
            outbox: outbox_tx,
        }
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

/// Task that listens to incoming messages from a peer.
///
/// When a messages arrives, it is deserialized then sent to the dispatcher.
pub struct ConnectionReader<T> {
    conn: T,
    inbox: mpsc::Sender<TypedMessage>,
    buffer: BytesMut,
    last_msg_received: watch::Sender<Instant>,
}

impl<T> ConnectionReader<T>
where
    T: AsyncRead + Unpin + Send + Sync + 'static,
{
    pub fn new(
        conn: T,
        inbox: mpsc::Sender<TypedMessage>,
        last_msg_received: watch::Sender<Instant>,
    ) -> Self {
        Self {
            conn,
            inbox,
            buffer: BytesMut::with_capacity(1024),
            last_msg_received,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection reader...");
        loop {
            match self.read_message().await {
                Ok(Some(msg)) => {
                    debug!("Dispatching message...");
                    match self.inbox.send(msg).await {
                        Ok(_) => {
                            // Message was sent to dispatcher, notify the ping receiver task
                            if let Err(e) = self.last_msg_received.send(Instant::now()) {
                                warn!(%e, "Failed to send last_msg_received timestamp");
                            }
                        }
                        Err(_) => {
                            info!("Inbox receiver was dropped. Shutting down.");
                            break;
                        }
                    }
                }
                Ok(None) => {
                    info!("shutting down...");
                    break;
                }
                Err(e) => {
                    error!(err = %e, "Error reading frame");
                    break;
                }
            }
        }
    }

    async fn read_message(&mut self) -> Result<Option<TypedMessage>> {
        loop {
            // If there is enough data in the buffer for a frame, return it
            if let Some(msg) = self.parse_message()? {
                return Ok(Some(msg));
            }

            trace!("Not enough data in the buffer, waiting for more data...");
            let n = self.conn.read_buf(&mut self.buffer).await?;
            if n == 0 {
                // We got EOF, check the buffer to see if there was left-over data
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(anyhow!("Connection reset by peer"));
                }
            } else {
                trace!("Read {} bytes", n);
            }
        }
    }

    fn parse_message(&mut self) -> Result<Option<TypedMessage>> {
        let mut buf = Cursor::new(&self.buffer[..]);
        trace!("{} bytes available in buffer", buf.remaining());
        if TypedMessage::check_size(&mut buf) {
            // Reset position at the beginning so we can parse
            buf.set_position(0);
            let res = TypedMessage::parse(&mut buf).map(Some);
            // The position of the cursor after we parsed a frame is the length
            // of the data we've consumed
            let len = buf.position() as usize;
            trace!("Consumed {} bytes", len);
            self.buffer.advance(len);

            res
        } else {
            // Not enough data
            Ok(None)
        }
    }
}

/// Task to send messages to the peer.
///
/// When a new message it received from the dispatcher, it is serialized then
/// written to the underlying connection.
pub struct ConnectionWriter<T> {
    conn: T,
    outbox: mpsc::Receiver<AsyncTypedMessage>,
    buf: BytesMut,
    last_msg_sent: watch::Sender<Instant>,
}

impl<T> ConnectionWriter<T>
where
    T: AsyncWrite + Sync + Send + 'static + Unpin,
{
    pub fn new(
        conn: T,
        outbox: mpsc::Receiver<AsyncTypedMessage>,
        last_msg_sent_tx: watch::Sender<Instant>,
    ) -> Self {
        Self {
            conn,
            outbox,
            buf: BytesMut::with_capacity(1024),
            last_msg_sent: last_msg_sent_tx,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection writer...");
        loop {
            if let Some(AsyncTypedMessage { msg, done }) = self.outbox.recv().await {
                if let Err(e) = self.send_msg(msg).await {
                    warn!(err = %e, "Failed to send message");
                } else {
                    // Keep track of when we last sent a message to this peer so we know when to send a Ping
                    self.last_msg_sent.send(Instant::now()).unwrap_or_else(
                        |e| warn!(err=%e, "Failed to send last_msg_sent timestamp"),
                    );
                    // Notify the caller that we've sent the message down the wire
                    done.send(()).unwrap_or_else(|_| {
                        warn!("Failed to notify caller: the receiver was dropped")
                    });
                }
            } else {
                info!("Shutting down");
                break;
            }
        }
    }

    async fn send_msg(&mut self, msg: TypedMessage) -> Result<()> {
        self.buf.clear();

        msg.write_to_bytes(&mut self.buf)?;
        self.conn.write_all(&self.buf[..]).await?;

        Ok(())
    }
}

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

struct ConnectionPingReceiver {
    last_message_received: watch::Receiver<Instant>,
    receive_duration: Duration,
}

impl ConnectionPingReceiver {
    fn new(last_message_received: watch::Receiver<Instant>) -> Self {
        Self {
            last_message_received,
            receive_duration: Duration::from_secs(90),
        }
    }

    pub async fn run(&self) {
        info!("Starting connection Ping receiver...");
        let mut interval = tokio::time::interval(self.receive_duration / 2);
        loop {
            let tick_time = interval.tick().await;
            let last_received_time = self.last_message_received.borrow();
            if tick_time.duration_since(*last_received_time) >= self.receive_duration {
                // TODO timeout the connection!
                warn!("No ping received for more than 90 seconds! Closing the connection (TODO)");
            }
        }
    }
}

pub struct ConnectionPingSender {
    outbox: mpsc::Sender<AsyncTypedMessage>,
    last_msg_sent: watch::Receiver<Instant>,
    send_duration: Duration,
}

impl ConnectionPingSender {
    pub fn new(
        outbox: mpsc::Sender<AsyncTypedMessage>,
        last_msg_sent: watch::Receiver<Instant>,
    ) -> Self {
        Self {
            outbox,
            last_msg_sent,
            send_duration: Duration::from_secs(90),
        }
    }

    pub async fn run(&self) {
        info!("Starting connection Ping sender...");
        let mut interval = tokio::time::interval(self.send_duration / 2);
        loop {
            let tick_time = interval.tick().await;
            let last_sent_time = *self.last_msg_sent.borrow();
            if tick_time <= last_sent_time {
                // This can happen when we send the ClusterConfig message and
                // the PingSender task hasn't started yet. Just ignore it...
                continue;
            }
            if tick_time.duration_since(last_sent_time) >= self.send_duration {
                debug!("No message sent in the last 90 seconds. Sending Ping...");

                let (ping, done) = self.ping_msg();
                if let Err(err) = self.outbox.send(ping).await {
                    warn!(%err, "Failed to send Ping message");
                }
                done.await.unwrap_or_else(
                    |e| warn!(err=%e, "Error while waiting to be notified. Sender was dropped?"),
                );
            }
        }
    }

    fn ping_msg(&self) -> (AsyncTypedMessage, oneshot::Receiver<()>) {
        let (done_tx, done_rx) = oneshot::channel();
        (
            AsyncTypedMessage {
                msg: TypedMessage::Ping(Ping::default()),
                done: done_tx,
            },
            done_rx,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Initial,
    Ready,
}
