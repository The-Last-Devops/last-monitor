//! Background scheduler + one-shot probing + heartbeat/debug persistence.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

use super::checks::probe;
use super::{cfg_bool, cfg_u64, Beat, Monitor, TICK};
use crate::AppState;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut last_run: HashMap<Uuid, Instant> = HashMap::new();
        let streaks: Arc<Mutex<HashMap<Uuid, u64>>> = Arc::new(Mutex::new(HashMap::new()));

        loop {
            match load_monitors(&state).await {
                Ok(monitors) => {
                    let now = Instant::now();
                    let live: std::collections::HashSet<Uuid> =
                        monitors.iter().map(|m| m.id).collect();
                    last_run.retain(|id, _| live.contains(id));
                    streaks.lock().unwrap().retain(|id, _| live.contains(id));

                    for m in monitors {
                        let due = match last_run.get(&m.id) {
                            Some(t) => now.duration_since(*t) >= m.interval,
                            None => true,
                        };
                        if due {
                            last_run.insert(m.id, now);
                            let data = state.data.clone();
                            let config = state.config.clone();
                            let streaks = streaks.clone();
                            tokio::spawn(async move {
                                // Push monitors aren't probed; we just check staleness — the
                                // "up" beats arrive via /pub/push/<token>.
                                if m.kind == "push" {
                                    let last: Option<(chrono::DateTime<chrono::Utc>,)> = sqlx::query_as(
                                        "SELECT time FROM heartbeats WHERE monitor_id = $1 ORDER BY time DESC LIMIT 1",
                                    )
                                    .bind(m.id)
                                    .fetch_optional(&data)
                                    .await
                                    .ok()
                                    .flatten();
                                    let stale = match last {
                                        Some((t,)) => {
                                            (chrono::Utc::now() - t).num_seconds().max(0) as u64
                                                > m.interval.as_secs()
                                        }
                                        None => true,
                                    };
                                    if stale {
                                        let beat = Beat {
                                            up: false,
                                            latency_ms: None,
                                            status_code: None,
                                            message: Some(
                                                "no push received within interval".into(),
                                            ),
                                            debug: None,
                                        };
                                        let _ = write_beat(&data, m.id, &beat).await;
                                    }
                                    return;
                                }
                                let mut beat = probe(&m).await;
                                // The raw check result (before upside-down / retries) is what
                                // we classify the debug record by.
                                let raw_up = beat.up;
                                let debug = beat.debug.take();

                                if cfg_bool(&m.config, "upside_down") {
                                    beat.up = !beat.up;
                                    if !beat.up {
                                        beat.message = Some("up (inverted by upside-down)".into());
                                    }
                                }
                                let retries = cfg_u64(&m.config, "retries", 1);
                                let streak = {
                                    let mut g = streaks.lock().unwrap();
                                    let s = if beat.up {
                                        0
                                    } else {
                                        g.get(&m.id).copied().unwrap_or(0) + 1
                                    };
                                    g.insert(m.id, s);
                                    s
                                };
                                if !beat.up && streak <= retries {
                                    beat.up = true;
                                    beat.message = Some(format!(
                                        "{} (retry {}/{})",
                                        beat.message.as_deref().unwrap_or("check failed"),
                                        streak,
                                        retries
                                    ));
                                }
                                if let Err(e) = write_beat(&data, m.id, &beat).await {
                                    tracing::error!(error = %e, monitor = %m.id, "write heartbeat");
                                }
                                if let Some(detail) = debug {
                                    let outcome = if raw_up { "ok" } else { "err" };
                                    let _ = write_debug(&config, m.id, outcome, &detail).await;
                                }
                            });
                        }
                    }
                }
                Err(e) => tracing::error!(error = %e, "load monitors"),
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// Probe a monitor once, immediately — called right after a monitor is created so
/// its status (and any alert on it) doesn't wait for the next scheduler cycle.
/// Records the raw result (no retry grace) so a service that's already down shows
/// down at once. Push monitors have nothing to probe and are skipped.
pub async fn check_once(state: &AppState, monitor_id: Uuid) {
    let row: Option<(String, String, i32, Json<Value>)> = sqlx::query_as(
        "SELECT kind::text, target, interval_secs, config FROM monitors \
         WHERE id = $1 AND enabled = true",
    )
    .bind(monitor_id)
    .fetch_optional(&state.config)
    .await
    .unwrap_or(None);
    let Some((kind, target, interval_secs, config)) = row else {
        return;
    };
    if kind == "push" {
        return;
    }
    let m = Monitor {
        id: monitor_id,
        kind,
        target,
        interval: Duration::from_secs(interval_secs.max(1) as u64),
        config: config.0,
    };
    let mut beat = probe(&m).await;
    let raw_up = beat.up;
    let debug = beat.debug.take();
    if cfg_bool(&m.config, "upside_down") {
        beat.up = !beat.up;
        if !beat.up {
            beat.message = Some("up (inverted by upside-down)".into());
        }
    }
    if let Err(e) = write_beat(&state.data, m.id, &beat).await {
        tracing::error!(error = %e, monitor = %m.id, "check_once write heartbeat");
    }
    if let Some(detail) = debug {
        let _ = write_debug(
            &state.config,
            m.id,
            if raw_up { "ok" } else { "err" },
            &detail,
        )
        .await;
    }
}

async fn load_monitors(state: &AppState) -> anyhow::Result<Vec<Monitor>> {
    let rows: Vec<(Uuid, String, String, i32, Json<Value>)> = sqlx::query_as(
        "SELECT id, kind::text, target, interval_secs, config \
         FROM monitors WHERE enabled = true",
    )
    .fetch_all(&state.config)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, target, interval_secs, config)| Monitor {
            id,
            kind,
            target,
            interval: Duration::from_secs(interval_secs.max(1) as u64),
            config: config.0,
        })
        .collect())
}

async fn write_beat(data: &sqlx::PgPool, monitor_id: Uuid, beat: &Beat) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO heartbeats (time, monitor_id, up, latency_ms, status_code, message) \
         VALUES (now(), $1, $2, $3, $4, $5)",
    )
    .bind(monitor_id)
    .bind(beat.up)
    .bind(beat.latency_ms)
    .bind(beat.status_code)
    .bind(beat.message.as_deref())
    .execute(data)
    .await?;
    Ok(())
}

/// Upsert the last 'ok' / 'err' request-response detail for a monitor.
async fn write_debug(
    config: &sqlx::PgPool,
    monitor_id: Uuid,
    outcome: &str,
    detail: &Value,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO monitor_debug (monitor_id, outcome, detail, at) VALUES ($1, $2, $3, now()) \
         ON CONFLICT (monitor_id, outcome) DO UPDATE SET detail = EXCLUDED.detail, at = now()",
    )
    .bind(monitor_id)
    .bind(outcome)
    .bind(sqlx::types::Json(detail))
    .execute(config)
    .await?;
    Ok(())
}
