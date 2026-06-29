-- TOTP replay defense: remember the last accepted 30s step per user and reject any
-- code from that step or earlier, so a code can't be reused within its validity
-- window (standard RFC 6238 mitigation). See api/twofa.rs / totp.rs.
ALTER TABLE users ADD COLUMN totp_last_step BIGINT NOT NULL DEFAULT 0;
