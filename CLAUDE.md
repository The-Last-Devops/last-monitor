# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`last-monitor` is a self-hosted server & service monitoring system written in Rust,
combining Beszel-style host metrics (agent on each server) with Uptime-Kuma-style
service checks + alerting. Multi-user with namespace-scoped RBAC and public status pages.

## Architecture (the big picture — read this before designing changes)

Cargo workspace with three crates plus a hub-served SSR frontend:

- `crates/shared` — types exchanged between agent and hub (e.g. `MetricsReport`,
  `IngestAck`, the `API_KEY_HEADER` constant). Both sides depend on this; keep the
  wire format here so they can't drift.
- `crates/agent` — `last-agent` binary. Runs on each monitored server, collects host
  metrics via `sysinfo`, and **pushes** them to the hub. Configured purely by env vars
  (`HUB_URL`, `API_KEY`; `INTERVAL` is an optional override). The push cadence is
  controlled by the hub, which returns the next interval in `IngestAck`.
- `crates/hub` — the central Axum server: ingest endpoint, service probes, alert engine,
  auth/RBAC, JSON API, **and** the server-rendered web UI.

### Decisions that are load-bearing (don't silently reverse them)

- **Push model, not pull.** Agents reach out to the hub (`POST /pub/ingest`), authenticating
  with a per-server enrollment token in the `x-agent-token` header. This is what lets agents
  sit behind NAT/firewalls. The hub never connects back to agents.
- **Two separate PostgreSQL databases, two `PgPool`s in the hub:**
  - `config` DB — plain Postgres: users, namespaces, RBAC/membership, server & monitor
    config, alert rules, status pages.
  - `data` DB — Postgres **+ TimescaleDB extension**: metrics & heartbeat hypertables.
  - **Never JOIN across the two.** They relate only by IDs at the application layer
    (e.g. `server_id` stored in both). They may share one Postgres instance early on and
    be split onto separate hosts later by changing only connection strings — keep code
    agnostic to that.
  - Use TimescaleDB **continuous aggregates + retention policies** for downsampling instead
    of hand-rolling it.
- **Frontend is Rust SSR + HTMX, not a JS SPA.** The hub renders HTML (templating) and uses
  HTMX for interactivity; realtime updates go over SSE/WebSocket; charts use **uPlot** loaded
  via `<script>`. Styling is **Tailwind via the standalone CLI binary** (no Node/`npm`,
  no `node_modules`) — it compiles to one CSS file that gets embedded/served by the hub.
  Chosen for minimal weight and a single-binary, single-codebase footprint. Do not introduce
  a React/Node build step. (daisyUI is an optional Tailwind plugin, not a hard dependency.)
- **RBAC is namespace-scoped.** Permissions live in a `memberships` table (user × namespace ×
  role: `owner` / `editor` / `viewer`), plus a system-level `admin` (bypasses to owner
  everywhere). Authorize at the namespace boundary — every namespaced route funnels through
  `rbac::require_role`, the single place permission rules live.
- **Auth = opaque DB-backed sessions, not JWT** (revocable: logout/admin-revoke work without a
  blocklist). Random token in the `sessions` table, delivered as an httpOnly cookie; passwords
  hashed with argon2. The `CurrentUser` axum extractor resolves the cookie on each request.
  **No open registration** — the first admin is bootstrapped from `ADMIN_EMAIL`/`ADMIN_PASSWORD`
  env on startup, and admins provision further users via `POST /api/users`. Auth lives entirely
  in `auth.rs` so adding OAuth/OIDC/LDAP later only means minting a session there.
- **Two auth paths, don't conflate them:** agents authenticate per-request with the
  `x-agent-token` header (no session); humans use the session cookie.
- **sqlx with runtime queries** (`sqlx::query` / `query_as`), not the compile-time `query!`
  macros, so the workspace builds without a live database / `DATABASE_URL` at compile time.

## Commands

```bash
# Build everything
cargo build

# Run the hub (needs CONFIG_DATABASE_URL + DATA_DATABASE_URL env vars)
cargo run -p hub

# Run an agent against a hub
HUB_URL=http://localhost:8080 API_KEY=<api-key> cargo run -p agent

# Tests
cargo test                      # whole workspace
cargo test -p hub               # one crate
cargo test -p hub <name>        # a single test by name

# Lint / format
cargo clippy --all-targets
cargo fmt

# Local stack (hub + Postgres/TimescaleDB)
docker compose up -d
```

## Conventions

- Agent ↔ hub wire types belong in `crates/shared` — change them there, never duplicate.
- Metrics/time-series writes target the `data` pool; everything else targets the `config` pool.
- Migrations are split per database (`migrations/config/`, `migrations/data/`); only the
  `data` DB runs `CREATE EXTENSION timescaledb`.
- **Checks/smoke-tests go in a committed `scripts/*.sh`, then run with `bash scripts/<name>.sh` —
  never ad-hoc one-liners (no inline `curl | python`, piped greps, etc.).** Write the check into a
  script, run it yourself, and don't ask for permission to run it. New checks should be idempotent
  and self-cleaning (e.g. `scripts/check-alerts.sh`).
