require 'json'

# @vautr/native iOS pod. The uniffi-generated Swift bindings
# (`vautr_ffi.swift`, `vautr_ffiFFI.h`, `vautr_ffiFFI.modulemap`) and the
# prebuilt static lib (`libvautr_ffi.a`) live directly in this dir (a real,
# non-symlinked copy of packages/native/ios). They are produced by:
#   cargo build -p vautr-ffi --target aarch64-apple-ios-sim   # -> libvautr_ffi.a
#   cargo run -p vautr-ffi --example gen_bindings <dir>       # -> vautr_ffi.swift + header
# `libvautr_ffi.a` is a large build artifact and is gitignored; re-run the
# cargo build above to regenerate it before `pod install` after a clean clone.
#
# NOTE: this copy lives outside node_modules on purpose. CocoaPods only emits a
# pod's clang modulemap from REAL header files; pnpm symlinks packages under
# node_modules, so the modulemap generator skips them and the app's generated
# ExpoModulesProvider cannot `import VautrNativeModule`. Sourcing from this real
# dir (via `pod 'VautrNativeModule', :path => '../VautrNativeModule-src'` in the
# Podfile) makes CocoaPods copy the headers and emit the module.

Pod::Spec.new do |s|
  s.name         = 'VautrNativeModule'
  s.version      = '0.1.0'
  s.summary      = 'Vautr uniffi core bridge for React Native'
  s.homepage     = 'https://github.com/vautr/vautr'
  s.license      = { :type => 'AGPL-3.0' }
  s.authors      = 'Vautr'
  s.platforms    = { :ios => '15.0' }
  s.source       = { :git => '', :tag => '0.1.0' }
  s.source_files = '*.swift', 'vautr_ffi/*.swift'
  s.swift_version = '5.9'
  s.static_framework = true
  s.dependency 'ExpoModulesCore'

  # Generated uniffi Swift bindings + C header + modulemap (relative paths).
  s.preserve_paths = [
    'vautr_ffi/vautr_ffi.swift',
    'vautr_ffi/vautr_ffiFFI.h',
    'vautr_ffi/vautr_ffiFFI.modulemap',
    'libvautr_ffi.a',
  ]
  # Umbrella header + DEFINES_MODULE let CocoaPods auto-emit the
  # `VautrNativeModule` clang module (mirrors how published Expo Swift modules
  # like ExpoSecureStore expose their module) so the app's ExpoModulesProvider
  # can `import VautrNativeModule`.
  s.public_header_files = ['VautrNativeModule.h', 'vautr_ffi/vautr_ffiFFI.h']
  s.header_dir = 'VautrNativeModule'

  s.vendored_libraries = ['libvautr_ffi.a']
  s.library = 'vautr_ffi'
  # PODS_TARGET_SRCROOT points at VautrNativeModule-src (this podspec's dir).
  # The static lib + generated module live directly under it.
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'OTHER_LDFLAGS' => '-L${PODS_TARGET_SRCROOT} -lvautr_ffi',
    'SWIFT_INCLUDE_PATHS' => '${PODS_TARGET_SRCROOT}/vautr_ffi',
    'CLANG_ENABLE_MODULES' => 'YES',
    'OTHER_SWIFT_FLAGS' => '-Xcc -fmodule-map-file="${PODS_TARGET_SRCROOT}/vautr_ffi/vautr_ffiFFI.modulemap"',
  }
  s.user_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '$(inherited) ${PODS_CONFIGURATION_BUILD_DIR}/VautrNativeModule ${PODS_TARGET_SRCROOT}/vautr_ffi',
  }
end
