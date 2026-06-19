-- Temperature sensors stored alongside each metrics sample.
ALTER TABLE metrics ADD COLUMN temps JSONB;

-- Per-container resource usage (Beszel-style Docker stats).
CREATE TABLE container_stats (
    time        TIMESTAMPTZ NOT NULL,
    server_id   UUID NOT NULL,
    name        TEXT NOT NULL,
    cpu_percent DOUBLE PRECISION NOT NULL,
    mem_used    BIGINT NOT NULL,
    net_rx      BIGINT NOT NULL,
    net_tx      BIGINT NOT NULL
);

SELECT create_hypertable('container_stats', 'time');
CREATE INDEX idx_container_stats_server_time ON container_stats (server_id, time DESC);
SELECT add_retention_policy('container_stats', INTERVAL '7 days');
