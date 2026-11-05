#!/usr/bin/env bash
# Regenerate the Kotlin/Swift uniffi bindings for the mobile native module.
# Run from repo root: scripts/gen-ffi-bindings.sh
#
# VTRFIX-INFRA-M08: the generated files used to land ONLY in
# modules/vautr-native/ffi-bindings — the legacy module, which is no longer
# the workspace-linked package (apps/mobile depends on packages/native,
# `@vautr/native`, per pnpm-lock.yaml). That guaranteed drift between what the
# generator produced and what actually ships. We still generate into the legacy
# dir (binding-gen scripts reference it), then mirror every artifact into the
# linked module's real destinations (paths mirror the existing files there).
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> building vautr-ffi"
cargo build -p vautr-ffi

OUT_DIR="modules/vautr-native/ffi-bindings"
echo "==> generating bindings -> $OUT_DIR"
cargo run -p vautr-ffi --example gen_bindings "$OUT_DIR"

# --- Mirror the fresh artifacts into the LINKED module (packages/native) ---
# Destination paths/names mirror the existing files there:
#   iOS pod sources (podspec consumes vautr_ffi/vautr_ffi*.swift|.h|.modulemap)
#   Kotlin: both checked-in copies (ios/ + android src set, nested uniffi dir)
LINKED_IOS="packages/native/ios"
LINKED_ANDROID_KT="packages/native/android/src/main/java/uniffi/vautr_ffi/uniffi/vautr_ffi"

echo "==> syncing bindings -> packages/native (linked module)"
mkdir -p "$LINKED_IOS/vautr_ffi" "$LINKED_IOS/uniffi/vautr_ffi" "$LINKED_ANDROID_KT"
cp "$OUT_DIR/vautr_ffi.swift"        "$LINKED_IOS/vautr_ffi/vautr_ffi.swift"
cp "$OUT_DIR/vautr_ffiFFI.h"         "$LINKED_IOS/vautr_ffi/vautr_ffiFFI.h"
cp "$OUT_DIR/vautr_ffiFFI.modulemap" "$LINKED_IOS/vautr_ffi/vautr_ffiFFI.modulemap"
# Keep the older top-level copies in ios/ in sync (they exist and must not drift).
cp "$OUT_DIR/vautr_ffiFFI.h"         "$LINKED_IOS/vautr_ffiFFI.h"
cp "$OUT_DIR/vautr_ffiFFI.modulemap" "$LINKED_IOS/vautr_ffiFFI.modulemap"
# Kotlin bindings (same file, two checked-in locations).
cp "$OUT_DIR/uniffi/vautr_ffi/vautr_ffi.kt" "$LINKED_IOS/uniffi/vautr_ffi/vautr_ffi.kt"
cp "$OUT_DIR/uniffi/vautr_ffi/vautr_ffi.kt" "$LINKED_ANDROID_KT/vautr_ffi.kt"

echo "==> done. Review modules/vautr-native/ffi-bindings/ and packages/native/"
