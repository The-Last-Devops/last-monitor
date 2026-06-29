//! Encryption for users' stored SSH keys (shell/exec — docs/exec-design.md).
//!
//! Each user has one random 32-byte **master key**; their SSH private keys are
//! sealed under it ([`seal_with_key`]). The master key is wrapped in two layers
//! (orchestrated in `masterkey.rs`):
//!   * inner — a KEK derived from **their own password** via argon2id ([`seal`]),
//!     so the hub can unwrap only while the user supplies their password; and
//!   * outer — the **application secret** ([`AppSecrets`] from `EXEC_APP_SECRET`,
//!     kept in env/KMS, never in this DB), so a DB-only leak unwraps nothing even
//!     with a guessed password. The secret is rotatable (re-wrap the outer layer
//!     only — no passwords needed); `app_kid` tags which secret wrapped a row.
//!
//! Stored blob layout: `nonce(24) || ciphertext+tag`. `kdf_salt` is stored per
//! credential (a dedicated salt — never the auth-hash salt).

use anyhow::{anyhow, Result};
use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use sha2::{Digest, Sha256};

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

/// AEAD-encrypt `plaintext` under a raw 32-byte key. Returns `nonce || ciphertext`.
/// Use for keys that are already high-entropy (the master key, the app KEK) — no
/// argon2 needed; see [`seal`] for password-derived keys.
pub fn seal_with_key(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
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

/// Decrypt a blob produced by [`seal_with_key`]. Fails on a wrong key or tampering.
pub fn open_with_key(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return Err(anyhow!("ciphertext too short"));
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    cipher
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| anyhow!("decryption failed (wrong key or tampered data)"))
}

/// A fresh random 32-byte master key for a new user.
pub fn gen_master_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

/// Encrypt `plaintext` under the password-derived KEK (argon2 over `salt`).
/// Returns `nonce || ciphertext`.
pub fn seal(password: &str, salt: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    seal_with_key(&derive_kek(password, salt)?, plaintext)
}

/// Decrypt a blob produced by [`seal`]. Fails (auth error) on the wrong password or
/// any tampering — never returns garbage.
pub fn open(password: &str, salt: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    open_with_key(&derive_kek(password, salt)?, blob)
}

// ---- application secret (the "pepper": outer wrap of the master key) ---------

/// One application secret: a 32-byte KEK plus a short public id so we can tell
/// which secret wrapped a given user's master key (enables rotation).
#[derive(Clone)]
pub struct AppKey {
    pub kid: String,
    kek: [u8; 32],
}

impl AppKey {
    /// Derive an [`AppKey`] from the operator-supplied secret string. The id and the
    /// KEK are domain-separated SHA-256 derivations, so the public `kid` never leaks
    /// the KEK. The secret should be high-entropy (e.g. 32 random bytes, base64).
    pub fn from_secret(secret: &str) -> Self {
        let kek: [u8; 32] =
            Sha256::digest([b"vantage-exec-app-kek\0".as_ref(), secret.as_bytes()].concat()).into();
        let kid_full =
            Sha256::digest([b"vantage-exec-app-kid\0".as_ref(), secret.as_bytes()].concat());
        AppKey {
            kid: hex::encode(&kid_full[..6]), // 12 hex chars — enough to distinguish secrets
            kek,
        }
    }

    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        seal_with_key(&self.kek, plaintext)
    }
    pub fn open(&self, blob: &[u8]) -> Result<Vec<u8>> {
        open_with_key(&self.kek, blob)
    }
}

/// The set of application secrets the hub knows: the `current` one (used to wrap
/// new rows) and an optional `old` one (present only during a rotation, so rows
/// still on the previous secret can be unwrapped). Loaded from env:
///   * `EXEC_APP_SECRET`     — current (omit in dev → no outer layer at all)
///   * `EXEC_APP_SECRET_OLD` — previous (set only while rotating)
#[derive(Clone, Default)]
pub struct AppSecrets {
    pub current: Option<AppKey>,
    pub old: Option<AppKey>,
}

impl AppSecrets {
    pub fn from_env() -> Self {
        let load = |var: &str| {
            std::env::var(var)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|s| AppKey::from_secret(&s))
        };
        AppSecrets {
            current: load("EXEC_APP_SECRET"),
            old: load("EXEC_APP_SECRET_OLD"),
        }
    }

    /// Whether an outer (app-secret) layer is in effect.
    pub fn enabled(&self) -> bool {
        self.current.is_some()
    }

    /// Find the secret whose id matches `kid` (current or old) to unwrap a row.
    pub fn find(&self, kid: &str) -> Option<&AppKey> {
        [self.current.as_ref(), self.old.as_ref()]
            .into_iter()
            .flatten()
            .find(|k| k.kid == kid)
    }
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
