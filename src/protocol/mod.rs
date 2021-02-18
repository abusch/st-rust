pub mod connection;
mod deviceid;
mod luhn;
mod model;

pub use deviceid::DeviceId;

/// Magic number used in `Announce` packets
pub const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();
