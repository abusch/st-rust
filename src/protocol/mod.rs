mod deviceid;
mod luhn;

pub use deviceid::DeviceId;

/// Magic number used in `Announce` packets
pub const MAGIC: &[u8] = &0x2EA7D90Bu32.to_be_bytes();
