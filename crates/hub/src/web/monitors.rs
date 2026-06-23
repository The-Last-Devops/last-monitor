use super::*;

#[derive(Serialize)]
pub struct MonitorRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub namespace: String,
    pub interval_secs: i32,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub up: Option<bool>,
    pub latency_ms: Option<i32>,
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
    pub message: Option<String>,
}

/// GET /api/monitors — each monitor (scoped to the caller's namespaces) plus
/// its latest heartbeat. Admins see every monitor.
pub async fn list_monitors(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<MonitorRow>>, StatusCode> {
    let monitors: Vec<(Uuid, String, String, String, String, i32, bool, sqlx::types::Json<serde_json::Value>)> = sqlx::query_as(
        "SELECT m.id, m.name, m.kind::text, m.target, n.name, m.interval_secs, m.enabled, m.config \
         FROM monitors m JOIN namespaces n ON n.id = m.namespace_id \
         WHERE $1 OR m.namespace_id IN ( \
            SELECT namespace_id FROM memberships WHERE user_id = $2) \
         ORDER BY m.name",
    )
    .bind(user.can_read_all())
    .bind(user.id)
    .fetch_all(&state.config)
    .await
    .map_err(internal)?;

    // Latest heartbeat for ALL monitors in ONE query (was N+1). DISTINCT ON + the
    // (monitor_id, time DESC) index makes this a fast per-monitor index scan.
    let ids: Vec<Uuid> = monitors.iter().map(|m| m.0).collect();
    #[allow(clippy::type_complexity)]
    let beat_rows: Vec<(
        Uuid,
        chrono::DateTime<chrono::Utc>,
        bool,
        Option<i32>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT DISTINCT ON (monitor_id) monitor_id, time, up, latency_ms, message \
         FROM heartbeats WHERE monitor_id = ANY($1) ORDER BY monitor_id, time DESC",
    )
    .bind(&ids)
    .fetch_all(&state.data)
    .await
    .map_err(internal)?;
    #[allow(clippy::type_complexity)]
    let mut latest: std::collections::HashMap<
        Uuid,
        (
            chrono::DateTime<chrono::Utc>,
            bool,
            Option<i32>,
            Option<String>,
        ),
    > = std::collections::HashMap::with_capacity(beat_rows.len());
    for (mid, t, up, lat, msg) in beat_rows {
        latest.insert(mid, (t, up, lat, msg));
    }

    let mut rows = Vec::with_capacity(monitors.len());
    for (id, name, kind, target, namespace, interval_secs, enabled, config) in monitors {
        let (last_check, up, latency_ms, message) = match latest.remove(&id) {
            Some((t, up, lat, msg)) => (Some(t), Some(up), lat, msg),
            None => (None, None, None, None),
        };
        rows.push(MonitorRow {
            id,
            name,
            kind,
            target,
            namespace,
            interval_secs,
            enabled,
            config: config.0,
            up,
            latency_ms,
            last_check,
            message,
        });
    }
    Ok(Json(rows))
}

#[derive(Serialize)]
pub struct MonitorDetail {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub namespace: String,
    pub interval_secs: i32,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub up: Option<bool>,
    pub latency_ms: Option<i32>,
    pub message: Option<String>,
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
    /// When the current up/down status began (start of the latest unbroken run).
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub uptime_24h: Option<f64>,
    pub uptime_7d: Option<f64>,
    pub uptime_30d: Option<f64>,
}

/// True if the user may view the given monitor; returns its (namespace, name,
/// kind, target, interval, enabled, config) when so.
#[allow(clippy::type_complexity)]
async fn load_monitor(
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

async fn uptime_pct(state: &AppState, id: Uuid, interval: &str) -> Option<f64> {
    sqlx::query_as::<_, (Option<f64>,)>(&format!(
        "SELECT avg((up)::int)::float8 * 100 FROM heartbeats \
         WHERE monitor_id = $1 AND time > now() - interval '{interval}'"
    ))
    .bind(id)
    .fetch_optional(&state.data)
    .await
    .ok()
    .flatten()
    .and_then(|(p,)| p)
}

/// GET /api/monitors/:id — one monitor with status, current-status duration and
/// uptime percentages, for the detail page.
pub async fn monitor_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<MonitorDetail>, StatusCode> {
    let (namespace, name, kind, target, interval_secs, enabled, config) =
        load_monitor(&state, &user, id).await?;

    let latest: Option<(
        chrono::DateTime<chrono::Utc>,
        bool,
        Option<i32>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT time, up, latency_ms, message FROM heartbeats \
         WHERE monitor_id = $1 ORDER BY time DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.data)
    .await
    .map_err(internal)?;

    let (last_check, up, latency_ms, message) = match &latest {
        Some((t, u, lat, msg)) => (Some(*t), Some(*u), *lat, msg.clone()),
        None => (None, None, None, None),
    };

    // Start of the current run = the first beat after the last opposite-status beat.
    let since: Option<chrono::DateTime<chrono::Utc>> = if let Some((_, cur_up, _, _)) = latest {
        let last_flip: Option<(Option<chrono::DateTime<chrono::Utc>>,)> =
            sqlx::query_as("SELECT max(time) FROM heartbeats WHERE monitor_id = $1 AND up <> $2")
                .bind(id)
                .bind(cur_up)
                .fetch_optional(&state.data)
                .await
                .map_err(internal)?;
        let flip = last_flip.and_then(|(t,)| t);
        sqlx::query_as::<_, (Option<chrono::DateTime<chrono::Utc>>,)>(
            "SELECT min(time) FROM heartbeats \
             WHERE monitor_id = $1 AND ($2::timestamptz IS NULL OR time > $2)",
        )
        .bind(id)
        .bind(flip)
        .fetch_optional(&state.data)
        .await
        .map_err(internal)?
        .and_then(|(t,)| t)
    } else {
        None
    };

    Ok(Json(MonitorDetail {
        id,
        name,
        kind,
        target,
        namespace,
        interval_secs,
        enabled,
        config,
        up,
        latency_ms,
        message,
        last_check,
        since,
        uptime_24h: uptime_pct(&state, id, "24 hours").await,
        uptime_7d: uptime_pct(&state, id, "7 days").await,
        uptime_30d: uptime_pct(&state, id, "30 days").await,
    }))
}

#[derive(Serialize)]
pub struct HeartbeatSeries {
    pub t: Vec<i64>,
    pub latency: Vec<Option<f64>>,
    /// 1 = up for the whole bucket, 0 = at least one down beat, null = no data.
    pub up: Vec<Option<f64>>,
}

/// (interval, bucket) for the heartbeat history chart — supports up to 30 days.
fn hb_range(range: &Option<String>) -> (&'static str, &'static str) {
    match range.as_deref() {
        Some("1h") => ("1 hour", "1 minute"),
        Some("6h") => ("6 hours", "5 minutes"),
        Some("7d") => ("7 days", "1 hour"),
        Some("30d") => ("30 days", "6 hours"),
        _ => ("24 hours", "15 minutes"),
    }
}

/// GET /api/monitors/:id/heartbeats?range= — bucketed latency + up/down series.
pub async fn monitor_heartbeats(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<RangeQuery>,
) -> Result<Json<HeartbeatSeries>, StatusCode> {
    load_monitor(&state, &user, id).await?; // authorize
    let (interval, bucket) = hb_range(&q.range);
    let rows: Vec<(chrono::DateTime<chrono::Utc>, Option<f64>, Option<f64>)> =
        sqlx::query_as(&format!(
            "SELECT time_bucket('{bucket}', time) AS b, \
                avg(latency_ms)::float8 AS latency, \
                min((up)::int)::float8 AS up \
         FROM heartbeats WHERE monitor_id = $1 AND time > now() - interval '{interval}' \
         GROUP BY b ORDER BY b"
        ))
        .bind(id)
        .fetch_all(&state.data)
        .await
        .map_err(internal)?;

    let mut s = HeartbeatSeries {
        t: Vec::with_capacity(rows.len()),
        latency: Vec::with_capacity(rows.len()),
        up: Vec::with_capacity(rows.len()),
    };
    for (b, latency, up) in rows {
        s.t.push(b.timestamp());
        s.latency.push(latency);
        s.up.push(up);
    }
    Ok(Json(s))
}
