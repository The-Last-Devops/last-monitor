//! Per-user master key lifecycle (docs/exec-design.md).
//!
//! Each user has one random master key that seals their SSH keys. It is stored
//! double-wrapped — inner by a password-derived KEK, outer by the application
//! secret — so neither a DB leak (needs the app secret) nor the app secret alone
//! (needs the password) unwraps it. The functions here are the only place that
//! assembles/strips those layers against the DB:
//!   * [`ensure`]            — provision-or-unwrap on use (create key / step-up)
//!   * [`provision_if_missing`] — cheap login hook (mint only; never unwraps)
//!   * [`rewrap_password`]   — user changes their own password (keys preserved)
//!   * [`reset`]             — admin resets a password (keys dropped, unrecoverable)
//!   * [`rotate_app_secret`] — re-wrap the outer layer to the current app secret

use crate::exec_crypto::{self, AppSecrets};
use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

type MasterKey = [u8; 32];

/// Wrap a master key for storage: inner (password KEK) then outer (app secret, if
/// enabled). Returns the blob + the app-secret id that wrapped the outer layer.
fn wrap(
    master: &MasterKey,
    password: &str,
    salt: &[u8],
    secrets: &AppSecrets,
) -> Result<(Vec<u8>, Option<String>)> {
    let inner = exec_crypto::seal(password, salt, master)?;
    match secrets.current.as_ref() {
        Some(app) => Ok((app.seal(&inner)?, Some(app.kid.clone()))),
        None => Ok((inner, None)),
    }
}

/// Reverse of [`wrap`]: strip the outer (app-secret) layer named by `app_kid`,
/// then the inner (password) layer. Wrong password or missing secret → error.
fn unwrap(
    blob: &[u8],
    app_kid: Option<&str>,
    password: &str,
    salt: &[u8],
    secrets: &AppSecrets,
) -> Result<MasterKey> {
    let inner = match app_kid {
        Some(kid) => secrets
            .find(kid)
            .ok_or_else(|| {
                anyhow!("application secret '{kid}' is not available — set EXEC_APP_SECRET (or EXEC_APP_SECRET_OLD during a rotation)")
            })?
            .open(blob)?,
        None => blob.to_vec(),
    };
    let raw = exec_crypto::open(password, salt, &inner)?;
    raw.try_into()
        .map_err(|_| anyhow!("stored master key has the wrong length"))
}

