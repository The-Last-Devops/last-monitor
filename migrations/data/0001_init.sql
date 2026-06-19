-- Data DB: time-series metrics + service heartbeats. PostgreSQL + TimescaleDB.
-- Linked to the config DB only by IDs (server_id / monitor_id) at the app layer.

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Host metrics pushed by agents.
CREATE TABLE metrics (
    time        TIMESTAMPTZ NOT NULL,
    server_id   UUID NOT NULL,
    cpu_percent DOUBLE PRECISION NOT NULL,
    mem_used    BIGINT NOT NULL,
    mem_total   BIGINT NOT NULL,
    swap_used   BIGINT NOT NULL,
    swap_total  BIGINT NOT NULL,
    disk_used   BIGINT NOT NULL,
    disk_total  BIGINT NOT NULL,
    net_rx      BIGINT NOT NULL,
    net_tx      BIGINT NOT NULL,
    load1       DOUBLE PRECISION NOT NULL,
    uptime      BIGINT NOT NULL
);

SELECT create_hypertable('metrics', 'time');
CREATE INDEX idx_metrics_server_time ON metrics (server_id, time DESC);

-- Service check results.
CREATE TABLE heartbeats (
    time        TIMESTAMPTZ NOT NULL,
    monitor_id  UUID NOT NULL,
    up          BOOLEAN NOT NULL,
    latency_ms  INTEGER,
    status_code INTEGER,
    message     TEXT
);

SELECT create_hypertable('heartbeats', 'time');
CREATE INDEX idx_heartbeats_monitor_time ON heartbeats (monitor_id, time DESC);

-- Retention: keep raw samples 7 days. Downsampling via continuous aggregates
-- can be layered on later (1m / 1h rollups) before tightening this further.
SELECT add_retention_policy('metrics', INTERVAL '7 days');
SELECT add_retention_policy('heartbeats', INTERVAL '30 days');

-- Compress older chunks to save space.
ALTER TABLE metrics SET (timescaledb.compress, timescaledb.compress_segmentby = 'server_id');
SELECT add_compression_policy('metrics', INTERVAL '1 day');
