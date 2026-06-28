-- Correct the exec credential model to PER-USER keys (docs/exec-design.md).
--
-- Anyone who shells into a host has their OWN OS account + key on that host; the hub
-- only provides the transport and stores their private key for them. So the SSH
-- credential is per (user, system) — NOT a shared per-system secret. 0017 modelled
-- it as a shared system key; this drops that and adds the per-user table.

-- Drop the shared-credential columns from 0017 (wrong model). `systems` keeps
-- shell_enabled (hub-side opt-in) + ssh_port (the loopback port the tunnel forwards
-- to) + ssh_host (kept for an explicit override; defaults to the tunnel loopback).
ALTER TABLE systems
    DROP COLUMN IF EXISTS ssh_user,
    DROP COLUMN IF EXISTS ssh_key_enc,
    DROP COLUMN IF EXISTS ssh_key_fingerprint;

-- Per-user SSH credential for a host. The private key is encrypted under a key
-- derived from the USER'S OWN password (argon2 over `kdf_salt`, a dedicated salt —
-- never the auth-hash salt). The hub can decrypt it only while the user supplies
-- their password at step-up; there is no server master key. Forgot/reset password =>
-- the row is undecryptable and the user re-uploads (only their own key is affected).
CREATE TABLE exec_credentials (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    system_id       UUID NOT NULL REFERENCES systems(id) ON DELETE CASCADE,
    ssh_user        TEXT NOT NULL,          -- the user's OS account name on that host
    key_enc         BYTEA NOT NULL,         -- AEAD(private key) under the password-derived KEK
    kdf_salt        BYTEA NOT NULL,         -- argon2 salt for the KEK (distinct from the auth hash)
    key_fingerprint TEXT NOT NULL,          -- public fingerprint; safe to show in the UI
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, system_id)             -- one credential per user per host
);
CREATE INDEX idx_exec_credentials_user   ON exec_credentials(user_id);
CREATE INDEX idx_exec_credentials_system ON exec_credentials(system_id);
