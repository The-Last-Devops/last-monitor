-- The per-host enable/disable SSH toggle was removed from the UI (it served no
-- purpose: access is already gated by RBAC `can_exec` + step-up + the host's own
-- SSH auth). Some hosts were left with shell_enabled = false and no way to turn it
-- back on. Drop the column entirely so every host is SSH-capable; whether a console
-- can actually open is decided by the live agent tunnel, not a stored flag.

ALTER TABLE systems DROP COLUMN IF EXISTS shell_enabled;
