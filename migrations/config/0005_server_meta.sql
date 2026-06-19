-- Host metadata reported by the agent (latest value kept on the server row).
ALTER TABLE servers ADD COLUMN kernel        TEXT;
ALTER TABLE servers ADD COLUMN cpu_model     TEXT;
ALTER TABLE servers ADD COLUMN cpu_cores     INTEGER;
ALTER TABLE servers ADD COLUMN agent_version TEXT;
