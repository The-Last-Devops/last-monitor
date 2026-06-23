-- Scheduled-backup settings + last-run marker (singleton app_settings row).
ALTER TABLE app_settings ADD COLUMN IF NOT EXISTS backup jsonb;
ALTER TABLE app_settings ADD COLUMN IF NOT EXISTS last_backup_at timestamptz;
