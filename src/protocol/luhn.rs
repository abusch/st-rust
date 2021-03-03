use anyhow::{anyhow, Result};

static LUHN_BASE32: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

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
pub fn luhn32(input: &[u8]) -> Result<u8> {
    let mut factor = 1;
    let mut sum: u32 = 0;
    let n = 32;

    for ch in input {
        let codepoint = codepoint32(*ch).ok_or_else(|| {
            anyhow!(
                "character {} is not valid BASE32. It should be one of [A-Z2-7]",
                ch,
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
        let s = b"AB725E4GHIQPL3ZFGT";
        assert_eq!(luhn32(s).unwrap(), b'G');
    }

    #[test]
    fn test_invalid() {
        let s = b"3734EJEKMRHWPZQTWYQ1";
        assert!(luhn32(s).is_err());
    }
}
