//! This module contains code to handle a connection to a peer.
use std::{io::Cursor, time::Duration};

use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, BytesMut};
use protobuf::Message;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    select,
    sync::{mpsc, watch},
    time::Instant,
};
use tracing::{debug, error, info, warn};

use crate::protos::{
    Close, ClusterConfig, DownloadProgress, Header, Index, IndexUpdate, MessageCompression,
    MessageType, Ping, Request, Response,
};

/// Handle to a connection to a peer.
pub struct ConnectionHandle {}

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

        let mut connection_reader = ConnectionReader::new(reader, inbox_tx, last_msg_received_tx);
        let mut connection_writer = ConnectionWriter::new(writer, outbox_rx);
        let mut connection_dispatcher = ConnectionDispatcher::new(inbox_rx, outbox_tx);
        let connection_ping_receiver: ConnectionPingReceiver =
            ConnectionPingReceiver::new(last_msg_received_rx);

        tokio::spawn(async move { connection_reader.run().await });
        tokio::spawn(async move { connection_writer.run().await });
        tokio::spawn(async move { connection_dispatcher.run().await });
        tokio::spawn(async move { connection_ping_receiver.run().await });

        Self {}
    }
}

/// Task that listens to incoming messages from a peer.
///
/// When a messages arrives, it is deserialized then sent to the dispatcher.
pub struct ConnectionReader<T> {
    conn: T,
    inbox: mpsc::Sender<Frame>,
    buffer: BytesMut,
    last_msg_received: watch::Sender<Instant>,
}

