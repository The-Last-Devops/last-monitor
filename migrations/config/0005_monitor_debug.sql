-- Last successful + last failed request/response per monitor, for debugging
-- (e.g. inspect the body behind a bare 406). Exactly two rows per monitor.
CREATE TABLE monitor_debug (
    monitor_id UUID NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    outcome    TEXT NOT NULL,            -- 'ok' | 'err'
    detail     JSONB NOT NULL,
    at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (monitor_id, outcome)
);
