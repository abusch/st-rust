use anyhow::Result;
use bytes::BytesMut;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    select,
    sync::{mpsc, watch},
    time::Instant,
};
use tracing::{info, warn};

use crate::protocol::{AsyncTypedMessage, TypedMessage};

/// Task to send messages to the peer.
///
/// When a new message it received from the dispatcher, it is serialized then
/// written to the underlying connection.
pub struct ConnectionWriter<T> {
    conn: T,
    outbox: mpsc::Receiver<AsyncTypedMessage>,
    buf: BytesMut,
    last_msg_sent: watch::Sender<Instant>,
    shutdown_rx: watch::Receiver<bool>,
}

impl<T> ConnectionWriter<T>
where
    T: AsyncWrite + Sync + Send + 'static + Unpin,
{
    pub fn new(
        conn: T,
        outbox: mpsc::Receiver<AsyncTypedMessage>,
        last_msg_sent_tx: watch::Sender<Instant>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            conn,
            outbox,
            buf: BytesMut::with_capacity(1024),
            last_msg_sent: last_msg_sent_tx,
            shutdown_rx,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting connection writer...");
        loop {
            select! {
            Some(AsyncTypedMessage { msg, done }) = self.outbox.recv() => {
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
            },
            _ = self.shutdown_rx.changed() => {
                info!("Shutting down");
                break;
            }
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
