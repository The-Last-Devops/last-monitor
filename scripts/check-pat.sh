#!/usr/bin/env bash
# Verify programmatic access tokens (PATs): mint one, use it as a Bearer token on a
# protected endpoint, confirm a bogus token is rejected, then revoke + confirm 401.
# Self-cleaning.
#   bash scripts/check-pat.sh
set -uo pipefail
BASE="${BASE:-http://localhost:8080}"
EMAIL="${ADMIN_EMAIL:-admin@local}"
PASS="${ADMIN_PASSWORD:-admin123}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
py() { python3 -c "$1"; }
say() { printf '%-40s ' "$1"; }
fail=0
chk() { say "$1"; if [ "$2" = "$3" ]; then echo "ok ($3)"; else echo "FAIL want $2 got $3"; fail=1; fi; }

for i in $(seq 1 60); do curl -s -o /dev/null -m 2 "$BASE/healthz" && break; sleep 1; done
curl -s -c "$JAR" -o /dev/null -X POST "$BASE/api/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}"

# mint a PAT (using the admin session)
RESP=$(curl -s -b "$JAR" -X POST "$BASE/api/pats" -H 'content-type: application/json' -d '{"name":"pat-probe"}')
TOKEN=$(printf '%s' "$RESP" | py "import sys,json;print(json.load(sys.stdin).get('token',''))")
PID=$(printf '%s' "$RESP" | py "import sys,json;print(json.load(sys.stdin).get('id',''))")
say "minted token"; [ -n "$TOKEN" ] && echo "ok (${TOKEN:0:14}…)" || { echo "FAIL"; fail=1; }

# the token works as a Bearer on a protected endpoint (NO cookie)
chk "Bearer token authorizes /api/systems" 200 \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$BASE/api/systems")"
# a bogus token is rejected
chk "bogus token rejected" 401 \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer lm_pat_bogus" "$BASE/api/systems")"
# no auth at all is rejected
chk "no auth rejected" 401 \
  "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/systems")"

# revoke, then the same token must stop working
curl -s -b "$JAR" -o /dev/null -X DELETE "$BASE/api/pats/$PID"
chk "revoked token rejected" 401 \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$BASE/api/systems")"

[ "$fail" -eq 0 ] && echo "OK" || { echo "PAT regressions"; exit 1; }
