-- SSH keys belong to the ACCOUNT, not the server (docs/exec-design.md, revised).
-- A user keeps a library of named keys, reusable across hosts. At connect time they
-- choose how to authenticate: their host password, or one of these keys (unsealed
-- with their account password). This replaces the per-(user,system) exec_credentials.

DROP TABLE IF EXISTS exec_credentials;

CREATE TABLE ssh_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,            -- the user's label for the key
    key_enc         BYTEA NOT NULL,           -- AEAD(private key) under the user's password KEK
    kdf_salt        BYTEA NOT NULL,           -- argon2 salt (distinct from the auth-hash salt)
    key_fingerprint TEXT NOT NULL,            -- public; shown in the UI
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)                    -- names are unique within a user's library
);
CREATE INDEX idx_ssh_keys_user ON ssh_keys(user_id);
