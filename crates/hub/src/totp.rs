//! TOTP (RFC 6238) primitives for two-factor auth + RFC 4648 base32, hand-rolled
//! to avoid extra dependencies. Pure functions — no DB, no state — so the login
//! wiring (see docs/auth-2fa-passkey.md) stays separable and these stay unit-tested
//! against the RFC test vectors.
//!
//! Defaults match every authenticator app: SHA-1, 6 digits, 30-second step.
#![allow(dead_code)] // wired into the auth/login flow in a follow-up (see the design doc)

use hmac::{Hmac, Mac};
use sha1::Sha1;

const STEP: u64 = 30;
const DIGITS: u32 = 6;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encode bytes as RFC 4648 base32 (no padding) — the format authenticator apps
/// expect in the `secret=` of an `otpauth://` URI.
pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// Decode an RFC 4648 base32 string (case-insensitive, padding/space tolerant).
pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.chars() {
        if c == '=' || c.is_whitespace() {
            continue;
        }
        let v = ALPHABET
            .iter()
            .position(|&a| a == c.to_ascii_uppercase() as u8)? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// The TOTP code for `secret` at unix time `now`, with `skew` half-window (0 = exact
/// step). Zero-padded to 6 digits.
fn code_at(secret: &[u8], counter: u64) -> u32 {
    type HmacSha1 = Hmac<Sha1>;
    let mut mac = HmacSha1::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let bin = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    bin % 10u32.pow(DIGITS)
}

/// Format a code as a zero-padded 6-digit string.
pub fn format_code(code: u32) -> String {
    format!("{code:0width$}", width = DIGITS as usize)
}

/// Verify `input` against `secret` at `now` (unix seconds), accepting the current
/// step and one step either side (±30s) to tolerate clock drift. Constant-ish: we
/// always check all three steps. `input` may contain spaces.
pub fn verify(secret: &[u8], input: &str, now: u64) -> bool {
    verify_step(secret, input, now).is_some()
}

/// Like [`verify`] but returns the matching 30s step (so callers can enforce that a
/// code is never accepted twice — reject any step <= the last accepted one).
pub fn verify_step(secret: &[u8], input: &str, now: u64) -> Option<u64> {
    let cleaned: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if cleaned.len() != DIGITS as usize {
        return None;
    }
    let want = cleaned.parse::<u32>().ok()?;
    let step = now / STEP;
    [step.wrapping_sub(1), step, step + 1]
        .into_iter()
        .find(|&s| code_at(secret, s) == want)
}

/// Build the `otpauth://totp/...` URI an authenticator app scans/imports.
pub fn provisioning_uri(secret_b32: &str, account: &str, issuer: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    let label = format!("{}:{}", enc(issuer), enc(account));
    format!(
        "otpauth://totp/{label}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits={DIGITS}&period={STEP}",
        enc(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4648 §10 base32 test vectors.
    #[test]
    fn base32_vectors() {
        assert_eq!(base32_encode(b""), "");
        assert_eq!(base32_encode(b"f"), "MY");
        assert_eq!(base32_encode(b"fo"), "MZXQ");
        assert_eq!(base32_encode(b"foo"), "MZXW6");
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("MZXW6YTBOI").unwrap(), b"foobar");
        assert_eq!(base32_decode("mzxw6ytboi").unwrap(), b"foobar"); // case-insensitive
        assert_eq!(base32_decode("MZXW 6YTB OI").unwrap(), b"foobar"); // space-tolerant
    }

    // RFC 6238 Appendix B test vectors (SHA-1, seed "12345678901234567890").
    #[test]
    fn rfc6238_vectors() {
        let secret = b"12345678901234567890";
        // (unix time, expected 8-digit code) → we use 6 digits, so take the last 6.
        let cases = [
            (59u64, "287082"),
            (1111111109, "081804"),
            (1111111111, "050471"),
            (1234567890, "005924"),
            (2000000000, "279037"),
        ];
        for (t, want) in cases {
            assert_eq!(format_code(code_at(secret, t / STEP)), want, "t={t}");
        }
    }

    #[test]
    fn verify_accepts_drift_and_rejects_wrong() {
        let secret = b"12345678901234567890";
        let now = 1111111111u64;
        assert!(verify(secret, "050471", now)); // exact step
        assert!(verify(secret, "081804", now)); // previous step (drift)
        assert!(!verify(secret, "000000", now));
        assert!(!verify(secret, "12345", now)); // wrong length
        assert!(!verify(secret, "abcdef", now));
    }
}
