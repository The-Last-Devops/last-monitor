#!/usr/bin/env bash
# Build the embedded Tailwind stylesheet. Downloads the standalone Tailwind CLI
# (no Node) on first run. Output: crates/hub/static/app.css (embedded into the
# hub binary via include_str! at compile time, so regenerate before `cargo build`
# when you change UI classes).
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=tooling/tailwindcss
if [ ! -x "$BIN" ]; then
  mkdir -p tooling
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) A=macos-arm64 ;;
    Darwin-x86_64) A=macos-x64 ;;
    Linux-x86_64) A=linux-x64 ;;
    Linux-aarch64) A=linux-arm64 ;;
    *) echo "unsupported platform"; exit 1 ;;
  esac
  echo "downloading tailwindcss ($A)…"
  curl -fsSL -o "$BIN" \
    "https://github.com/tailwindlabs/tailwindcss/releases/download/v3.4.17/tailwindcss-$A"
  chmod +x "$BIN"
fi

"$BIN" -c web/tailwind.config.js -i web/input.css -o crates/hub/static/app.css --minify
echo "wrote crates/hub/static/app.css"
