#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
mode="${1:-full}"

# CI artifacts do not benefit from rustc incremental state between clean jobs.
export CARGO_INCREMENTAL=0

if [[ "$mode" != "full" && "$mode" != "rust-only" ]]; then
  echo "Unknown Linux verification mode: $mode (expected full or rust-only)" >&2
  exit 2
fi

if [[ "$mode" == "full" ]]; then
  npm ci
  npm run test:quick
fi

for dependency in webkit2gtk-4.1 javascriptcoregtk-4.1; do
  if ! pkg-config --exists "$dependency"; then
    echo "Missing Linux build dependency: $dependency" >&2
    exit 1
  fi
done

npm run test:rust
npx tauri build --debug --no-bundle
