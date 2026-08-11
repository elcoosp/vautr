# Vautr Technology Stack – Final (July 2026)

**Version:** 2.0  
**Last updated:** July 2026  
**Maintainer:** Vautr Core Team

---

## 0. Version Philosophy

- **Major‑only pinning** for all crates that follow semver (≥1.0). For pre‑1.0 crates (0.x), pin to the **major version `0.x`** (e.g., `tokio = "1"`, `reqwest = "0.13"`, `zip = "2"`).
- **Pre‑release crates are included** — pinning to `0.13` automatically picks up `0.13.4` stable, while `0.6` picks up `0.6.0-rc.8` today.
- **GPUI** (no semver) remains pinned to a specific git commit hash.
- **Expo SDK** pinned to major version (`expo@57`).

---

## 1. Core Rust Dependencies

### 1.1 Cryptography & PKI

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `chacha20poly1305` | `0.11` | XChaCha20‑Poly1305 AEAD with optional hardware acceleration. Latest `0.11.0-rc.3`. |
| `argon2` | `0.6` | Argon2id KDF. Latest `0.6.0-rc.8`. |
| `hkdf` | `0.13` | HMAC‑based HKDF. Latest `0.13.0-rc.5`. |
| `opaque-ke` | `4.1` | OPAQUE aPAKE (RFC 9807). Latest `4.1.0-pre.1`. |
| `x25519-dalek` | `2` | X25519 for KEM‑DEM sharing. |
| `ed25519-dalek` | `2` | Ed25519 for RK authentication. |
| `bip39` | `2` | BIP‑39 mnemonic encoding/decoding for Emergency Kit. |
| `zeroize` | `1.8` | Secure memory zeroing. |
| `rand` / `rand_core` | `0.9` | CSPRNG backed by `OsRng`. Use `rand_core` with `getrandom` for WASM. |

### 1.2 Database (Local SQLite + ORM)

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `sea-orm` | `2.0` | Async ORM. Latest `2.0.0-rc.38`. |
| `sea-orm-migration` | `2.0` | Migration framework. |
| `sqlx` | `0.9` | Compile‑time checked SQL. Latest `0.9.0`. |

**Performance Pragmas:**
```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA cache_size=-2000;
PRAGMA page_size=4096;
PRAGMA wal_autocheckpoint=1000;
```

### 1.3 Networking (Client & Server)

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `reqwest` | `0.13` | Async HTTP client. Latest `0.13.4`. |
| `axum` | `0.8` | Async web framework. Latest `0.8.9`. |
| `tower` | `0.5` | Middleware abstraction. |
| `tower-http` | `0.6` | HTTP‑specific middleware. |

### 1.4 Async Runtime, Logging & Utilities

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `tokio` | `1` | Async runtime. Latest `1.52.1`. |
| `tracing` | `0.1` | Structured logging. |
| `tracing-subscriber` | `0.3` | Log subscriber. |
| `tracing-opentelemetry` | `0.28` | Bridges `tracing` to OpenTelemetry for metrics. |
| `anyhow` | `1` | Flexible error handling. |
| `thiserror` | `2` | Precise errors for libraries. |
| `serde` / `serde_json` | `1` / `1` | Serialisation framework. |
| `uuid` | `1` | UUID v4 generation. |
| `chrono` | `0.4` | Unix epoch timestamps. |
| `base64` | `0.22` | Base64 encoding for API. |
| `once_cell` | `1` | Lazy static initialisation. |

### 1.5 Import, File Storage & Parallel Processing

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `zip` / `async_zip` | `2` / `0.0.2` | Streaming ZIP extraction for 1Password imports. |
| `csv` | `1` | CSV parsing for browser exports. |
| `flate2` | `1` | Deflate compression for ZIP archives. |
| `rayon` | `1` | Data‑parallel encryption for import fast‑path. |
| `fs4` | `0.12` | Cross‑platform file locking for `vautr_files`. |
| `tempfile` | `3` | Sandboxed temporary directory for import extraction. |

### 1.6 Configuration & CLI

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `dotenvy` | `0.15` | `.env` file loading. |
| `clap` | `4` | CLI argument parsing for server/admin tools. |
| `figment` | `0.10` | Layered configuration (env + file + defaults). |

### 1.7 Observability & Metrics

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `opentelemetry` | `0.28` | OpenTelemetry SDK. |
| `opentelemetry-otlp` | `0.28` | OTLP exporter for telemetry. |
| `metrics` | `0.24` | Metrics facade. |
| `metrics-exporter-prometheus` | `0.15` | Prometheus exporter for SQLite health. |
| `sentry` | `0.47` | Crash reporting (user consent required). |

### 1.8 Cross‑Platform Bindings

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `uniffi` | `0.31` | Multi‑language bindings for mobile. |
| `wasm-bindgen` | `0.2` | JS bindings for web client. |
| `wasm-pack` | `0.15` | Build Rust → WASM packages. |
| `console_error_panic_hook` | `0.1` | Forwards Rust panics to browser console. |