/// Load a user's stored wrap (enc, salt, app_kid). `enc`/`salt` are `None` until
/// the master key is provisioned.
async fn load(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>, Option<String>)> {
    sqlx::query_as::<_, (Option<Vec<u8>>, Option<Vec<u8>>, Option<String>)>(
        "SELECT master_key_enc, master_key_salt, app_kid FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .context("no such user")
}

async fn store(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    enc: &[u8],
    salt: &[u8],
    app_kid: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE users SET master_key_enc = $2, master_key_salt = $3, app_kid = $4 WHERE id = $1",
    )
    .bind(user_id)
    .bind(enc)
    .bind(salt)
    .bind(app_kid)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return the user's master key, provisioning (minting + storing) one if absent.
/// The `password` must already be verified by the caller — if a key exists it is
/// unwrapped with it, so a wrong password errors.
pub async fn ensure(
    pool: &sqlx::PgPool,
    secrets: &AppSecrets,
    user_id: Uuid,
    password: &str,
) -> Result<MasterKey> {
    let (enc, salt, kid) = load(pool, user_id).await?;
    if let (Some(enc), Some(salt)) = (enc, salt) {
        return unwrap(&enc, kid.as_deref(), password, &salt, secrets);
    }
    // mint
    let master = exec_crypto::gen_master_key();
    let salt = exec_crypto::gen_salt();
    let (enc, kid) = wrap(&master, password, &salt, secrets)?;
    store(pool, user_id, &enc, &salt, kid.as_deref()).await?;
    Ok(master)
}

/// Login hook: mint + store a master key if the user has none yet; otherwise do
/// nothing (never unwraps — cheap). Lets existing accounts gain a master key on
/// their next login, when the plaintext password is available.
pub async fn provision_if_missing(
    pool: &sqlx::PgPool,
    secrets: &AppSecrets,
    user_id: Uuid,
    password: &str,
) -> Result<()> {
    let (enc, _, _) = load(pool, user_id).await?;
    if enc.is_some() {
        return Ok(());
    }
    let master = exec_crypto::gen_master_key();
    let salt = exec_crypto::gen_salt();
    let (enc, kid) = wrap(&master, password, &salt, secrets)?;
    // guard a concurrent provision: only write if still unset
    sqlx::query(
        "UPDATE users SET master_key_enc = $2, master_key_salt = $3, app_kid = $4 \
         WHERE id = $1 AND master_key_enc IS NULL",
    )
    .bind(user_id)
    .bind(&enc)
    .bind(&salt)
    .bind(kid.as_deref())
    .execute(pool)
    .await?;
    Ok(())
}

/// User changes their own password: unwrap the master key with the old password,
/// re-wrap under the new one. SSH keys keep working (the master key is unchanged).
/// If the user has no master key yet, one is minted under the new password.
pub async fn rewrap_password(
    pool: &sqlx::PgPool,
    secrets: &AppSecrets,
    user_id: Uuid,
    old_password: &str,
    new_password: &str,
) -> Result<()> {
    let (enc, salt, kid) = load(pool, user_id).await?;
    let master = match (enc, salt) {
        (Some(enc), Some(salt)) => unwrap(&enc, kid.as_deref(), old_password, &salt, secrets)?,
        _ => exec_crypto::gen_master_key(),
    };
    let salt = exec_crypto::gen_salt();
    let (enc, kid) = wrap(&master, new_password, &salt, secrets)?;
    store(pool, user_id, &enc, &salt, kid.as_deref()).await
}

/// Admin reset (the old password is unknown): mint a NEW master key under the new
/// password and DROP the user's SSH keys — they were sealed under the old master
/// key, which can no longer be recovered. Returns how many keys were dropped.
pub async fn reset(
    pool: &sqlx::PgPool,
    secrets: &AppSecrets,
    user_id: Uuid,
    new_password: &str,
) -> Result<u64> {
    let master = exec_crypto::gen_master_key();
    let salt = exec_crypto::gen_salt();
    let (enc, kid) = wrap(&master, new_password, &salt, secrets)?;
    store(pool, user_id, &enc, &salt, kid.as_deref()).await?;
    let dropped = sqlx::query("DELETE FROM ssh_keys WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(dropped)
}

/// Re-wrap every provisioned master key's OUTER (app-secret) layer to the CURRENT
/// secret. No passwords are needed — only the outer layer changes. Rows already on
/// the current secret (or with no master key) are skipped. Returns #re-wrapped.
///
/// Used to enable the app secret for the first time, or to rotate it (provide the
/// previous value as `EXEC_APP_SECRET_OLD` so old rows can be unwrapped).
pub async fn rotate_app_secret(pool: &sqlx::PgPool, secrets: &AppSecrets) -> Result<usize> {
    let current = secrets
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("EXEC_APP_SECRET is not set — nothing to rotate to"))?;
    let rows = sqlx::query_as::<_, (Uuid, Vec<u8>, Option<String>)>(
        "SELECT id, master_key_enc, app_kid FROM users WHERE master_key_enc IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut n = 0usize;
    for (id, enc, kid) in rows {
        if kid.as_deref() == Some(current.kid.as_str()) {
            continue; // already on the current secret
        }
        // strip the existing outer layer (if any) down to the inner, password-wrapped blob
        let inner = match kid.as_deref() {
            Some(k) => secrets
                .find(k)
                .ok_or_else(|| {
                    anyhow!("user {id} is wrapped with secret '{k}', which is not available — set EXEC_APP_SECRET_OLD to it")
                })?
                .open(&enc)?,
            None => enc, // had no outer layer yet
        };
        let new_enc = current.seal(&inner)?;
        sqlx::query("UPDATE users SET master_key_enc = $2, app_kid = $3 WHERE id = $1")
            .bind(id)
            .bind(&new_enc)
            .bind(&current.kid)
            .execute(pool)
            .await?;
        n += 1;
    }
    Ok(n)
}
