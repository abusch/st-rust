//! Module to deal with device ids.
use std::{
    convert::{TryFrom, TryInto},
    fmt::Display,
    str::FromStr,
};

use anyhow::{anyhow, ensure, Result};
use ascii::{AsciiChar, AsciiStr, AsciiString};
use bytes::Bytes;
use data_encoding::BASE32_NOPAD;
use ring::digest::{digest, Digest, SHA256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    pub fn new(id: &[u8]) -> Self {
        DeviceId(
            id.try_into()
                .expect("Invalid length slice length! Expecting 32."),
        )
    }

    pub fn from_der_cert(cert: &[u8]) -> DeviceId {
        let sha256: Digest = digest(&SHA256, cert);

        DeviceId(
            sha256
                .as_ref()
                .try_into()
                .expect("SHA256 has wrong length!"),
        )
    }

    pub fn to_bytes(&self) -> Bytes {
        Bytes::copy_from_slice(&self.0[..])
    }

    pub fn to_string(&self) -> String {
        let base32 = AsciiString::from_ascii(BASE32_NOPAD.encode(&self.0))
            .expect("BASE32_NOPAD.encode() produced invalid characters!");
        let luhnified = luhnify(&base32).unwrap();
        let chunkified = chunkify(&luhnified);

        chunkified.into()
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl FromStr for DeviceId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ascii_str = AsciiString::from_ascii(s)
            .map_err(|e| anyhow!("Device ID string contained non ascii characters: {}", e))?;
        // De-chunkify
        let dechunkified = dechunkify(&ascii_str);

        // De-luhnify
        let deluhnified = deluhnify(&dechunkified)?;
        let decoded = BASE32_NOPAD.decode(deluhnified.as_bytes())?;

        Ok(DeviceId::new(&decoded[..]))
    }
}

impl TryFrom<String> for DeviceId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        DeviceId::from_str(&value)
    }
}

impl From<DeviceId> for String {
    fn from(id: DeviceId) -> Self {
        id.to_string()
    }
}

fn luhnify(s: &AsciiStr) -> Result<AsciiString> {
    assert!(s.len() == 52, "Unsupported string length");
    let mut res = AsciiString::with_capacity(56);

    for chunk in chunk_str(s, 13) {
        res.push_str(chunk);
        let check = super::luhn::luhn32(chunk)?;
        res.push(check);
    }

    Ok(res)
}

fn deluhnify(s: &AsciiStr) -> Result<AsciiString> {
    ensure!(s.len() == 56);

    let mut res = AsciiString::with_capacity(52);
    for chunk in chunk_str(s, 14) {
        let data = &chunk[..13];
        let checksum = chunk[13];
        let computed_checksum = super::luhn::luhn32(data)?;
        ensure!(
            computed_checksum == checksum,
            "Device ID checksum validation failed!"
        );
        res.push_str(data);
    }

    Ok(res)
}

fn chunkify(s: &AsciiStr) -> AsciiString {
    let num_chunks = s.len() / 7;
    let mut res = AsciiString::with_capacity(num_chunks * (7 + 1) - 1);
    for (i, chunk) in chunk_str(s, 7).enumerate() {
        res.push_str(chunk);
        if i != num_chunks - 1 {
            res.push(AsciiChar::Minus);
        }
    }

    res
}

fn dechunkify(s: &AsciiStr) -> AsciiString {
    let mut dechunked = AsciiString::with_capacity(56);
    s.split(AsciiChar::Minus)
        .for_each(|chunk| dechunked.push_str(chunk));
    dechunked
}

/// Return an iterator over chunks of the input string of size `size`.
///
/// Note that the input size is expected to be a multiple of the size. Any remaining bit at the end will be dropped.
fn chunk_str(s: &AsciiStr, size: usize) -> ChunkedStr {
    ChunkedStr { size, remaining: s }
}

pub struct ChunkedStr<'a> {
    size: usize,
    remaining: &'a AsciiStr,
}

impl<'a> Iterator for ChunkedStr<'a> {
    type Item = &'a AsciiStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.len() < self.size {
            None
        } else {
            let chunk = &self.remaining[..self.size];
            self.remaining = &self.remaining[self.size..];
            Some(chunk)
        }
    }
}

#[cfg(test)]
mod tests {
    use ascii::AsAsciiStr;

    use super::*;

    #[test]
    fn test_chunkify() {
        assert_eq!(
            chunkify("012345601234560123456".as_ascii_str().unwrap()),
            "0123456-0123456-0123456".to_string()
        );
    }

    #[test]
    fn dechunkify() {
        let s = "UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD"
            .as_ascii_str()
            .unwrap();
        assert_eq!(
            "UYA4Y6ARMU7JXNRU4CXE422XPLPZ67VCOXJPLGGPIDKC25D3ABBREDQD",
            super::dechunkify(s).as_str()
        );
    }

    #[test]
    fn chunked_str() {
        let s = "123456789".as_ascii_str().unwrap();
        let expected = vec!["123", "456", "789"];
        let chunks = super::chunk_str(s, 3)
            .map(|s| s.as_str())
            .collect::<Vec<_>>();
        assert_eq!(expected, chunks);
    }

    #[test]
    fn id_from_string() {
        let s = "UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD".to_string();
        let res = DeviceId::try_from(s);
        assert!(res.is_ok());
    }
}