---

## 2. Frontend (Web SPA + Browser Extension)

### 2.1 Shared (Web & Extension)

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `react` / `react-dom` | `^19` | Latest `19.2.6`. |
| `@tanstack/react-router` | `^1` | Type‑safe routing. |
| `@tanstack/react-query` | `^5` | Server‑state management. |
| `tailwindcss` | `^4` | Latest `4.3.0`. |
| `@tailwindcss/vite` | `^4` | Vite plugin for Tailwind v4. |
| `@tailwindcss/typography` | `^0.5` | Markdown styling. |
| `@tailwindcss/forms` | `^0.5` | Form resets. |
| `base-ui` | `^1` | Unstyled headless components. Latest `1.4.0`. |
| `lucide-react` | `^0` | Icon set. |
| `motion` | `^12` | React animation library. Latest `12.34.2`. |
| `@tanstack/react-table` | `^8` | Headless table. |
| `@tanstack/react-virtual` | `^3` | Virtualised lists. |
| `react-hook-form` | `^7` | Form handling. |
| `zod` | `^4` | Schema validation. Latest `4.4.2`. |
| `sonner` | `^2` | Toast notifications. |
| `vaul` | `^1` | Drawer component. |
| `embla-carousel-react` | `^8` | Carousel library. |
| `cmdk` | `^1` | Command palette. |
| `zustand` | `^5` | Normalised store for `DecryptedOverview` map. |
| `immer` | `^10` | Immutable updates for Zustand (optional). |
| `ts-pattern` | `^5` | Exhaustive pattern matching for `VaultStateUpdate`. |

### 2.2 Web SPA – Build & PWA

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `vite` | `^8` | **Vite 8 stable** with Rolldown. |
| `vite-plugin-pwa` | `^1` | PWA manifest and service worker. |

### 2.3 Browser Extension – Build

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `@crxjs/vite-plugin` | `^2` | Vite plugin for Chrome extensions. Works with Vite 8. |
| `webextension-polyfill` | `^0.12` | Cross‑browser extension API wrapper. |

---

## 3. Mobile Clients (React Native + Expo)

### 3.1 Expo & Core

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `expo` | `57` | **Expo SDK 57** (RN 0.86, React 19). |
| `expo-secure-store` | `57` | Keychain / Keystore for SVK. |
| `expo-local-authentication` | `16` | Biometric authentication. |
| `expo-haptics` | `57` | Haptic feedback. |
| `expo-image` | `1` | High‑performance image component. |
| `expo-build-properties` | `0` | Configures iOS deployment target (Sentry RN 8 requires iOS 15+). |
| `expo-dev-client` | `3` | Development client for native overlay testing. |
| `expo-file-system` | `55` | File operations for attachments. |

### 3.2 UI & Styling

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `react-native-reusables` | `^0.7` | shadcn/ui for React Native. |
| `nativewind` | `^5` | **NativeWind v5** – Tailwind CSS v4 for RN. |

### 3.3 Animations, Gestures & Navigation

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `react-native-reanimated` | `^4` | 60fps animations, Fabric compatible. |
| `react-native-gesture-handler` | `^2` | Native touch handling. |
| `react-native-screens` | `^4` | Native navigation. Latest `4.25.1`. |
| `react-native-safe-area-context` | `^5` | Safe area insets. |
| `react-native-bottom-sheet` | `^4` | Bottom sheet for conflict modal. |

### 3.4 Storage & Components

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `react-native-mmkv` | `^4` | High‑performance KV store. Latest `4.3.1`. |
| `react-native-svg` | `^15` | SVG rendering, Fabric compatible. |

### 3.5 Crash Reporting

| Package | Version Constraint | Notes |
| :--- | :--- | :--- |
| `@sentry/react-native` | `^8` | Version 8 requires iOS 15+ and RN 0.85+. |

---

## 4. Desktop Client (GPUI)

| Crate | Version Constraint | Notes |
| :--- | :--- | :--- |
| `gpui` | `git` master (Zed) | `{ git = "https://github.com/zed-industries/zed" }` — the `gpui` crate under `crates/gpui` of Zed's monorepo. |
| `gpui_platform` | `git` master (Zed) | `{ git = "https://github.com/zed-industries/zed", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }` — the application entry (`gpui_platform::application()`). |
| `gpui-component` | `git` master (Longbridge) | `{ git = "https://github.com/longbridge/gpui-component" }` — cross‑platform component library. Master already pulls `gpui` from `zed-industries/zed`, so it unifies with our `gpui` to one crate automatically. |
| `kael` | `0.1` | Advanced features (webviews, tray, blur). Optional. |
| `gpui-animation` | `0.2` | Lightweight state‑driven transitions. |
| `gpui-transitions` | `0.1` | Interpolation‑based transitions. |

