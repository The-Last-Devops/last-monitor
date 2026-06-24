-- A rule may now fan out to several notify channels, and optionally re-notify
-- while still firing. Channels move from a single FK on `alerts` to a join table.

CREATE TABLE alert_channels (
    alert_id   UUID NOT NULL REFERENCES alerts(id)   ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    PRIMARY KEY (alert_id, channel_id)
);

-- carry existing single-channel wiring over
INSERT INTO alert_channels (alert_id, channel_id)
SELECT id, channel_id FROM alerts;

-- re-notify cadence while a rule stays firing; NULL = notify once, never repeat
ALTER TABLE alerts ADD COLUMN renotify_secs INTEGER;

-- single-channel column is now redundant
ALTER TABLE alerts DROP COLUMN channel_id;
