//! Service-check probe engine (the Uptime-Kuma half).
//!
//! A single background scheduler reloads enabled monitors from the config DB,
//! fires each one on its own interval, and writes a heartbeat row into the data DB.
//! Probes run as detached tasks so one slow target never stalls the scheduler.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::Value;
use sqlx::types::Json;
use tokio::time::timeout;
use uuid::Uuid;

use crate::AppState;

/// A monitor definition as needed by the prober (subset of the config row).
#[derive(Clone)]
struct Monitor {
    id: Uuid,
    kind: String,
    target: String,
    interval: Duration,
    config: Value,
}

/// Outcome of a single probe.
struct Beat {
    up: bool,
    latency_ms: Option<i32>,
    status_code: Option<i32>,
    message: Option<String>,
}

/// How often the scheduler reloads the monitor list and checks what is due.
const TICK: Duration = Duration::from_secs(2);

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("build probe client");
        // monitor_id -> last time we fired it.
        let mut last_run: HashMap<Uuid, Instant> = HashMap::new();

        loop {
            match load_monitors(&state).await {
                Ok(monitors) => {
                    let now = Instant::now();
                    // Drop bookkeeping for monitors that no longer exist.
                    let live: std::collections::HashSet<Uuid> =
                        monitors.iter().map(|m| m.id).collect();
                    last_run.retain(|id, _| live.contains(id));

                    for m in monitors {
                        let due = match last_run.get(&m.id) {
                            Some(t) => now.duration_since(*t) >= m.interval,
                            None => true, // never run -> run now
                        };
                        if due {
                            last_run.insert(m.id, now);
                            let client = client.clone();
                            let data = state.data.clone();
                            tokio::spawn(async move {
                                let beat = probe(&client, &m).await;
                                if let Err(e) = write_beat(&data, m.id, &beat).await {
                                    tracing::error!(error = %e, monitor = %m.id, "write heartbeat");
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

async fn probe(client: &reqwest::Client, m: &Monitor) -> Beat {
    let start = Instant::now();
    match m.kind.as_str() {
        "http" | "keyword" => probe_http(client, m, start).await,
        "tcp" => probe_tcp(m, start).await,
        "ping" => probe_ping(m).await,
        other => Beat {
            up: false,
            latency_ms: None,
            status_code: None,
            message: Some(format!("unsupported monitor kind: {other}")),
        },
    }
}

/// ICMP ping. Requires the hub to hold CAP_NET_RAW (or run as root); on Linux,
/// granting `cap_net_raw` to the binary / container is enough. Errors (including
/// permission) surface as a down heartbeat with a message.
async fn probe_ping(m: &Monitor) -> Beat {
    // target is a bare host/IP. Resolve to an address.
    let addr = match tokio::net::lookup_host((m.target.as_str(), 0)).await {
        Ok(mut it) => match it.next() {
            Some(sa) => sa.ip(),
            None => return down("no DNS result"),
        },
        Err(e) => return down(&e.to_string()),
    };

    let client = match surge_ping::Client::new(&surge_ping::Config::default()) {
        Ok(c) => c,
        Err(e) => return down(&format!("icmp socket: {e}")),
    };
    let id = surge_ping::PingIdentifier(rand::random());
    let mut pinger = client.pinger(addr, id).await;
    pinger.timeout(Duration::from_secs(5));
    match pinger.ping(surge_ping::PingSequence(0), &[0u8; 32]).await {
        Ok((_, rtt)) => Beat {
            up: true,
            latency_ms: Some(rtt.as_millis() as i32),
            status_code: None,
            message: None,
        },
        Err(e) => down(&e.to_string()),
    }
}

fn down(msg: &str) -> Beat {
    Beat {
        up: false,
        latency_ms: None,
        status_code: None,
        message: Some(truncate(msg)),
    }
}

async fn probe_http(client: &reqwest::Client, m: &Monitor, start: Instant) -> Beat {
    let expected = m.config.get("expected_status").and_then(|v| v.as_i64());
    let keyword = m
        .config
        .get("keyword")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    match client.get(&m.target).send().await {
        Ok(resp) => {
            let status = resp.status();
            let code = status.as_u16() as i32;
            // For keyword monitors we must read the body.
            let (mut up, body) = if keyword.is_some() {
                match resp.text().await {
                    Ok(b) => (true, Some(b)),
                    Err(_) => (false, None),
                }
            } else {
                (true, None)
            };

            let mut message = None;
            up &= match expected {
                Some(want) => code == want as i32,
                None => status.is_success(),
            };
            if !up && message.is_none() {
                message = Some(format!("unexpected status {code}"));
            }
            if let Some(kw) = &keyword {
                let found = body.as_deref().map(|b| b.contains(kw)).unwrap_or(false);
                if !found {
                    up = false;
                    message = Some(format!("keyword '{kw}' not found"));
                }
            }

            Beat {
                up,
                latency_ms: Some(start.elapsed().as_millis() as i32),
                status_code: Some(code),
                message,
            }
        }
        Err(e) => Beat {
            up: false,
            latency_ms: Some(start.elapsed().as_millis() as i32),
            status_code: None,
            message: Some(truncate(&e.to_string())),
        },
    }
}

async fn probe_tcp(m: &Monitor, start: Instant) -> Beat {
    // target is "host:port".
    let connect = tokio::net::TcpStream::connect(&m.target);
    match timeout(Duration::from_secs(10), connect).await {
        Ok(Ok(_stream)) => Beat {
            up: true,
            latency_ms: Some(start.elapsed().as_millis() as i32),
            status_code: None,
            message: None,
        },
        Ok(Err(e)) => Beat {
            up: false,
            latency_ms: Some(start.elapsed().as_millis() as i32),
            status_code: None,
            message: Some(truncate(&e.to_string())),
        },
        Err(_) => Beat {
            up: false,
            latency_ms: None,
            status_code: None,
            message: Some("connect timeout".into()),
        },
    }
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

fn truncate(s: &str) -> String {
    s.chars().take(200).collect()
}
