use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr};
use lazy_static::lazy_static;

lazy_static! {
    static ref LUHN_BASE32: &'static AsciiStr =
        unsafe { AsciiStr::from_ascii_unchecked(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567") };
}

fn codepoint32(b: u8) -> Option<u8> {
    if (b'A'..=b'Z').contains(&b) {
        Some(b - b'A')
    } else if (b'2'..=b'7').contains(&b) {
        Some(b + 26 - b'2')
    } else {
        None
    }
}

/// Computes the luhn32 checksum character of the given ascii string.
pub fn luhn32(s: &AsciiStr) -> Result<AsciiChar> {
    let mut factor = 1;
    let mut sum: u32 = 0;
    let n = 32;

    for ch in s.chars() {
        let codepoint = codepoint32(ch as u8).ok_or_else(|| {
            anyhow!(
                "digit {} is not valid in alphabet {}",
                ch,
                LUHN_BASE32.as_str()
            )
        })?;
        let mut addend = factor * codepoint;
        addend = (addend / n) + (addend % n);
        sum += addend as u32;
        factor = if factor == 2 { 1 } else { 2 };
    }

    let remainder = (sum % n as u32) as u8;
    let check_codepoint = (n - remainder) % n;
    Ok(LUHN_BASE32[check_codepoint as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok() {
        let s = AsciiStr::from_ascii(b"AB725E4GHIQPL3ZFGT").unwrap();
        assert_eq!(luhn32(s).unwrap(), 'G');
    }

    #[test]
    fn test_invalid() {
        let s = AsciiStr::from_ascii(b"3734EJEKMRHWPZQTWYQ1").unwrap();
        assert!(luhn32(s).is_err());
    }
}
