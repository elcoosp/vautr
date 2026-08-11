#!/usr/bin/env bash
# CI guard (prd.md §"Restricted API" / data.md §1 rule 4):
# `read_secret` must NOT exist in any mobile or web artifact. It is only
# available to the Desktop (GPUI) build via the `desktop-api` feature.
#
# We build the restricted (feature-less) artifacts and assert the `read_secret`
# symbol string is absent, then sanity-check that a `desktop-api` build DOES
# contain it (so the check is meaningful, not vacuously passing).
#
# Usage: scripts/check-restricted-api.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SYMBOL="read_secret"
FAIL=0
# wasm-bindgen exports the method as `webclient_read_secret`; the Rust symbol
# also survives mangled in the wasm. Match either form.
sym_present() {
  # $1 = artifact path. Returns 0 if the symbol is present.
  # NOTE: must NOT use `grep -q` in a pipe under `pipefail` — grep -q closes the
  # pipe early, the producer (nm/strings) gets SIGPIPE, and pipefail turns the
  # match into a failure. `grep -c` reads to EOF, so it is safe.
  local n
  n=$(nm "$1" 2>/dev/null | grep -ac "$SYMBOL")
  if [ "$n" -gt 0 ]; then return 0; fi
  n=$(strings -a "$1" 2>/dev/null | grep -ac "$SYMBOL")
  [ "$n" -gt 0 ]
}


# --- Mobile (UniFFI staticlib) --------------------------------------------
echo "== Building vautr-ffi (staticlib, no desktop-api) =="
cargo build -p vautr-ffi --lib 2>&1 | tail -1
FFI_A="target/debug/libvautr_ffi.a"
if [ ! -f "$FFI_A" ]; then
  echo "ERROR: expected artifact $FFI_A not found"
  exit 1
fi
if sym_present "$FFI_A"; then
  echo "FAIL: '$SYMBOL' symbol present in mobile staticlib ($FFI_A)"
  FAIL=1
else
  echo "OK:   '$SYMBOL' absent from mobile staticlib"
fi

# --- Web (wasm-bindgen) --------------------------------------------------
echo "== Building vautr-wasm (wasm32, no desktop-api) =="
cargo build -p vautr-wasm --lib --target wasm32-unknown-unknown 2>&1 | tail -1
WASM="target/wasm32-unknown-unknown/debug/vautr_wasm.wasm"
if [ ! -f "$WASM" ]; then
  echo "ERROR: expected artifact $WASM not found"
  exit 1
fi
if sym_present "$WASM"; then
  echo "FAIL: '$SYMBOL' symbol present in web wasm ($WASM)"
  FAIL=1
else
  echo "OK:   '$SYMBOL' absent from web wasm"
fi

# --- Sanity: desktop-api build MUST contain it ---------------------------
echo "== Sanity: vautr-wasm (wasm32, desktop-api) should contain '$SYMBOL' =="
cargo build -p vautr-wasm --lib --target wasm32-unknown-unknown --features desktop-api 2>&1 | tail -1
WASM_D="target/wasm32-unknown-unknown/debug/vautr_wasm.wasm"
if sym_present "$WASM_D"; then
  echo "OK:   '$SYMBOL' present in desktop-api web build (sanity check passed)"
else
  echo "FAIL: '$SYMBOL' missing even in desktop-api build (check gate is broken)"
  FAIL=1
fi

if [ "$FAIL" -ne 0 ]; then
  echo "::error:: Restricted-API symbol check FAILED"
  exit 1
fi
echo "== Restricted-API symbol check PASSED =="
