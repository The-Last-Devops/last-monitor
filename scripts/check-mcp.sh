#!/usr/bin/env bash
# Smoke-test the embedded MCP server: mint a PAT, then drive POST /mcp over
# JSON-RPC (initialize → tools/list → tools/call) authed by that token. Confirms
# unauthenticated access is rejected. Self-cleaning.
#   bash scripts/check-mcp.sh
set -uo pipefail
BASE="${BASE:-http://localhost:8080}"
EMAIL="${ADMIN_EMAIL:-admin@local}"
PASS="${ADMIN_PASSWORD:-admin123}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
py() { python3 -c "$1"; }
say() { printf '%-40s ' "$1"; }
fail=0

for i in $(seq 1 60); do curl -s -o /dev/null -m 2 "$BASE/healthz" && break; sleep 1; done
curl -s -c "$JAR" -o /dev/null -X POST "$BASE/api/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}"
RESP=$(curl -s -b "$JAR" -X POST "$BASE/api/pats" -H 'content-type: application/json' -d '{"name":"mcp-probe"}')
TOKEN=$(printf '%s' "$RESP" | py "import sys,json;print(json.load(sys.stdin).get('token',''))")
PID=$(printf '%s' "$RESP" | py "import sys,json;print(json.load(sys.stdin).get('id',''))")

mcp() { curl -s -X POST "$BASE/mcp" -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' -d "$1"; }

say "initialize -> serverInfo.name"
printf '%s' "$(mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}')" \
  | py "import sys,json;d=json.load(sys.stdin);print(d['result']['serverInfo']['name'])" | grep -qx "vantage" \
  && echo "ok" || { echo "FAIL"; fail=1; }

say "tools/list -> tool count"
N=$(printf '%s' "$(mcp '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')" | py "import sys,json;print(len(json.load(sys.stdin)['result']['tools']))")
[ "$N" -ge 6 ] && echo "ok ($N tools)" || { echo "FAIL ($N)"; fail=1; }

say "tools/call list_services -> content"
printf '%s' "$(mcp '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_services","arguments":{}}}')" \
  | py "import sys,json;d=json.load(sys.stdin);print('ok' if d['result']['content'][0]['type']=='text' else 'no')" | grep -qx ok \
  && echo "ok" || { echo "FAIL"; fail=1; }

say "tools/call unknown -> isError"
printf '%s' "$(mcp '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope","arguments":{}}}')" \
  | py "import sys,json;d=json.load(sys.stdin);print('err' if d.get('error') or d['result'].get('isError') else 'no')" | grep -qx err \
  && echo "ok" || { echo "FAIL"; fail=1; }

say "no auth rejected (401)"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"ping"}')" = 401 ] \
  && echo "ok" || { echo "FAIL"; fail=1; }

curl -s -b "$JAR" -o /dev/null -X DELETE "$BASE/api/pats/$PID"
[ "$fail" -eq 0 ] && echo "OK" || { echo "MCP regressions"; exit 1; }
