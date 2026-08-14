# Vautr dev task runner. Run `just` to list recipes.
# Common reused dev commands — mirrors hermes verify gates + the iOS native
# bridge build flow.

# Re-run the root watcher (wr.sh) on change.
wr:
    watchexec -w ./wr.sh --clear -r "./wr.sh"

# --- workspace-wide gates (mirrors `hermes verify`) ---

# Format + lint + typecheck + test across Rust + pnpm workspaces.
check: fmt lint typecheck test

# Rust: format check (cargo fmt --check on touched crates).
fmt:
    cargo fmt --all -- --check

# Rust: format in place.
fmt-fix:
    cargo fmt --all

# Rust: build all workspace crates.
cargo-build:
    cargo build --workspace

# Rust: run all workspace tests.
cargo-test:
    cargo test --workspace

# pnpm: lint (biome) across the workspace.
lint:
    pnpm biome check .

# pnpm: auto-fix lint.
lint-fix:
    pnpm biome check --write --unsafe .

# pnpm: typecheck all packages + apps.
typecheck:
    pnpm -r typecheck

# pnpm: run all package/app tests.
test:
    pnpm -r test

# pnpm: build all packages + apps.
build:
    pnpm -r build

# Full hermes verify (build, typecheck, test, lint, format, readiness).
verify:
    hermes verify --json

# --- uniffi FFI bindings (vautr-ffi -> Swift/Kotlin) ---

# Build vautr-ffi and regenerate the uniffi Swift/Kotlin bindings into
# modules/vautr-native/ffi-bindings/.
gen-ffi:
    cargo build -p vautr-ffi
    cargo run -p vautr-ffi --example gen_bindings modules/vautr-native/ffi-bindings

# Build the static lib for the iOS simulator (aarch64) — input to the pod.
cargo-ios-sim:
    cargo build -p vautr-ffi --target aarch64-apple-ios-sim

# --- iOS native bridge (VTR-070 native layer) ---
# Builds the static lib + bindings, copies the pod sources to a real (non-
# symlinked) dir (CocoaPods skips modulemap gen for pnpm-symlinked sources),
# then prebuilds + installs pods + builds the simulator app.

# Regenerate bindings + static lib, then refresh the real-path pod copy.
ios-setup: cargo-ios-sim gen-ffi
    rm -rf apps/mobile/VautrNativeModule-src
    cp -R packages/native/ios apps/mobile/VautrNativeModule-src

# Generate the Xcode project (idempotent; regenerates apps/mobile/ios).
ios-prebuild:
    cd apps/mobile && npx expo prebuild --platform ios --no-install

# Install pods (VautrNativeModule is wired via :path in the Podfile).
ios-pod-install:
    cd apps/mobile/ios && pod install

# Build the simulator app (no signing). Requires ios-setup first.
ios-build:
    cd apps/mobile/ios && xcodebuild \
        -workspace Vautr.xcworkspace -scheme Vautr \
        -configuration Debug -sdk iphonesimulator \
        -destination 'id=27088715-6C8B-436A-AC66-B2DA978A2944' \
        CODE_SIGN_IDENTITY="" CODE_SIGNING_ALLOWED=NO build

# Full iOS flow: setup -> prebuild -> pod install -> build.
ios: ios-setup ios-prebuild ios-pod-install ios-build

# --- Android native bridge (VTR-070 native layer) ---
# Builds the shared lib (.so) + Kotlin bindings, then prebuilds + assembles.

# Build the aarch64 .so via cargo-ndk into the module's jniLibs. Requires
# ANDROID_NDK_HOME (brew cask android-ndk: the NDK lives inside the .app bundle).
android-so:
    export ANDROID_NDK_HOME="/opt/homebrew/Caskroom/android-ndk/29/AndroidNDK14206865.app/Contents/NDK"
    cargo ndk -t arm64-v8a -o packages/native/android/src/main/jniLibs build -p vautr-ffi --release

# Copy the generated Kotlin bindings into the module's source set.
android-bindings:
    mkdir -p packages/native/android/src/main/java/uniffi/vautr_ffi
    cp -R modules/vautr-native/ffi-bindings/uniffi/vautr_ffi/. packages/native/android/src/main/java/uniffi/vautr_ffi/

# Generate the Gradle project (regenerates apps/mobile/android).
android-prebuild:
    cd apps/mobile && npx expo prebuild --platform android --no-install

# Assemble the debug APK (no signing). Requires android-so + android-bindings first.
android-build:
    cd apps/mobile/android && ./gradlew :app:assembleDebug --no-daemon

# Full Android flow.
android: android-so android-bindings android-prebuild android-build

# --- servers / utils ---

# Start the Docker dev server stack.
dev-server:
    ./scripts/dev-server.sh
