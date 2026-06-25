#!/usr/bin/env bash
# Guard against the "blank/black flash on navigation" regression.
# 1. No route may be lazy-imported (lazy routes blank the view during chunk fetch).
# 2. Every data-fetching page must own a loading flag so it shows a loader, not
#    an empty content area, before its first fetch settles.
#   bash scripts/check-no-blank.sh
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$REPO/frontend/src"
fail=0

# 1. router has no lazy route components (real lazy routes look like `=> import('../pages/…')`;
#    the empty-paren `() => import()` in a comment is ignored).
n=$(grep -cE "=> import\('" "$SRC/router/index.js" || true)
if [ "$n" -ne 0 ]; then
  echo "FAIL: $n lazy route import(s) in router/index.js — use eager imports"
  grep -nE "=> import\('" "$SRC/router/index.js" || true
  fail=1
else
  echo "ok: router uses eager imports (0 lazy)"
fi

# 2. pages that fetch on mount expose some loading/placeholder state (loader, a
#    first-load flag, a `checking`/`busy` flag, or a `!m`/`!meta` guard).
for f in "$SRC"/pages/*.vue; do
  name="$(basename "$f")"
  case "$name" in Login.vue) continue ;; esac          # login has no data load
  grep -q 'api.get\|api.post' "$f" || continue          # no fetching → no loader needed
  if grep -qE 'loading|loaded|checking|busy|Loading|!m"|!meta' "$f"; then
    echo "ok: $name has a loading state"
  else
    echo "FAIL: $name fetches data but has no loading flag"
    fail=1
  fi
done

[ "$fail" -eq 0 ] && echo "OK" || { echo "regressions found"; exit 1; }
