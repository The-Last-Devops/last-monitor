-- Namespaces become k8s-style: a single DNS-label `name` identifies them
-- (no separate slug + display name). Adopt the existing lowercase slug as name.
UPDATE namespaces SET name = slug;
ALTER TABLE namespaces DROP COLUMN slug;
ALTER TABLE namespaces ADD CONSTRAINT namespaces_name_key UNIQUE (name);
