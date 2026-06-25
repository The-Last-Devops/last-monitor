#!/usr/bin/env bash
# Health-check the dev stack used while iterating on the UI:
#   - hub API on :8080 (healthz)
#   - Vite dev server on :5173 (the URL you open in the browser)
#   - /api/about (feeds the version badge in the top bar)
#   bash scripts/check-dev-stack.sh
set -uo pipefail
HUB="${HUB_URL:-http://localhost:8080}"
VITE="${VITE_URL:-http://localhost:5173}"
EMAIL="${ADMIN_EMAIL:-admin@local}"
PASS="${ADMIN_PASSWORD:-admin123}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
say() { printf '%-26s ' "$1"; }

say "hub /healthz"; curl -s -o /dev/null -w '%{http_code}\n' -m 3 "$HUB/healthz" || echo "DOWN"
say "vite :5173";   curl -s -o /dev/null -w '%{http_code}\n' -m 3 "$VITE/"       || echo "DOWN"

curl -s -c "$JAR" -o /dev/null -X POST "$HUB/api/auth/login" \
  -H 'content-type: application/json' -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}"
say "/api/about version"
curl -s -b "$JAR" "$HUB/api/about" \
  | python3 -c "import sys,json;d=json.load(sys.stdin);print(d.get('version','?'),'(build',d.get('git_sha','?')+')')"
echo "OK"
