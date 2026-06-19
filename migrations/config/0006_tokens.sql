-- Reusable enrollment tokens: one token can enroll MANY servers (e.g. a k8s
-- DaemonSet). A server is identified by (token_id, hostname) and auto-registers
-- on first metrics push. Deleting a token removes every server it enrolled.
CREATE TABLE enrollment_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace_id UUID NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token        TEXT NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Migrate existing per-server tokens into reusable tokens.
INSERT INTO enrollment_tokens (namespace_id, name, token)
    SELECT namespace_id, name, agent_token FROM servers;

ALTER TABLE servers ADD COLUMN token_id UUID REFERENCES enrollment_tokens(id) ON DELETE CASCADE;
UPDATE servers s SET token_id = t.id FROM enrollment_tokens t WHERE t.token = s.agent_token;
UPDATE servers SET hostname = name WHERE hostname IS NULL;

ALTER TABLE servers ALTER COLUMN hostname SET NOT NULL;
ALTER TABLE servers ALTER COLUMN token_id SET NOT NULL;
ALTER TABLE servers DROP COLUMN agent_token;

-- One server per (token, hostname).
CREATE UNIQUE INDEX servers_token_host ON servers (token_id, hostname);
