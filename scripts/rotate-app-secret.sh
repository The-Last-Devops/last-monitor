#!/usr/bin/env bash
# Rotate (or first-time enable) the application secret that wraps the OUTER layer of
# every user's SSH-key master key. Re-wraps the outer layer only — no user passwords
# are needed. See README "SSH key encryption (EXEC_APP_SECRET)".
#
# Usage:
#   EXEC_APP_SECRET="<new secret>" \
#   EXEC_APP_SECRET_OLD="<previous secret, if rotating>" \
#   CONFIG_DATABASE_URL=… DATA_DATABASE_URL=… \
#     bash scripts/rotate-app-secret.sh
#
# After it succeeds: unset EXEC_APP_SECRET_OLD and restart the hub with only the new
# EXEC_APP_SECRET. KEEP THE SECRET BACKED UP — losing it makes every SSH key unrecoverable.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

if [ -z "${EXEC_APP_SECRET:-}" ]; then
  echo "EXEC_APP_SECRET must be set (the secret to wrap TO)." >&2
  exit 1
fi
: "${CONFIG_DATABASE_URL:?CONFIG_DATABASE_URL is required}"
: "${DATA_DATABASE_URL:?DATA_DATABASE_URL is required}"

# Prefer an already-built release binary; fall back to `cargo run`.
if [ -x "./target/release/vantage-hub" ]; then
  exec ./target/release/vantage-hub rotate-app-secret
elif [ -x "./target/debug/vantage-hub" ]; then
  exec ./target/debug/vantage-hub rotate-app-secret
else
  exec cargo run -q -p vantage-hub -- rotate-app-secret
fi
