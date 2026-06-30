-- Move hub-wide settings from the wide singleton app_settings row to a key→value
-- store, so adding a setting no longer needs a schema migration. Values are JSONB;
-- types + defaults live in code (crate::settings).
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Backfill each non-null column of the old singleton row into a kv entry.
INSERT INTO settings (key, value)
SELECT k, v
FROM (
    SELECT 's3'                   AS k, s3                            AS v FROM app_settings WHERE id = 1
    UNION ALL SELECT 'backup',               backup                        FROM app_settings WHERE id = 1
    UNION ALL SELECT 'last_backup_at',       to_jsonb(last_backup_at)      FROM app_settings WHERE id = 1
    UNION ALL SELECT 'audit_retention_days', to_jsonb(audit_retention_days) FROM app_settings WHERE id = 1
    UNION ALL SELECT 'data_cap_limit_bytes', to_jsonb(data_cap_limit_bytes) FROM app_settings WHERE id = 1
    UNION ALL SELECT 'data_cap_enabled',     to_jsonb(data_cap_enabled)    FROM app_settings WHERE id = 1
) s
WHERE v IS NOT NULL AND v::text <> 'null'
ON CONFLICT (key) DO NOTHING;

-- Seed defaults for keys that may be missing (fresh install, or new settings).
INSERT INTO settings (key, value) VALUES
    ('data_cap_limit_bytes',           to_jsonb(10737418240::bigint)),  -- 10 GB
    ('data_cap_enabled',               to_jsonb(false)),
    ('exec_transcript_retention_days', to_jsonb(30)),
    ('alert_events_retention_days',    to_jsonb(365)),
    ('exec_sessions_retention_days',   to_jsonb(365)),
    ('sessions_retention_days',        to_jsonb(14))
ON CONFLICT (key) DO NOTHING;

DROP TABLE app_settings;
