//! Server-rendered web UI + JSON endpoints feeding it.
//!
//! Minimal skeleton: a dashboard page (HTML + HTMX + uPlot via CDN) and a JSON
//! endpoint listing servers with their latest metric. Real templating (Askama/Maud),
//! auth, and namespaces come next.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::AppState;

mod monitors;
mod systems;

pub use monitors::*;
pub use systems::*;

/// True if the user may view the given server (admin / read-only admin, or a
/// member of its namespace).
pub async fn can_view_system(
    state: &AppState,
    user: &CurrentUser,
    system_id: Uuid,
) -> Result<bool, StatusCode> {
    if user.can_read_all() {
        return Ok(true);
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM systems s \
         JOIN memberships m ON m.namespace_id = s.namespace_id \
         WHERE s.id = $1 AND m.user_id = $2",
    )
    .bind(system_id)
    .bind(user.id)
    .fetch_optional(&state.config)
    .await
    .map_err(internal)?;
    Ok(row.is_some())
}
#[derive(serde::Deserialize)]
pub struct RangeQuery {
    #[serde(default)]
    pub range: Option<String>,
}
/// Maps a UI range key to a Postgres interval. Bounded by raw retention (1 day).
fn range_interval(range: &Option<String>) -> &'static str {
    match range.as_deref() {
        Some("30m") => "30 minutes",
        Some("3h") => "3 hours",
        Some("6h") => "6 hours",
        Some("12h") => "12 hours",
        Some("24h") => "24 hours",
        _ => "1 hour",
    }
}
fn range_bucket(range: &Option<String>) -> &'static str {
    match range.as_deref() {
        Some("6h") => "5 minutes",
        Some("12h") => "10 minutes",
        Some("24h") => "15 minutes",
        _ => "1 minute", // 30m / 1h / 3h
    }
}
fn internal<E: std::fmt::Display>(e: E) -> StatusCode {
    tracing::error!(error = %e, "web handler DB error");
    StatusCode::INTERNAL_SERVER_ERROR
}
