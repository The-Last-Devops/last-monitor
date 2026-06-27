//! Service-monitor JSON endpoints feeding the SPA.
//!
//! Split by concern, each re-exported below so `web::list_monitors`,
//! `web::monitor_detail`, `web::monitor_heartbeats`, `web::monitor_events`, and
//! `web::recent_events` paths stay unchanged:
//! - [`list`] — the monitors list with each monitor's latest beat + sparkline.
//! - [`detail`] — one monitor with status, current-run duration and uptime %.
//! - [`events`] — heartbeat history + status-transition feeds (per-monitor and
//!   global).
//!
//! [`load_monitor`] (authorize + load a monitor) and [`hb_range`] (the
//! heartbeat-history range table) are shared by detail and events, so they live
//! here.

use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::web::internal;
use crate::AppState;

mod detail;
mod events;
mod list;

pub use detail::monitor_detail;
pub use events::{monitor_events, monitor_heartbeats, recent_events};
pub use list::list_monitors;

/// True if the user may view the given monitor; returns its (namespace, name,
/// kind, target, interval, enabled, config) when so.
#[allow(clippy::type_complexity)]
pub(super) async fn load_monitor(
    state: &AppState,
    user: &CurrentUser,
    id: Uuid,
) -> Result<(String, String, String, String, i32, bool, serde_json::Value), StatusCode> {
    let row: Option<(
        String,
        String,
        String,
        String,
        i32,
        bool,
        sqlx::types::Json<serde_json::Value>,
    )> = sqlx::query_as(
        "SELECT n.name, m.name, m.kind::text, m.target, m.interval_secs, m.enabled, m.config \
             FROM monitors m JOIN namespaces n ON n.id = m.namespace_id \
             WHERE m.id = $1 AND ($2 OR m.namespace_id IN ( \
                SELECT namespace_id FROM memberships WHERE user_id = $3))",
    )
    .bind(id)
    .bind(user.can_read_all())
    .bind(user.id)
    .fetch_optional(&state.config)
    .await
    .map_err(internal)?;
    let (namespace, name, kind, target, interval_secs, enabled, config) =
        row.ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        namespace,
        name,
        kind,
        target,
        interval_secs,
        enabled,
        config.0,
    ))
}

/// (interval, bucket) for the heartbeat history chart — supports up to 30 days.
pub(super) fn hb_range(range: &Option<String>) -> (&'static str, &'static str) {
    match range.as_deref() {
        Some("1h") => ("1 hour", "1 minute"),
        Some("6h") => ("6 hours", "5 minutes"),
        Some("7d") => ("7 days", "1 hour"),
        Some("30d") => ("30 days", "6 hours"),
        Some("90d") => ("90 days", "1 day"),
        Some("1y") => ("365 days", "1 day"),
        _ => ("24 hours", "15 minutes"),
    }
}
