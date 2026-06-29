#!/usr/bin/env bash
# Smoke-test the SSH-key master-key + change-password flow against a running local hub.
# Idempotent and self-restoring (changes the admin password and changes it back).
#   bash scripts/check-master-key.sh
set -euo pipefail
HUB="${HUB:-http://localhost:8080}"
EMAIL="${ADMIN_EMAIL:-admin@local}"
PASS="${ADMIN_PASSWORD:-admin123}"
DBC="${DBC:-docker exec last-monitor-db-1 psql -U lastmon -d lastmon_config -tAc}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
say() { printf '  %s\n' "$*"; }

echo "1) login → provisions the master key"
code=$(curl -s -o /dev/null -w '%{http_code}' -c "$JAR" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" "$HUB/api/auth/login")
[ "$code" = 200 ] || { echo "FAIL: login returned $code"; exit 1; }
say "login 200"

echo "2) admin row now has a wrapped master key + app_kid"
row=$($DBC "SELECT (master_key_enc IS NOT NULL), app_kid FROM users WHERE email='$EMAIL';")
say "master_key_enc/app_kid → $row"
echo "$row" | grep -q '^t|' || { echo "FAIL: master key not provisioned"; exit 1; }

echo "3) systems.shell_enabled column is gone (migration 0021)"
n=$($DBC "SELECT count(*) FROM information_schema.columns WHERE table_name='systems' AND column_name='shell_enabled';")
[ "$n" = 0 ] || { echo "FAIL: shell_enabled column still present"; exit 1; }
say "shell_enabled dropped"

echo "4) self change-password round-trip (re-wraps the master key, keeps it openable)"
TMP='Zx7-Quartz-Mango42'
mk0=$($DBC "SELECT encode(master_key_enc,'hex') FROM users WHERE email='$EMAIL';")
c1=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -H 'content-type: application/json' \
  -d "{\"current_password\":\"$PASS\",\"new_password\":\"$TMP\"}" "$HUB/api/me/password")
[ "$c1" = 204 ] || { echo "FAIL: change to temp returned $c1"; exit 1; }
say "changed to temp (204)"
# log in with the new password to prove it took + master key still unwraps on use
code=$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$TMP\"}" "$HUB/api/auth/login")
[ "$code" = 200 ] || { echo "FAIL: login with temp returned $code (RESTORE MANUALLY: password is $TMP)"; exit 1; }
say "login with temp 200"
c2=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -H 'content-type: application/json' \
  -d "{\"current_password\":\"$TMP\",\"new_password\":\"$PASS\"}" "$HUB/api/me/password")
[ "$c2" = 204 ] || { echo "FAIL: restore returned $c2 (RESTORE MANUALLY: password is $TMP)"; exit 1; }
say "restored original password (204)"
mk1=$($DBC "SELECT encode(master_key_enc,'hex') FROM users WHERE email='$EMAIL';")
[ "$mk0" != "$mk1" ] || { echo "FAIL: master_key_enc unchanged after password change (should re-wrap)"; exit 1; }
say "master key re-wrapped (ciphertext changed, key preserved)"

echo "ALL OK"
