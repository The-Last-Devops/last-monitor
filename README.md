<div align="center">

# Last Monitor

**Lightweight, self-hosted server & service monitoring — written in Rust.**

Beszel-style host metrics (an agent on every server) combined with Uptime-Kuma-style
service checks and alerting, plus multi-user namespaces, RBAC, and public status pages —
all served from a single Rust binary.

</div>

---

## Why

- **One small binary.** The hub serves the API, the realtime web UI, the alert engine and
  the service prober. No Node, no SPA build step, no `node_modules`.
- **Push-based agents.** Agents reach out to the hub, so they work behind NAT/firewalls.
  One reusable enrollment token can register a whole fleet (e.g. a Kubernetes DaemonSet) —
  servers auto-register by hostname.
- **Time-series done right.** Metrics live in PostgreSQL + TimescaleDB with automatic
  downsampling (1-minute / 1-hour rollups) and per-tier retention.

## Features

**Host metrics** (agent) — CPU, memory, swap, disk usage, **disk I/O**, network throughput,
load average, uptime, temperature sensors, NVIDIA **GPU** (usage / VRAM / power), and
**per-container Docker stats** (CPU / memory / network).

**Service monitors** — HTTP, TCP, keyword and ICMP **ping** checks on a schedule, with
uptime % and heartbeat history.

**Alerting** — rules on monitor-down, server-offline or metric thresholds (CPU / memory /
load), with cooldown and recovery notifications. Channels: webhook, Telegram (email is a
stubbed extension point).

**Multi-tenant** — namespaces (k8s-style names), RBAC (`owner` / `editor` / `viewer`) plus a
system `admin`, opaque revocable sessions, admin-provisioned users.

**Web UI** — realtime dashboard with sortable/filterable columns, per-server charts (uPlot),
public status pages, command palette (⌘K), light/dark theme, and a data-management page
(DB size, per-table storage, retention controls).

## Architecture

```
                 push (x-agent-token)
  ┌─────────┐  ───────────────────────►  ┌──────────────────────────────┐
  │ agent   │     POST /api/ingest        │  hub (Axum, single binary)   │
  │ (Rust)  │                             │  ingest · probes · alerts    │
  └─────────┘                             │  auth/RBAC · JSON API · SSR  │
   one per host                           └───────────────┬──────────────┘
                                          ┌───────────────┴──────────────┐
                                          │ config DB (Postgres)         │  users, namespaces,
                                          │ data   DB (Postgres+Timescale)│  RBAC, tokens, monitors
                                          └──────────────────────────────┘  metrics, heartbeats
```

Two **separate** PostgreSQL databases (config vs time-series), related only by IDs at the
application layer — never JOINed — so the time-series store can be scaled or relocated
independently. See [CLAUDE.md](CLAUDE.md) for the full design.

## Quick start (Docker Compose)

```bash
git clone <repo> && cd last-monitor
./deploy/build-css.sh                 # build the embedded Tailwind CSS (first run downloads the CLI)
docker compose up -d --build          # Postgres/TimescaleDB + hub (:8080) + Adminer (:8088) + a bundled agent
```

Open **http://localhost:8080** and sign in with `ADMIN_EMAIL` / `ADMIN_PASSWORD`
(defaults `admin@local` / `admin123` — change them). A bundled agent monitors the Docker
host out of the box.

## Adding servers

In the UI: **Add server** → create a reusable enrollment token → copy the install snippet
for your target (**binary**, **Docker**, **Docker Compose**, or **Kubernetes DaemonSet**).
Run the agent with that token anywhere; servers appear automatically.

```bash
# Docker (reports host metrics via shared namespaces + mounts)
docker run -d --restart=unless-stopped --pid=host \
  -e HUB_URL=https://hub.example.com -e AGENT_TOKEN=<token> -e DISK_PATH=/host \
  -v /:/host:ro -v /var/run/docker.sock:/var/run/docker.sock:ro \
  ghcr.io/<owner>/last-monitor-agent:latest
```

## Development

```bash
cargo build                  # whole workspace (hub + agent + shared)
cargo test                   # unit tests
cargo clippy --all-targets   # lint
cargo fmt                    # format
./deploy/build-css.sh        # regenerate embedded CSS after UI class changes
python3 deploy/shot.py <url> out.png <email> <password>   # screenshot a page (headless Chrome)
```

Stack: **Rust + Axum** (hub), **sysinfo + bollard** (agent), **sqlx** (runtime queries),
**PostgreSQL + TimescaleDB**, **Maud + HTMX + uPlot + Tailwind** (SSR UI, no JS build).

## License

MIT
