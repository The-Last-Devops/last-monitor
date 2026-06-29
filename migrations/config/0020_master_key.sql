-- Envelope encryption for users' SSH keys (docs/exec-design.md).
--
-- Before: each SSH key was sealed directly under a KEK derived from the user's
-- password. Changing the password meant re-encrypting every key, and an admin
-- reset (no old password) would orphan them.
--
-- Now: each user gets one random per-user MASTER KEY. SSH keys are sealed under
-- the master key (which never changes), so a password change only re-wraps the
-- master key, not the keys. The master key is wrapped in two layers:
--   inner: a KEK derived from the user's password (argon2) — needs the password
--   outer: an application secret (EXEC_APP_SECRET, env/KMS, NOT in this DB) — so
--          a DB-only leak can't unwrap anything even with a guessed password.
-- `app_kid` records which app-secret version wrapped the outer layer, so the
-- secret can be rotated (re-wrap the outer layer only — no passwords needed).

ALTER TABLE users
    ADD COLUMN master_key_enc  BYTEA,  -- wrapped master key (inner [+ outer]); NULL until provisioned
    ADD COLUMN master_key_salt BYTEA,  -- argon2 salt for the password KEK (inner layer)
    ADD COLUMN app_kid         TEXT;   -- id of the app secret that wrapped the outer layer (NULL = no outer)

-- ssh_keys gains an encryption version. Existing rows are legacy (=1): sealed
-- directly under the password KEK using kdf_salt. New rows (=2) are sealed under
-- the master key; they're migrated to =2 transparently the next time they're used.
ALTER TABLE ssh_keys
    ADD COLUMN enc_ver SMALLINT NOT NULL DEFAULT 1;

-- ver=2 rows don't use a per-key argon2 salt (the master key is already random).
ALTER TABLE ssh_keys
    ALTER COLUMN kdf_salt DROP NOT NULL;
