#!/usr/bin/env bash
# Inspect what the hub serves at / : headers + whether it's the new Vue SPA
# (good) or the old SSR HTML (bad). Helps tell "browser cache" vs "server".
set -u
HUB_URL="${HUB_URL:-http://localhost:8080}"

echo "== response headers for / =="
curl -sS -D - -o /tmp/lm-root.html "$HUB_URL/" 2>&1 | grep -iE '^HTTP/|^content-type|^cache-control' || true

echo "== body markers =="
if grep -q 'id="app"' /tmp/lm-root.html; then echo "  ✓ NEW Vue SPA (<div id=app>)"; else echo "  ✗ no <div id=app>"; fi
if grep -qiE 'htmx|/ui/servers|Last Monitor|/static/app.css' /tmp/lm-root.html; then echo "  ✗ OLD SSR markers present"; else echo "  ✓ no old-SSR markers"; fi
echo "== script tags =="
grep -oE '<script[^>]*src="[^"]*"' /tmp/lm-root.html | head

echo "== an asset (should be cache-control immutable) =="
asset=$(grep -oE '/assets/[^"]+\.js' /tmp/lm-root.html | head -1)
[ -n "${asset:-}" ] && curl -sS -D - -o /dev/null "$HUB_URL$asset" 2>&1 | grep -iE '^HTTP/|^cache-control' || echo "  (no asset found in /)"
