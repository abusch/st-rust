//! Module to deal with device ids.
use std::{convert::TryInto, fmt::Display};

use anyhow::Result;
use data_encoding::BASE32_NOPAD;
use ring::digest::{digest, Digest, SHA256};

#[derive(Debug)]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    pub fn from_der_cert(cert: &[u8]) -> DeviceId {
        let sha256: Digest = digest(&SHA256, cert);

        DeviceId(
            sha256
                .as_ref()
                .try_into()
                .expect("SHA256 has wrong length!"),
        )
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base32 = BASE32_NOPAD.encode(&self.0);
        let luhnified =
            luhnify(&base32).expect("BASE32_NOPAD.encode() produced invalid characters!");
        let chunkified = chunkify(&luhnified);
       write!(f, "{}", &chunkified)
    }
}

fn luhnify(s: &str) -> Result<String> {
    assert!(s.len() == 52, "Unsupported string length");
    let mut res = String::with_capacity(56);

    for chunk in chunk_str(s, 13) {
        res.push_str(chunk);
        let check = super::luhn::luhn32(chunk)?;
        res.push(check);
    }

    Ok(res)
}

fn chunkify(s: &str) -> String {
    let num_chunks = s.len() / 7;
    let mut res = String::with_capacity(num_chunks * (7 + 1) - 1);
    for (i, chunk) in chunk_str(s, 7).enumerate() {
        res.push_str(chunk);
        if i != num_chunks - 1 {
            res.push('-');
        }
    }

    res
}

fn chunk_str(s: &str, size: usize) -> ChunkedStr {
    ChunkedStr { size, remaining: s }
}

pub struct ChunkedStr<'a> {
    size: usize,
    remaining: &'a str,
}

impl<'a> Iterator for ChunkedStr<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.len() < self.size {
            None
        } else {
            let (chunk, remaining) = self.remaining.split_at(self.size);
            self.remaining = remaining;
            Some(chunk)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunkify() {
        assert_eq!(
            chunkify("012345601234560123456"),
            "0123456-0123456-0123456".to_string()
        );
    }
}