impl<T> ConnectionReader<T>
where
    T: AsyncRead + Unpin + Send + Sync + 'static,
{
    pub fn new(
        conn: T,
        inbox: mpsc::Sender<Frame>,
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
        loop {
            match self.read_frame().await {
                Ok(Some(frame)) => {
                    debug!("Dispatching frame...");
                    match self.inbox.send(frame).await {
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

    async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            // If there is enough data in the buffer for a frame, return it
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            debug!("Not enough data in the buffer, waiting for more data...");
            let n = self.conn.read_buf(&mut self.buffer).await?;
            if n == 0 {
                // We got EOF, check the buffer to see if there was left-over data
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(anyhow!("Connection reset by peer"));
                }
            } else {
                debug!("Read {} bytes", n);
            }
        }
    }

    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = Cursor::new(&self.buffer[..]);
        debug!("{} bytes available in buffer", buf.remaining());
        if Frame::check_size(&mut buf) {
            // Reset position at the beginning so we can parse
            buf.set_position(0);
            let res = Frame::parse(&mut buf).map(Some);
            // The position of the cursor after we parsed a frame is the length
            // of the data we've consumed
            let len = buf.position() as usize;
            debug!("Consumed {} bytes", len);
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
    outbox: mpsc::Receiver<Frame>,
    buf: BytesMut,
}

impl<T> ConnectionWriter<T>
where
    T: AsyncWrite + Sync + Send + 'static + Unpin,
{
    pub fn new(conn: T, outbox: mpsc::Receiver<Frame>) -> Self {
        Self {
            conn,
            outbox,
            buf: BytesMut::with_capacity(1024),
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(frame) = self.outbox.recv().await {
                if let Err(e) = self.send_msg(frame).await {
                    warn!(err = %e, "Failed to send message");
                }
            } else {
                info!("Shutting down");
                break;
            }
        }
    }

    async fn send_msg(&mut self, frame: Frame) -> Result<()> {
        self.buf.clear();

        frame.write_to_bytes(&mut self.buf)?;
        self.conn.write_all(&self.buf[..]).await?;

        Ok(())
    }
}

/// Handles messages from the reader.
///
/// Can optionally send out messages to the writer.
pub struct ConnectionDispatcher {
    inbox: mpsc::Receiver<Frame>,
    outbox: mpsc::Sender<Frame>,
}

impl ConnectionDispatcher {
    pub fn new(inbox: mpsc::Receiver<Frame>, outbox: mpsc::Sender<Frame>) -> Self {
        Self { inbox, outbox }
    }

    pub async fn run(&mut self) {
        loop {
            let msg = select! {
                res = self.inbox.recv() => {
                    match res {
                        Some(frame) => frame,
                        None => {
                            info!("Shutting down");
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    debug!("Time to send ping");
                    Frame::Ping(Ping::new())
                }
            };
            if let Err(e) = self.dispatch(&msg).await {
                warn!(%e, ?msg, "Failed to dispatch message");
            }
        }
    }

    async fn dispatch(&mut self, message: &Frame) -> Result<()> {
        match message {
            Frame::ClusterConfig(_) => {}
            Frame::Index(_) => {}
            Frame::IndexUpdate(_) => {}
            Frame::Request(_) => {}
            Frame::Response(_) => {}
            Frame::DownloadProgress(_) => {}
            Frame::Ping(_) => {}
            Frame::Close(_) => {}
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

/// TODO rename to something better than Frame...
#[derive(Debug)]
pub enum Frame {
    ClusterConfig(ClusterConfig),
    Index(Index),
    IndexUpdate(IndexUpdate),
    Request(Request),
    Response(Response),
    DownloadProgress(DownloadProgress),
    Ping(Ping),
    Close(Close),
}

impl Frame {
    /// Returns true if there is enough data in the buffer to parse a full frame
    /// (header + Message)
    pub fn check_size(buf: &mut Cursor<&[u8]>) -> bool {
        // Do we have enough to read the header length?
        if buf.remaining() < 2 {
            return false;
        }

        let hdr_len = buf.get_u16();
        // Do we have enough data to read the header?
        if buf.remaining() < hdr_len as usize {
            return false;
        }

        buf.advance(hdr_len as usize);
        if buf.remaining() < 4 {
            return false;
        }
        let msg_len = buf.get_u32();
        if buf.remaining() < msg_len as usize {
            return false;
        }

        true
    }

    /// Parse a frame (header + message).
    ///
    /// The content of the buffer should have already checked with
    /// [`check_size`] to make sure there is enough data in the buffer,
    /// otherwise this will panic.
    pub fn parse(buf: &mut Cursor<&[u8]>) -> Result<Frame> {
        let hdr_len = buf.get_u16();
        debug!("Header length = {}", hdr_len);
        let hdr_bytes = buf.copy_to_bytes(hdr_len as usize);
        let header = Header::parse_from_bytes(&hdr_bytes[..])?;
        debug!(?header, "Got header");

        let msg_len = buf.get_u32();
        let msg_bytes = buf.copy_to_bytes(msg_len as usize);

        if header.get_compression() == MessageCompression::MESSAGE_COMPRESSION_LZ4 {
            // TODO Implement LZ4 compression support
            return Err(anyhow!("Compressed message data is not implemented!"));
        }

        let msg = match header.get_field_type() {
            MessageType::MESSAGE_TYPE_CLUSTER_CONFIG => {
                let m = ClusterConfig::parse_from_bytes(&msg_bytes[..])?;
                debug!("Got ClusterConfig with {} folders", m.get_folders().len());
                Self::ClusterConfig(m)
            }
            MessageType::MESSAGE_TYPE_INDEX => {
                Self::Index(Index::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_INDEX_UPDATE => {
                Self::IndexUpdate(IndexUpdate::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_REQUEST => {
                Self::Request(Request::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_RESPONSE => {
                Self::Response(Response::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_DOWNLOAD_PROGRESS => {
                Self::DownloadProgress(DownloadProgress::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_PING => Self::Ping(Ping::parse_from_bytes(&msg_bytes[..])?),
            MessageType::MESSAGE_TYPE_CLOSE => {
                Self::Close(Close::parse_from_bytes(&msg_bytes[..])?)
            }
        };

        debug!(?msg, "Got message");
        Ok(msg)
    }

    pub fn header(&self) -> Header {
        let mut header = Header::new();
        let msg_type = match *self {
            Frame::ClusterConfig(_) => MessageType::MESSAGE_TYPE_CLUSTER_CONFIG,
            Frame::Index(_) => MessageType::MESSAGE_TYPE_INDEX,
            Frame::IndexUpdate(_) => MessageType::MESSAGE_TYPE_INDEX_UPDATE,
            Frame::Request(_) => MessageType::MESSAGE_TYPE_REQUEST,
            Frame::Response(_) => MessageType::MESSAGE_TYPE_RESPONSE,
            Frame::DownloadProgress(_) => MessageType::MESSAGE_TYPE_DOWNLOAD_PROGRESS,
            Frame::Ping(_) => MessageType::MESSAGE_TYPE_PING,
            Frame::Close(_) => MessageType::MESSAGE_TYPE_CLOSE,
        };
        header.set_field_type(msg_type);
        header.set_compression(MessageCompression::MESSAGE_COMPRESSION_NONE);

        header
    }

    pub fn write_to_bytes(&self, buf: &mut impl BufMut) -> Result<()> {
        // TODO is there anyway to avoid allocating some Vecs here?
        let header = self.header();
        let header_bytes = header.write_to_bytes()?;
        buf.put_u16(header_bytes.len() as u16);
        buf.put_slice(&header_bytes[..]);

        let bytes = match self {
            Frame::ClusterConfig(m) => m.write_to_bytes(),
            Frame::Index(m) => m.write_to_bytes(),
            Frame::IndexUpdate(m) => m.write_to_bytes(),
            Frame::Request(m) => m.write_to_bytes(),
            Frame::Response(m) => m.write_to_bytes(),
            Frame::DownloadProgress(m) => m.write_to_bytes(),
            Frame::Ping(m) => m.write_to_bytes(),
            Frame::Close(m) => m.write_to_bytes(),
        }?;
        buf.put_u32(bytes.len() as u32);
        buf.put_slice(&bytes[..]);

        Ok(())
    }
}
