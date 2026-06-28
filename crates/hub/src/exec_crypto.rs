//! Encryption for users' stored SSH keys (shell/exec — docs/exec-design.md).
//!
//! A user's SSH private key is sealed under a key (KEK) derived from **their own
//! password** via argon2id, then encrypted with XChaCha20-Poly1305. There is no
//! server master key: the hub can unseal a key only while the user supplies their
//! password (at step-up). A DB or env leak alone reveals nothing.
//!
//! Stored blob layout: `nonce(24) || ciphertext+tag`. `kdf_salt` is stored per
//! credential (a dedicated salt — never the auth-hash salt).
#![allow(dead_code)] // wired into the exec_credentials API in Phase 3b

use anyhow::{anyhow, Result};
use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

/// A fresh random argon2 salt for a new credential.
pub fn gen_salt() -> Vec<u8> {
    let mut salt = vec![0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Derive the 32-byte KEK from the user's password and the credential's salt.
fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut kek = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|e| anyhow!("kek derivation failed: {e}"))?;
    Ok(kek)
}

/// Encrypt `plaintext` (the SSH private key) under the password-derived KEK.
/// Returns `nonce || ciphertext`.
pub fn seal(password: &str, salt: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let kek = derive_kek(password, salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&kek));
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow!("encryption failed"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt a blob produced by [`seal`]. Fails (auth error) on the wrong password or
/// any tampering — never returns garbage.
pub fn open(password: &str, salt: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return Err(anyhow!("ciphertext too short"));
    }
    let kek = derive_kek(password, salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&kek));
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    cipher
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| anyhow!("decryption failed (wrong password or tampered data)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let salt = gen_salt();
        let key = b"-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END-----\n";
        let blob = seal("Str0ng&Passphrase!", &salt, key).unwrap();
        assert_ne!(&blob[24..], key); // actually encrypted
        let out = open("Str0ng&Passphrase!", &salt, &blob).unwrap();
        assert_eq!(out, key);
    }

    #[test]
    fn wrong_password_fails() {
        let salt = gen_salt();
        let blob = seal("correct-Horse9Battery", &salt, b"secret").unwrap();
        assert!(open("wrong-Horse9Battery", &salt, &blob).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let salt = gen_salt();
        let mut blob = seal("correct-Horse9Battery", &salt, b"secret").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(open("correct-Horse9Battery", &salt, &blob).is_err());
    }

    #[test]
    fn wrong_salt_fails() {
        let blob = seal("correct-Horse9Battery", &gen_salt(), b"secret").unwrap();
        assert!(open("correct-Horse9Battery", &gen_salt(), &blob).is_err());
    }

    #[test]
    fn nonce_makes_each_ciphertext_unique() {
        let salt = gen_salt();
        let a = seal("correct-Horse9Battery", &salt, b"secret").unwrap();
        let b = seal("correct-Horse9Battery", &salt, b"secret").unwrap();
        assert_ne!(a, b); // random nonce per seal
    }
}
