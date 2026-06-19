//! Agent metrics ingest endpoint.
//!
//! Flow: authenticate the agent by its enrollment token (config DB) -> resolve the
//! owning server -> write the sample into the data DB hypertable.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use shared::{IngestAck, MetricsReport, AGENT_TOKEN_HEADER};
use uuid::Uuid;

use crate::AppState;

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(report): Json<MetricsReport>,
) -> Result<Json<IngestAck>, StatusCode> {
    let token = headers
        .get(AGENT_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Resolve the (reusable) enrollment token -> its namespace.
    let tok: (Uuid, Uuid) =
        sqlx::query_as("SELECT id, namespace_id FROM enrollment_tokens WHERE token = $1")
            .bind(token)
            .fetch_optional(&state.config)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "config DB error during ingest");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::UNAUTHORIZED)?;
    let (token_id, namespace_id) = tok;

    let hostname = if report.hostname.is_empty() {
        "unknown".to_string()
    } else {
        report.hostname.clone()
    };

    // Auto-register / update the server identified by (token, hostname).
    let server: (Uuid,) = sqlx::query_as(
        "INSERT INTO servers (namespace_id, token_id, name, hostname, kernel, cpu_model, cpu_cores, agent_version, last_seen) \
         VALUES ($1, $2, $3, $3, $4, $5, $6, $7, now()) \
         ON CONFLICT (token_id, hostname) DO UPDATE SET \
            last_seen = now(), kernel = $4, cpu_model = $5, cpu_cores = $6, agent_version = $7 \
         RETURNING id",
    )
    .bind(namespace_id)
    .bind(token_id)
    .bind(&hostname)
    .bind(&report.kernel)
    .bind(&report.cpu_model)
    .bind(report.cpu_cores as i32)
    .bind(&report.agent_version)
    .fetch_one(&state.config)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "server upsert during ingest");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let server_id = server.0;
    let ts = chrono::DateTime::from_timestamp(report.ts, 0).unwrap_or_else(chrono::Utc::now);

    // Write the sample into the data DB. server_id is the cross-DB link (no JOINs).
    sqlx::query(
        r#"
        INSERT INTO metrics (
            time, server_id, cpu_percent, mem_used, mem_total,
            swap_used, swap_total, disk_used, disk_total,
            net_rx, net_tx, load1, uptime, temps,
            disk_read, disk_write, gpus
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
        "#,
    )
    .bind(ts)
    .bind(server_id)
    .bind(report.cpu_percent as f64)
    .bind(report.mem_used as i64)
    .bind(report.mem_total as i64)
    .bind(report.swap_used as i64)
    .bind(report.swap_total as i64)
    .bind(report.disk_used as i64)
    .bind(report.disk_total as i64)
    .bind(report.net_rx as i64)
    .bind(report.net_tx as i64)
    .bind(report.load1)
    .bind(report.uptime as i64)
    .bind(sqlx::types::Json(&report.temps))
    .bind(report.disk_read as i64)
    .bind(report.disk_write as i64)
    .bind(sqlx::types::Json(&report.gpus))
    .execute(&state.data)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "data DB error during ingest");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Per-container stats (best-effort).
    for c in &report.containers {
        let _ = sqlx::query(
            "INSERT INTO container_stats (time, server_id, name, cpu_percent, mem_used, net_rx, net_tx) \
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(ts)
        .bind(server_id)
        .bind(&c.name)
        .bind(c.cpu_percent as f64)
        .bind(c.mem_used as i64)
        .bind(c.net_rx as i64)
        .bind(c.net_tx as i64)
        .execute(&state.data)
        .await;
    }

    Ok(Json(IngestAck {
        ok: true,
        next_interval_secs: 0, // 0 => agent keeps its current interval
    }))
}
