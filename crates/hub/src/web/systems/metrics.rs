use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::web::{can_view_system, chart_tier, internal, RangeQuery};
use crate::AppState;

#[derive(Serialize)]
pub struct MetricsHistory {
    pub t: Vec<i64>,
    pub cpu: Vec<f64>,
    pub mem_pct: Vec<f64>,
    /// Raw used memory in bytes (for byte-unit overlays alongside container stats).
    pub mem_used: Vec<f64>,
    pub disk_pct: Vec<f64>,
    pub net_rx: Vec<f64>,
    pub net_tx: Vec<f64>,
    /// Disk read / write throughput (bytes/sec).
    pub dr: Vec<f64>,
    pub dw: Vec<f64>,
    /// Load average 1 / 5 / 15 minutes.
    pub load1: Vec<f64>,
    pub load5: Vec<f64>,
    pub load15: Vec<f64>,
    /// CPU time breakdown (% of total): user / system / iowait / steal.
    pub cpu_user: Vec<f64>,
    pub cpu_system: Vec<f64>,
    pub cpu_iowait: Vec<f64>,
    pub cpu_steal: Vec<f64>,
    /// Disk I/O utilization (% of interval the busiest disk was busy).
    pub disk_util: Vec<f64>,
}

/// GET /api/systems/:id/metrics?range=1h|6h|24h — samples for charting (newest last).
pub async fn system_metrics_series(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<RangeQuery>,
) -> Result<Json<MetricsHistory>, StatusCode> {
    if !can_view_system(&state, &user, id).await? {
        return Err(StatusCode::FORBIDDEN);
    }
    // Read the tier that covers this range (raw for short, rollups for long) and
    // bucket to the display resolution so the point count stays bounded and the
    // chart spans the whole window. mem/disk % computed in SQL to stay within
    // sqlx's 16-element FromRow limit.
    let (suffix, timecol, interval, bucket) = chart_tier(&q.range);
    let sql = format!(
        "SELECT time_bucket('{bucket}', {timecol}) AS time, avg(cpu_percent)::float8 AS cpu, \
                CASE WHEN avg(mem_total)>0 THEN avg(mem_used)::float8/avg(mem_total)*100 ELSE 0 END AS mem_pct, \
                CASE WHEN avg(disk_total)>0 THEN avg(disk_used)::float8/avg(disk_total)*100 ELSE 0 END AS disk_pct, \
                max(net_rx) AS net_rx, max(net_tx) AS net_tx, \
                max(COALESCE(disk_read,0)) AS dread, max(COALESCE(disk_write,0)) AS dwrite, \
                avg(load1)::float8 AS l1, avg(COALESCE(load5,0))::float8 AS l5, avg(COALESCE(load15,0))::float8 AS l15, \
                avg(COALESCE(cpu_user,0))::float8 AS cu, avg(COALESCE(cpu_system,0))::float8 AS cs, \
                avg(COALESCE(cpu_iowait,0))::float8 AS cio, avg(COALESCE(cpu_steal,0))::float8 AS cst, \
                avg(mem_used)::float8 AS mu, avg(COALESCE(disk_util,0))::float8 AS disk_util \
         FROM system_metrics{suffix} WHERE system_id = $1 AND {timecol} > now() - interval '{interval}' \
         GROUP BY 1 ORDER BY 1 LIMIT 4000"
    );
    // FromRow struct (not a tuple) so we're not capped at sqlx's 16-column limit.
    #[derive(sqlx::FromRow)]
    struct Sample {
        time: chrono::DateTime<chrono::Utc>,
        cpu: f64,
        mem_pct: f64,
        disk_pct: f64,
        net_rx: i64,
        net_tx: i64,
        dread: i64,
        dwrite: i64,
        l1: f64,
        l5: f64,
        l15: f64,
        cu: f64,
        cs: f64,
        cio: f64,
        cst: f64,
        mu: f64,
        disk_util: f64,
    }
    let rows: Vec<Sample> = sqlx::query_as(&sql)
        .bind(id)
        .fetch_all(&state.data)
        .await
        .map_err(internal)?;

    let mut h = MetricsHistory {
        t: Vec::new(),
        cpu: Vec::new(),
        mem_pct: Vec::new(),
        mem_used: Vec::new(),
        disk_pct: Vec::new(),
        net_rx: Vec::new(),
        net_tx: Vec::new(),
        dr: Vec::new(),
        dw: Vec::new(),
        load1: Vec::new(),
        load5: Vec::new(),
        load15: Vec::new(),
        cpu_user: Vec::new(),
        cpu_system: Vec::new(),
        cpu_iowait: Vec::new(),
        cpu_steal: Vec::new(),
        disk_util: Vec::new(),
    };
    // Cumulative counters -> per-second rate from consecutive deltas.
    let mut prev: Option<(i64, i64, i64, i64, i64)> = None; // (ts, rx, tx, dr, dw)
    for row in rows {
        let Sample {
            time,
            cpu,
            mem_pct,
            disk_pct,
            net_rx,
            net_tx,
            dread,
            dwrite,
            l1,
            l5,
            l15,
            cu,
            cs,
            cio,
            cst,
            mu,
            disk_util,
        } = row;
        let ts = time.timestamp();
        h.t.push(ts);
        h.cpu.push(cpu);
        h.mem_pct.push(mem_pct);
        h.mem_used.push(mu);
        h.disk_pct.push(disk_pct);
        h.load1.push(l1);
        h.load5.push(l5);
        h.load15.push(l15);
        h.cpu_user.push(cu);
        h.cpu_system.push(cs);
        h.cpu_iowait.push(cio);
        h.cpu_steal.push(cst);
        h.disk_util.push(disk_util);
        let (rx_rate, tx_rate, dr_rate, dw_rate) = match prev {
            Some((pt, prx, ptx, pdr, pdw)) if ts > pt => {
                let dt = (ts - pt) as f64;
                (
                    (net_rx - prx).max(0) as f64 / dt,
                    (net_tx - ptx).max(0) as f64 / dt,
                    (dread - pdr).max(0) as f64 / dt,
                    (dwrite - pdw).max(0) as f64 / dt,
                )
            }
            _ => (0.0, 0.0, 0.0, 0.0),
        };
        h.net_rx.push(rx_rate);
        h.net_tx.push(tx_rate);
        h.dr.push(dr_rate);
        h.dw.push(dw_rate);
        prev = Some((ts, net_rx, net_tx, dread, dwrite));
    }
    Ok(Json(h))
}
