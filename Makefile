# Vautr developer orchestration. Thin wrappers over the real build tools so a
# fresh checkout can go green with `make <target>` instead of memorizing every
# cargo/pnpm/ffi invocation. AGENTS.md is the authoritative dev reference.

CARGO ?= cargo
PNPM ?= pnpm
ANDROID_NDK_HOME ?= $(HOME)/Library/Android/sdk/ndk/27.1.12297006

.PHONY: help mobile-bootstrap web-bootstrap server-build test-all fmt clippy check-wasm prepare-wasm

help:
	@echo "Vautr Makefile — developer onboarding targets:"
	@echo "  mobile-bootstrap  Build the FFI native lib + generate bindings, install mobile deps"
	@echo "  web-bootstrap     Build the WASM crypto modules, install web deps"
	@echo "  server-build      Build the Rust server (release)"
	@echo "  test-all          Run all Rust + TypeScript tests"
	@echo "  fmt               cargo fmt across the workspace"
	@echo "  clippy            cargo clippy (warnings denied) across touched crates"
	@echo "  prepare-wasm      Build all WASM crypto artifacts (web + extension)"
	@echo "  check-wasm        Fail if any prebuilt WASM artifact is missing"

# Mobile: build the host + ios-sim FFI lib, regenerate Swift/Kotlin bindings, then
# install JS deps. The .so cross-compile is a separate step (see vautr-mobile-uniffi
# skill) because it needs the Android NDK.
mobile-bootstrap:
	$(CARGO) build -p vautr-ffi
	$(CARGO) build -p vautr-ffi --target aarch64-apple-ios-sim --release
	bash scripts/gen-ffi-bindings.sh
	cd apps/mobile && $(PNPM) install

web-bootstrap:
	bash scripts/prepare-wasm.sh
	cd apps/web && $(PNPM) install

server-build:
	$(CARGO) build -p vautr-server --release

test-all:
	$(CARGO) test --workspace
	cd apps/web && $(PNPM) test
	cd apps/extension && $(PNPM) test

prepare-wasm:
	bash scripts/prepare-wasm.sh

check-wasm:
	node scripts/check-wasm.mjs

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace --all-targets
