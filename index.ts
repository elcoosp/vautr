// Expo dev-server entry shim for this pnpm monorepo.
//
// Expo's Metro resolves the dev-server entry `./index` from the *workspace*
// root (pnpm-workspace.yaml detection), not from apps/mobile, so the real
// mobile entry (apps/mobile/index) cannot be found. This re-export redirects
// `./index` at the workspace root to the actual app entry. Vite (web) and the
// Rust workspace are unaffected — this file is only consumed by the Expo
// Metro dev server.
export * from './apps/mobile/index';
