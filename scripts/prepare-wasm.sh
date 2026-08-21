#!/bin/bash
# Build the Vautr WASM crypto modules from source.
#
# The web and extension clients consume prebuilt wasm from `wasm-pkg` dirs; they
# must NEVER fall back to a throwing dev shim. Run this before `pnpm build` (or
# rely on the `prepare`/`predev` hooks that call it) so the shipped bundle always
# carries real crypto. See AGENTS.md + docs/audit/codebase-homogeneity-audit.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> Building web client wasm (vautr-wasm --target web)"
wasm-pack build core/vautr-wasm --target web --out-dir apps/web/wasm-pkg --out-name vautr_wasm

echo "==> Building extension popup wasm (vautr-wasm --target web)"
wasm-pack build core/vautr-wasm --target web --out-dir apps/extension/wasm-pkg --out-name vautr_wasm

echo "==> Building extension service-worker crypto wasm (vautr-crypto-wasm --target nodejs)"
wasm-pack build core/vautr-crypto-wasm --target nodejs --out-dir apps/extension/sw-wasm-pkg-nodejs --out-name vautr_crypto_wasm

echo "==> WASM build complete."
