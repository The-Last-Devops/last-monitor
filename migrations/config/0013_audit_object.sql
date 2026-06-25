-- Human-readable name of the object a mutating call targeted (e.g. the channel's
-- name), resolved at log time. Display name only — never config/secrets.
ALTER TABLE audit_log ADD COLUMN object_name TEXT;
