#!/usr/bin/env bash
# Regenerate the Kotlin/Swift uniffi bindings for the mobile native module.
# Run from repo root: scripts/gen-ffi-bindings.sh
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> building vautr-ffi"
cargo build -p vautr-ffi

OUT_DIR="modules/vautr-native/ffi-bindings"
echo "==> generating bindings -> $OUT_DIR"
cargo run -p vautr-ffi --example gen_bindings "$OUT_DIR"

echo "==> done. Review modules/vautr-native/ffi-bindings/"
