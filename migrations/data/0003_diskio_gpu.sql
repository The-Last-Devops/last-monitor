-- Disk I/O counters (cumulative bytes) + per-GPU stats snapshot.
ALTER TABLE metrics ADD COLUMN disk_read  BIGINT;
ALTER TABLE metrics ADD COLUMN disk_write BIGINT;
ALTER TABLE metrics ADD COLUMN gpus       JSONB;
