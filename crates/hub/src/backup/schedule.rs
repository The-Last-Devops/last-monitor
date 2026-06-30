//! Scheduled (cron) backups: schedule settings handlers plus the background
//! loop that pushes a snapshot to S3 when due and prunes old objects.

use axum::{extract::State, http::StatusCode, Json};
use chrono::{NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::CurrentUser;
use crate::AppState;

use super::s3::{load_s3, s3_key, s3_request, uri_encode, S3Config};
use super::{admin, build_snapshot, gzip};

fn default_mode() -> String {
    "daily".into()
}
fn default_keep() -> i64 {
    14
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BackupSchedule {
    #[serde(default)]
    pub enabled: bool,
    /// "interval" (every N hours) or "daily" (at HH:MM UTC).
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub interval_hours: i64,
    #[serde(default)]
    pub daily_time: String, // "HH:MM" UTC
    #[serde(default)]
    pub include_metrics: bool,
    #[serde(default = "default_keep")]
    pub keep: i64,
}

async fn load_schedule(
    state: &AppState,
) -> Result<(Option<BackupSchedule>, Option<chrono::DateTime<Utc>>), String> {
    let sched = crate::settings::get_opt::<BackupSchedule>(&state.config, "backup").await;
    let last =
        crate::settings::get_opt::<chrono::DateTime<Utc>>(&state.config, "last_backup_at").await;
    Ok((sched, last))
}

/// GET /api/admin/backup/schedule — current schedule + last run.
pub async fn schedule_get(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Value>, StatusCode> {
    admin(&user)?;
    let (sched, last) = load_schedule(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "schedule": sched.map(|s| serde_json::to_value(s).unwrap_or(Value::Null)),
        "last_backup_at": last.map(|t| t.to_rfc3339()),
    })))
}

/// PUT /api/admin/backup/schedule — save the schedule (admin).
pub async fn schedule_put(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(sched): Json<BackupSchedule>,
) -> Result<StatusCode, (StatusCode, String)> {
    admin(&user).map_err(|s| (s, "admin only".into()))?;
    crate::settings::set(&state.config, "backup", &sched)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Background loop: once a minute, run a backup to S3 if the schedule is due.
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = tick_schedule(&state).await {
                tracing::debug!(error = %e, "scheduled backup tick (ignored)");
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

async fn tick_schedule(state: &AppState) -> Result<(), String> {
    let (sched, last) = load_schedule(state).await?;
    let Some(s) = sched else { return Ok(()) };
    if !s.enabled {
        return Ok(());
    }
    // S3 must be configured to store scheduled backups.
    let cfg = match load_s3(state).await {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let now = Utc::now();
    let due = if s.mode == "interval" {
        let h = s.interval_hours.max(1);
        last.map(|l| (now - l).num_hours() >= h).unwrap_or(true)
    } else {
        let t = NaiveTime::parse_from_str(&s.daily_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(3, 0, 0).unwrap());
        let target = now.date_naive().and_time(t).and_utc();
        now >= target && last.map(|l| l < target).unwrap_or(true)
    };
    if !due {
        return Ok(());
    }

    let snap = build_snapshot(state, s.include_metrics).await?;
    let body = gzip(&serde_json::to_vec(&snap).map_err(|e| e.to_string())?);
    let name = format!("vantage-backup-{}.json.gz", now.format("%Y%m%d-%H%M%S"));
    let key = s3_key(&cfg, &name);
    s3_request(&cfg, "PUT", &key, "", body).await?;
    crate::settings::set(&state.config, "last_backup_at", &Utc::now())
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(key = %key, "scheduled backup uploaded");
    prune(&cfg, s.keep.max(1) as usize).await;
    Ok(())
}

/// Keep only the newest `keep` backup objects in the bucket.
async fn prune(cfg: &S3Config, keep: usize) {
    let prefix = s3_key(cfg, "");
    let query = format!("list-type=2&prefix={}", uri_encode(&prefix, true));
    let Ok(resp) = s3_request(cfg, "GET", "", &query, Vec::new()).await else {
        return;
    };
    let xml = resp.text().await.unwrap_or_default();
    let mut keys: Vec<String> = xml
        .split("<Key>")
        .skip(1)
        .filter_map(|s| s.split("</Key>").next())
        .map(|s| s.to_string())
        .collect();
    keys.sort_unstable();
    keys.reverse(); // newest (timestamped) first
    for k in keys.into_iter().skip(keep) {
        let _ = s3_request(cfg, "DELETE", &k, "", Vec::new()).await;
    }
}