**Excluded:** `fluent-gpui`, `gpui-rsx`, `adabraka-ui`.

**GPUI Source Policy:**
The desktop manifest depends on the **master branches** of the upstream repositories — `gpui`
from `zed-industries/zed` and `gpui-component` from `longbridge/gpui-component` — as git
dependencies, and compiles against those updated versions. This is a deliberate product decision:
we track the actively developed head rather than the frozen crates.io pins. `gpui-component`
master is versioned `0.5.x`, requires Rust `edition 2024`, and uses the current API
(`impl Render for X { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement }`,
`gpui_component::init(cx)`, `gpui_component::Root::new(view, window, cx)`,
`gpui_platform::application()`). Because `gpui-component` master already depends on `gpui` from
`zed-industries/zed`, both resolve to the same crate — no `[patch]` is required.

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed" }
gpui_platform = { git = "https://github.com/zed-industries/zed", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
```

Rebuild from these git sources on a release cadence (or whenever a breaking change lands upstream),
and keep `gpui` / `gpui_platform` / `gpui-component` versions in lockstep. The desktop UI MUST use
`gpui-component` widgets (Button, Input, List, Form, etc.) for its primary interface — not only raw
`gpui` divs.

---

## 5. Testing & Verification

| Tool / Crate | Version Constraint | Purpose |
| :--- | :--- | :--- |
| `loom` | `0.7` | Concurrency model testing for `sync_epoch` and DashMap. |
| `proptest` | `1.5` | Property‑based testing for crypto invariants. |
| `cargo-fuzz` | `0.12` | Nightly CI fuzz targets for AEAD parsing. |
| `wiremock` | `0.6` | Mock HTTP server for OCC and 412 tests. |
| `mockall` | `0.13` | Mocking PersistenceWorker and SQLite. |
| `rstest` | `0.24` | Fixture‑based testing for DB and crypto. |
| `trybuild` | `1` | Compile‑fail tests for restricted API enforcement. |
| `@testing-library/react` | `^16` | Web component testing. |
| `@testing-library/react-native` | `^13` | Mobile component testing. |
| `detox` | `^20` | E2E tests for mobile (React Native). |
| `maestro` | latest | Mobile UI automation (black‑box testing). |

---

## 6. Build & Quality Tools

| Tool | Version Constraint | Notes |
| :--- | :--- | :--- |
| `@biomejs/biome` | `^2` | Latest `2.4.7`. |
| `cargo-audit` | `0.22` | Vulnerability scanning. |
| `cargo-deny` | `0.19` | License / duplicate checks. |
| `cargo-tarpaulin` | `0.35` | Code coverage. |
| `pre-commit` | `^4` | Git hooks. |
| `renovate` / `dependabot` | — | Automated dependency updates. |
| **GitHub Actions** | — | CI/CD, matrix builds, AI security review. |
| **Docker** | `^29` | Multi‑arch images. |
| `storybook` | `^10` | UI component workshop. |

---

## 7. Dependency Update Cadence

- **Every two weeks** – Automated dependency updates (non‑breaking changes auto‑merged).
- **Critical CVEs** – Patched within 24 hours.
- **GPUI git revision** – Updated every two weeks.

---

## 8. Summary Table by Layer

| Layer | Key Dependencies |
| :--- | :--- |
| **Core Crypto & PKI** | `chacha20poly1305 0.11`, `argon2 0.6`, `x25519-dalek 2`, `ed25519-dalek 2`, `bip39 2`, `zeroize 1.8` |
| **Core DB** | `sea-orm 2.0`, `sqlx 0.9` + SQLite pragmas |
| **Core Net** | `reqwest 0.13`, `axum 0.8`, `tower 0.5` |
| **Import & File Storage** | `zip 2`, `csv 1`, `rayon 1`, `fs4 0.12`, `tempfile 3` |
| **Config & CLI** | `dotenvy 0.15`, `clap 4`, `figment 0.10` |
| **Observability** | `tracing 0.1`, `opentelemetry 0.28`, `metrics 0.24`, `sentry 0.47` |
| **Bindings** | `uniffi 0.31`, `wasm-bindgen 0.2` |
| **Web/Extension** | `vite 8`, `react 19`, `tailwindcss 4`, `zustand 5`, `ts-pattern 5`, `base-ui 1` |
| **Mobile** | `expo 57`, `nativewind 5`, `react-native-reanimated 4`, `expo-secure-store 57`, `@sentry/react-native 8` |
| **Desktop** | `gpui` (git hash), `gpui-component 0.5` |
| **Testing** | `loom 0.7`, `proptest 1.5`, `wiremock 0.6`, `mockall 0.13`, `rstest 0.24` |
| **Quality** | `@biomejs/biome 2`, `cargo-audit 0.22`, `pre-commit 4` |

---

This stack is now fully comprehensive and ready for implementation. Let me know if you need any adjustments.
