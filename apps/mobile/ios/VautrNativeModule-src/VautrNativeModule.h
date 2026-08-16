// Umbrella header for the VautrNativeModule pod.
//
// The Swift sources (`VautrNativeModule.swift`, `vautr_ffi/vautr_ffi.swift`)
// consume the uniffi C bridge via `import vautr_ffiFFI` (declared by
// `vautr_ffi/vautr_ffiFFI.modulemap`, which is on SWIFT_INCLUDE_PATHS) and the
// generated Swift API via `import vautr_ffi`. The app target imports this pod
// through the generated ExpoModulesProvider, which needs the emitted
// `VautrNativeModule-Swift.h`. Nothing needs to be imported here.
