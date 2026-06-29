#!/usr/bin/env bash
# Smoke-test the TOTP 2FA flow against a running local hub: enroll → enable →
# login requires a code → backup code works → disable. Self-cleaning. Computes
# TOTP codes with the Python standard library (no extra deps).
#   ADMIN_EMAIL=tfatest@local ADMIN_PASSWORD=TfaTest-Aa123456 bash scripts/check-2fa.sh
set -euo pipefail
HUB="${HUB:-http://localhost:8080}"
EMAIL="${ADMIN_EMAIL:-tfatest@local}"
PASS="${ADMIN_PASSWORD:-TfaTest-Aa123456}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
say() { printf '  %s\n' "$*"; }

code_for() { # $1 = base32 secret → current 6-digit TOTP
  python3 - "$1" <<'PY'
import base64,hmac,hashlib,struct,time,sys
s=sys.argv[1].upper(); s+='='*((8-len(s)%8)%8)
key=base64.b32decode(s)
msg=struct.pack('>Q', int(time.time())//30)
h=hmac.new(key,msg,hashlib.sha1).digest()
o=h[19]&15
print('%06d'%((struct.unpack('>I',h[o:o+4])[0]&0x7fffffff)%1000000))
PY
}

echo "1) login (no 2FA yet)"
c=$(curl -s -o /dev/null -w '%{http_code}' -c "$JAR" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" "$HUB/api/auth/login")
[ "$c" = 200 ] || { echo "FAIL: login $c"; exit 1; }; say "200"

echo "2) start enrollment → secret"
SECRET=$(curl -s -b "$JAR" -X POST "$HUB/api/me/2fa/start" | grep -oE '"secret":"[^"]*"' | cut -d'"' -f4)
[ -n "$SECRET" ] || { echo "FAIL: no secret"; exit 1; }; say "secret=$SECRET"

echo "3) enable with a computed code"
CODE=$(code_for "$SECRET")
BODY=$(curl -s -b "$JAR" -H 'content-type: application/json' -d "{\"code\":\"$CODE\"}" "$HUB/api/me/2fa/enable")
echo "$BODY" | grep -q backup_codes || { echo "FAIL: enable did not return backup codes: $BODY"; exit 1; }
BACKUP=$(echo "$BODY" | grep -oE '"[0-9a-f]{5}-[0-9a-f]{5}"' | head -1 | tr -d '"')
say "enabled; sample backup=$BACKUP"

echo "4) status shows enabled"
curl -s -b "$JAR" "$HUB/api/me/2fa" | grep -q '"enabled":true' || { echo "FAIL: not enabled"; exit 1; }; say "enabled:true"

echo "5) login without code → twofa_required"
R=$(curl -s -H 'content-type: application/json' -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" "$HUB/api/auth/login")
echo "$R" | grep -q '"twofa_required":true' || { echo "FAIL: expected twofa_required, got: $R"; exit 1; }; say "twofa_required"

echo "6) login WITH a fresh code → success"
sleep 1; CODE=$(code_for "$SECRET")
c=$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\",\"totp_code\":\"$CODE\"}" "$HUB/api/auth/login")
[ "$c" = 200 ] || { echo "FAIL: login+code $c"; exit 1; }; say "200"

echo "7) login with a BACKUP code → success (one-time)"
c=$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\",\"totp_code\":\"$BACKUP\"}" "$HUB/api/auth/login")
[ "$c" = 200 ] || { echo "FAIL: backup login $c"; exit 1; }; say "200"
# the same backup code must now be rejected
c=$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\",\"totp_code\":\"$BACKUP\"}" "$HUB/api/auth/login")
[ "$c" = 401 ] || { echo "FAIL: reused backup code not rejected ($c)"; exit 1; }; say "reused backup rejected (401)"

echo "8) disable (cleanup)"
c=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -H 'content-type: application/json' \
  -d "{\"password\":\"$PASS\"}" "$HUB/api/me/2fa/disable")
[ "$c" = 204 ] || { echo "FAIL: disable $c"; exit 1; }; say "disabled (204)"

echo "ALL OK"
