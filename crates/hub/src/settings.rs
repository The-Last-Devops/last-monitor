//! Hub-wide settings as a key→value store (the `settings` table). Replaces the old
//! one-column-per-setting `app_settings` row: adding a setting is now a code change
//! (a new key + its default), not a migration. Values are JSONB; the typed helpers
//! keep each setting's type + default in the calling code.

use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;

/// Read a setting deserialized to `T`, or `default` when it's absent / unparsable.
pub async fn get<T: DeserializeOwned>(pool: &PgPool, key: &str, default: T) -> T {
    get_opt(pool, key).await.unwrap_or(default)
}

/// Read a setting if present and parsable as `T`.
pub async fn get_opt<T: DeserializeOwned>(pool: &PgPool, key: &str) -> Option<T> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.and_then(|(v,)| serde_json::from_value(v).ok())
}

/// Upsert a setting (serialized to JSONB).
pub async fn set<T: Serialize>(pool: &PgPool, key: &str, value: &T) -> Result<(), sqlx::Error> {
    let v = serde_json::to_value(value).map_err(|e| {
        sqlx::Error::Encode(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    })?;
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, now()) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(key)
    .bind(v)
    .execute(pool)
    .await?;
    Ok(())
}
