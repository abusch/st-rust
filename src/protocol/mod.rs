pub mod connection;
mod deviceid;
mod luhn;
mod model;

use anyhow::Result;
use bytes::{Buf, BufMut, Bytes};
use prost::Message;
use tokio::sync::oneshot;
use tracing::debug;

use crate::protos::{
    Close, ClusterConfig, DownloadProgress, Header, Index, IndexUpdate, MessageCompression,
    MessageType, Ping, Request, Response,
};
pub use deviceid::DeviceId;

/// Magic number used in `Announce` packets
pub const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();

/// Represent the different message defined in the BEP protocol.
///
/// We wrap them in this enum so we can pass them around as a unified type (for
/// example sending them over a channel) while still encoding the type
/// information.
#[derive(Debug)]
pub enum TypedMessage {
    ClusterConfig(ClusterConfig),
    Index(Index),
    IndexUpdate(IndexUpdate),
    Request(Request),
    Response(Response),
    DownloadProgress(DownloadProgress),
    Ping(Ping),
    Close(Close),
}

impl TypedMessage {
    /// Returns true if there is enough data in the buffer to parse a full frame
    /// (header + Message)
    pub fn check_size(buf: &mut impl Buf) -> bool {
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
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let hdr_len = buf.get_u16();
        debug!("Header length = {}", hdr_len);
        let hdr_bytes = buf.copy_to_bytes(hdr_len as usize);
        let header = Header::decode(&hdr_bytes[..])?;
        debug!(?header, "Got header");

        let msg_len = buf.get_u32();
        let mut msg_bytes = buf.copy_to_bytes(msg_len as usize);

        if header.compression() == MessageCompression::Lz4 {
            debug!("Got a compressed message");
            let decompressed_msg = lz4_flex::decompress_size_prepended(&msg_bytes[..])?;
            debug!("Decompressed data size: {} bytes", decompressed_msg.len());
            msg_bytes = Bytes::from(decompressed_msg);
        }

        let msg = match header.r#type() {
            MessageType::ClusterConfig => {
                let m = ClusterConfig::decode(&msg_bytes[..])?;
                debug!("Got ClusterConfig with {} folders", m.folders.len());
                Self::ClusterConfig(m)
            }
            MessageType::Index => Self::Index(Index::decode(&msg_bytes[..])?),
            MessageType::IndexUpdate => Self::IndexUpdate(IndexUpdate::decode(&msg_bytes[..])?),
            MessageType::Request => Self::Request(Request::decode(&msg_bytes[..])?),
            MessageType::Response => Self::Response(Response::decode(&msg_bytes[..])?),
            MessageType::DownloadProgress => {
                Self::DownloadProgress(DownloadProgress::decode(&msg_bytes[..])?)
            }
            MessageType::Ping => Self::Ping(Ping::decode(&msg_bytes[..])?),
            MessageType::Close => Self::Close(Close::decode(&msg_bytes[..])?),
        };

        debug!(?msg, "Got message");
        Ok(msg)
    }

    pub fn header(&self) -> Header {
        let mut header = Header::default();
        header.r#type = self.message_type().into();
        header.set_compression(MessageCompression::None);

        header
    }

    pub fn message_type(&self) -> MessageType {
        match self {
            Self::ClusterConfig(_) => MessageType::ClusterConfig,
            Self::Index(_) => MessageType::Index,
            Self::IndexUpdate(_) => MessageType::IndexUpdate,
            Self::Request(_) => MessageType::Request,
            Self::Response(_) => MessageType::Response,
            Self::DownloadProgress(_) => MessageType::DownloadProgress,
            Self::Ping(_) => MessageType::Ping,
            Self::Close(_) => MessageType::Close,
        }
    }

    pub fn write_to_bytes(&self, buf: &mut impl BufMut) -> Result<()> {
        let header = self.header();
        header.write_len16_and_bytes(buf)?;

        match self {
            Self::ClusterConfig(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::Index(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::IndexUpdate(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::Request(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::Response(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::DownloadProgress(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::Ping(m) => {
                m.write_len32_and_bytes(buf)?;
            }
            Self::Close(m) => {
                m.write_len32_and_bytes(buf)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct AsyncTypedMessage {
    msg: TypedMessage,
    done: oneshot::Sender<()>,
}

pub trait MessageExt: Message {
    fn write_len16_and_bytes(&self, buf: &mut impl BufMut) -> Result<()>;
    fn write_len32_and_bytes(&self, buf: &mut impl BufMut) -> Result<()>;
}

impl<T> MessageExt for T
where
    T: Message + Sized,
{
    fn write_len16_and_bytes(&self, buf: &mut impl BufMut) -> Result<()> {
        buf.put_u16(self.encoded_len() as u16);
        self.encode(buf)?;
        Ok(())
    }

    fn write_len32_and_bytes(&self, buf: &mut impl BufMut) -> Result<()> {
        buf.put_u32(self.encoded_len() as u32);
        self.encode(buf)?;
        Ok(())
    }
}
