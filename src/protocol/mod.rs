pub mod connection;
mod deviceid;
mod luhn;
mod model;

use crate::protos::Close;
use crate::protos::DownloadProgress;
use crate::protos::Ping;
use crate::protos::Response;
use bytes::{Buf, BufMut};
use std::io::Cursor;

use anyhow::{anyhow, Result};
use protobuf::Message;
use tokio::sync::oneshot;
use tracing::debug;

use crate::protos::{
    ClusterConfig, Header, Index, IndexUpdate, MessageCompression, MessageType, Request,
};
pub use deviceid::DeviceId;

/// Magic number used in `Announce` packets
pub const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();

#[derive(Debug)]
pub struct TypedMessage {
    typ: MessageType,
    msg: Box<dyn Message>,
}

impl TypedMessage {
    pub fn new(typ: MessageType, msg: Box<dyn Message>) -> Self {
        Self { typ, msg }
    }

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
    pub fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self> {
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

        let msg: Box<dyn Message> = match header.get_field_type() {
            MessageType::MESSAGE_TYPE_CLUSTER_CONFIG => {
                let m = ClusterConfig::parse_from_bytes(&msg_bytes[..])?;
                debug!("Got ClusterConfig with {} folders", m.get_folders().len());
                Box::new(m)
            }
            MessageType::MESSAGE_TYPE_INDEX => Box::new(Index::parse_from_bytes(&msg_bytes[..])?),
            MessageType::MESSAGE_TYPE_INDEX_UPDATE => {
                Box::new(IndexUpdate::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_REQUEST => {
                Box::new(Request::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_RESPONSE => {
                Box::new(Response::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_DOWNLOAD_PROGRESS => {
                Box::new(DownloadProgress::parse_from_bytes(&msg_bytes[..])?)
            }
            MessageType::MESSAGE_TYPE_PING => Box::new(Ping::parse_from_bytes(&msg_bytes[..])?),
            MessageType::MESSAGE_TYPE_CLOSE => Box::new(Close::parse_from_bytes(&msg_bytes[..])?),
        };

        debug!(?msg, "Got message");
        Ok(TypedMessage::new(header.get_field_type(), msg))
    }

    pub fn header(&self) -> Header {
        let mut header = Header::new();
        header.set_field_type(self.typ);
        header.set_compression(MessageCompression::MESSAGE_COMPRESSION_NONE);

        header
    }

    pub fn write_to_bytes(&self, buf: &mut impl BufMut) -> Result<()> {
        // TODO is there anyway to avoid allocating some Vecs here?
        let header = self.header();
        let header_bytes = header.write_to_bytes()?;
        buf.put_u16(header_bytes.len() as u16);
        buf.put_slice(&header_bytes[..]);

        let bytes = self.msg.write_to_bytes()?;
        buf.put_u32(bytes.len() as u32);
        buf.put_slice(&bytes[..]);

        Ok(())
    }
}

#[derive(Debug)]
pub struct AsyncTypedMessage {
    msg: TypedMessage,
    done: oneshot::Sender<()>,
}
