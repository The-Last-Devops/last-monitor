-- TOTP two-factor auth (RFC 6238) — opt-in, per user. Works with Google
-- Authenticator / 1Password / Authy etc. (see docs/auth-2fa-passkey.md).
--
-- The shared secret is sealed under the application secret (EXEC_APP_SECRET), the
-- same one that wraps SSH-key master keys, so a DB-only leak can't mint codes.
-- `totp_kid` records which app secret sealed it (NULL = stored raw, dev only).
-- Backup codes are stored as a JSON array of sha256 hex hashes; each is single-use.

ALTER TABLE users
    ADD COLUMN totp_secret_enc  BYTEA,                         -- sealed TOTP secret (NULL until enrolled)
    ADD COLUMN totp_kid         TEXT,                          -- app-secret id that sealed it
    ADD COLUMN totp_enabled     BOOLEAN NOT NULL DEFAULT false, -- false while a secret is pending verification
    ADD COLUMN totp_backup_codes TEXT;                         -- JSON array of sha256(code) hex, one-time use
