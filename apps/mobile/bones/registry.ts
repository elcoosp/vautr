// boneyard bone registry (VTR: loading-skeleton UI via github.com/0xGF/boneyard).
//
// For React Native, bones are captured at runtime by `boneyard-js/native`'s
// <Skeleton> when the boneyard CLI is running (`npx boneyard-js build --native`)
// against a device/simulator. Until captured, this is an intentional no-op
// placeholder so the `import './bones/registry'` in index.ts resolves and
// <Skeleton> gracefully falls back to its `fallback` prop (the original
// <ActivityIndicator> / "Loading…" node) when no bones are available.
export {};
