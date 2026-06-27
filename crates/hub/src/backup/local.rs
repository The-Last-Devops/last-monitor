//! Local backup: download a gzipped snapshot, or restore from an uploaded one.

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use serde_json::Value;

use crate::auth::CurrentUser;
use crate::AppState;

use super::{admin, build_snapshot, gzip, maybe_gunzip, restore_snapshot, BackupQuery};

/// GET /api/admin/backup?metrics= — download a gzipped snapshot.
pub async fn download(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<BackupQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    admin(&user)?;
    let snap = build_snapshot(&state, q.metrics).await.map_err(|e| {
        tracing::error!(error = %e, "backup build");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let body = gzip(&serde_json::to_vec(&snap).unwrap_or_default());
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let name = format!("vantage-backup-{stamp}.json.gz");
    Ok((
        [
            (header::CONTENT_TYPE, "application/gzip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        body,
    ))
}

/// POST /api/admin/restore — restore from an uploaded snapshot (gz or plain JSON).
pub async fn restore(
    State(state): State<AppState>,
    user: CurrentUser,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    admin(&user).map_err(|s| (s, "admin only".into()))?;
    let raw = maybe_gunzip(&body);
    let snap: Value = serde_json::from_slice(&raw)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid backup: {e}")))?;
    restore_snapshot(&state, &snap)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(StatusCode::NO_CONTENT)
}
