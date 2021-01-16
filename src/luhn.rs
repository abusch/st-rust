use anyhow::{anyhow, Result};

const LUHN_BASE32: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn codepoint32(b: u8) -> Option<u8> {
    if (b'A'..=b'Z').contains(&b) {
        Some(b - b'A')
    } else if (b'2'..=b'7').contains(&b) {
        Some(b + 26 - b'2')
    } else {
        None
    }
}

pub fn luhn32(s: &str) -> Result<char> {
    let mut factor = 1;
    let mut sum: u32 = 0;
    let n = 32;

    for c in s.chars() {
        let codepoint = codepoint32(c as u8)
            .ok_or_else(|| anyhow!("digit {} is not valid in alphabet {}", c, LUHN_BASE32))?;
        let mut addend = factor * codepoint;
        addend = (addend / n) + (addend % n);
        sum += addend as u32;
        factor = if factor == 2 { 1 } else { 2 };
    }

    let remainder = (sum % n as u32) as u8;
    let check_codepoint = (n - remainder) % n;
    LUHN_BASE32
        .chars()
        .nth(check_codepoint as usize)
        .ok_or_else(|| anyhow!("Invalid check codepoint: {}", check_codepoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok() {
        assert_eq!(luhn32("AB725E4GHIQPL3ZFGT").unwrap(), 'G');
    }

    #[test]
    fn test_invalid() {
        assert!(luhn32("3734EJEKMRHWPZQTWYQ1").is_err());
    }
}
