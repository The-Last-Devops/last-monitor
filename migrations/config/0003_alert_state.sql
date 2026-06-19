-- Per-rule alert state, so the engine fires once on transition (not every tick),
-- sends recovery, and honours cooldown for re-notification while still firing.
CREATE TABLE alert_state (
    rule_id       UUID PRIMARY KEY REFERENCES alert_rules(id) ON DELETE CASCADE,
    firing        BOOLEAN NOT NULL DEFAULT false,
    last_changed  TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_notified TIMESTAMPTZ
);
