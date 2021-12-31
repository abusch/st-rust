use std::io::Cursor;

use anyhow::{Result, anyhow};
use bytes::{BytesMut, Buf};
use tokio::{sync::{mpsc, watch}, time::Instant, io::{AsyncRead, AsyncReadExt}};
use tracing::{info, debug, warn, error, trace};

use crate::protocol::TypedMessage;

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


