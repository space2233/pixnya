#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

for dependency in webkit2gtk-4.1 javascriptcoregtk-4.1; do
  if ! pkg-config --exists "$dependency"; then
    echo "Missing Linux build dependency: $dependency" >&2
    exit 1
  fi
done

npm ci
npm run check
node --test scripts/*.test.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tauri build --debug --no-bundle
