-- Audit trail of user actions: every mutating API call (who / what / when / result).
CREATE TABLE audit_log (
    id         BIGSERIAL PRIMARY KEY,
    at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_email TEXT,                 -- NULL for unauthenticated calls
    method     TEXT NOT NULL,        -- POST / PATCH / PUT / DELETE
    path       TEXT NOT NULL,
    status     INTEGER NOT NULL
);
CREATE INDEX idx_audit_at ON audit_log (at DESC);
