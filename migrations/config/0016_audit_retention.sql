-- How many days of audit log to keep. NULL = keep forever (default).
ALTER TABLE app_settings ADD COLUMN IF NOT EXISTS audit_retention_days int;
-- Speeds up the newest-first listing, the filtered counts, and retention pruning.
CREATE INDEX IF NOT EXISTS audit_log_at_idx ON audit_log (at DESC);
