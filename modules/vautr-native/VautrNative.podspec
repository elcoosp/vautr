require 'json'

package = JSON.parse(File.read(File.join(__dir__, '..', 'package.json')))

Pod::Spec.new do |s|
  s.name         = 'VautrNative'
  s.version      = package['version']
  s.summary      = 'Vautr uniffi core native bridge'
  s.description  = 'Expo TurboModule linking the Vautr Rust vault core (VTR-061).'
  s.author       = 'Vautr'
  s.homepage     = 'https://github.com/vautr/vautr'
  s.license      = 'AGPL-3.0'
  s.platforms    = { :ios => '15.0' }
  s.source       = { :git => '' }
  s.source_files = 'ios/**/*.{h,m,mm,swift}'
  s.vendored_libraries = 'ios/libs/libvautr_ffi.a'
  # The Rust static lib is produced by `cargo build -p vautr-ffi` and copied to
  # ios/libs/ (see docs/modules/vautr-native/README.md).
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_INCLUDE_PATHS' => '"${PODS_ROOT}/../../modules/vautr-native/ffi-bindings"'
  }
  s.dependency 'ExpoModulesCore'
end
