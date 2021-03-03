//! Module to deal with device ids.
use std::{
    convert::{TryFrom, TryInto},
    fmt::Display,
    str::FromStr,
};

use anyhow::{ensure, Result};
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
        let base32 = BASE32_NOPAD.encode(&self.0);
        let luhnified = luhnify(&base32.as_bytes()).unwrap();
        let chunkified = chunkify(&luhnified);

        // SAFETY: the bytes come from BASE32 data, and so belong to [A-Z2-7], which
        // are valid utf-8 characters. Same with the lunh32 checkpoints. '-'
        // separators are inserted, which are also valid utf-8 characters.
        unsafe { String::from_utf8_unchecked(chunkified) }
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
        // De-chunkify
        let dechunkified = dechunkify(&s.as_bytes());

        // De-luhnify
        let deluhnified = deluhnify(&dechunkified)?;
        let decoded = BASE32_NOPAD.decode(&deluhnified)?;

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

fn luhnify(s: &[u8]) -> Result<Vec<u8>> {
    assert!(s.len() == 52, "Unsupported string length");
    let mut res = Vec::with_capacity(56);

    for chunk in s.chunks_exact(13) {
        res.extend_from_slice(chunk);
        let check = super::luhn::luhn32(chunk)?;
        res.push(check);
    }

    Ok(res)
}

fn deluhnify(s: &[u8]) -> Result<Vec<u8>> {
    ensure!(s.len() == 56);

    let mut res = Vec::with_capacity(52);
    for chunk in s.chunks_exact(14) {
        let data = &chunk[..13];
        let checksum = chunk[13];
        let computed_checksum = super::luhn::luhn32(data)?;
        ensure!(
            computed_checksum == checksum,
            "Device ID checksum validation failed!"
        );
        res.extend_from_slice(data);
    }

    Ok(res)
}

fn chunkify(s: &[u8]) -> Vec<u8> {
    let num_chunks = s.len() / 7;
    let mut res = Vec::with_capacity(num_chunks * (7 + 1) - 1);
    for (i, chunk) in s.chunks_exact(7).enumerate() {
        res.extend_from_slice(chunk);
        if i != num_chunks - 1 {
            res.push(b'-');
        }
    }

    res
}

fn dechunkify(s: &[u8]) -> Vec<u8> {
    let mut dechunked = Vec::with_capacity(56);
    s.split(|c| *c == b'-')
        .for_each(|chunk| dechunked.extend_from_slice(chunk));
    dechunked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunkify() {
        assert_eq!(
            chunkify(b"012345601234560123456"),
            b"0123456-0123456-0123456"
        );
    }

    #[test]
    fn dechunkify() {
        let s = b"UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD";
        assert_eq!(
            super::dechunkify(s),
            b"UYA4Y6ARMU7JXNRU4CXE422XPLPZ67VCOXJPLGGPIDKC25D3ABBREDQD"
        );
    }

    #[test]
    fn id_from_string() {
        let s = "UYA4Y6A-RMU7JXN-RU4CXE4-22XPLPZ-67VCOXJ-PLGGPID-KC25D3A-BBREDQD".to_string();
        let res = DeviceId::try_from(s);
        assert!(res.is_ok());
    }
}
