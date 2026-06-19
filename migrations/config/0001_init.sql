-- Config DB: users, namespaces, RBAC, server & monitor config.
-- Plain PostgreSQL (no TimescaleDB here).

CREATE EXTENSION IF NOT EXISTS pgcrypto; -- gen_random_uuid()

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,            -- argon2
    is_admin      BOOLEAN NOT NULL DEFAULT false,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE namespaces (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- RBAC: a user's role within a namespace.
CREATE TYPE ns_role AS ENUM ('owner', 'editor', 'viewer');

CREATE TABLE memberships (
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    namespace_id UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    role         ns_role NOT NULL,
    PRIMARY KEY (user_id, namespace_id)
);

-- A monitored host running an agent.
CREATE TABLE servers (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    hostname     TEXT,
    agent_token  TEXT NOT NULL UNIQUE,      -- enrollment token presented by the agent
    enabled      BOOLEAN NOT NULL DEFAULT true,
    last_seen    TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A service check (Uptime-Kuma style).
CREATE TYPE monitor_kind AS ENUM ('http', 'tcp', 'ping', 'keyword');

CREATE TABLE monitors (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id  UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    kind          monitor_kind NOT NULL,
    target        TEXT NOT NULL,            -- url / host:port / host
    interval_secs INTEGER NOT NULL DEFAULT 60,
    config        JSONB NOT NULL DEFAULT '{}'::jsonb, -- keyword, headers, expected status, etc.
    enabled       BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE notification_channels (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL,             -- telegram | webhook | email
    config       JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE alert_rules (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    monitor_id  UUID REFERENCES monitors(id) ON DELETE CASCADE,
    server_id   UUID REFERENCES servers(id) ON DELETE CASCADE,
    channel_id  UUID NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    condition   JSONB NOT NULL DEFAULT '{}'::jsonb, -- e.g. {"cpu_percent": {">": 90}} or {"down_for_secs": 120}
    cooldown_secs INTEGER NOT NULL DEFAULT 300,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    CHECK (monitor_id IS NOT NULL OR server_id IS NOT NULL)
);

CREATE TABLE status_pages (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    slug         TEXT NOT NULL UNIQUE,      -- public URL slug
    title        TEXT NOT NULL,
    config       JSONB NOT NULL DEFAULT '{}'::jsonb, -- which monitors/servers to show
    is_public    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_servers_namespace ON servers(namespace_id);
CREATE INDEX idx_monitors_namespace ON monitors(namespace_id);
