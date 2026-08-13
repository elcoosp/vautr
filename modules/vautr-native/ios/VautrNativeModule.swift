import ExpoModulesCore
import Foundation
import uniffi_vautr_ffi

/**
 * Expo TurboModule bridging the Vautr uniffi core on iOS (VTR-061).
 *
 * Mirrors `VautrNativeBridge` (packages/vautr-client-sdk/src/mobile.ts). The
 * Rust `MobileClient` is held here; secrets never reach JS as strings —
 * `renderInOverlay`/`performAction` deliver plaintext only to the native
 * `PlatformActionHandler`.
 */
public class VautrNativeModule: Module {
    private var client: MobileClient?

    private let platformHandler = PlatformActionHandlerImpl()

    public func definition() -> ModuleDefinition {
        Name("VautrNative")

        // Native overlay handler: receives the plaintext secret for copy /
        // autofill / overlay render. JS never sees it.
        class PlatformActionHandlerImpl: PlatformActionHandler {
            func onAction(action: CoreAction, secret: String) {
                switch action {
                case .renderInOverlay:
                    // Route to the native overlay UI (released on unmount).
                    break
                case .copyToClipboard:
                    // Copy `secret` to the system pasteboard.
                    break
                case .autofill:
                    // Hand `secret` to the autofill service.
                    break
                }
            }
        }

        AsyncFunction("initialize") { (dbPath: String) in
            self.client = try await MobileClient.initialize(dbPath: dbPath)
            self.client?.setPlatformHandler(handler: self.platformHandler)
        }

        AsyncFunction("unlock") { (rawKey: Data, localGen: Double) in
            try await self.client?.unlock(rawKey: rawKey, localGen: UInt64(localGen))
        }

        AsyncFunction("listOverviews") { () -> String in
            self.client?.listOverviews() ?? "[]"
        }

        AsyncFunction("revealSecret") { (uuid: String) -> String in
            // Opaque handle (u64) surfaced to JS as a String.
            let h = try await self.client?.revealSecret(uuid: uuid) ?? 0
            return String(h)
        }

        AsyncFunction("releaseSecret") { (handle: Double) in
            self.client?.releaseSecret(handle: UInt64(handle))
        }

        AsyncFunction("renderInOverlay") { (handle: Double) in
            try await self.client?.renderSecretInOverlay(handle: UInt64(handle))
        }

        AsyncFunction("lock") {
            try await self.client?.lock()
        }

        AsyncFunction("sync") {
            try await self.client?.sync()
        }

        AsyncFunction("setSecureEnclaveBridge") { (svkBase64: String?) in
            self.client?.setSecureEnclaveBridge(bridge: SecureEnclaveBridgeImpl(svkBase64: svkBase64))
        }
    }

    private class SecureEnclaveBridgeImpl: SecureEnclaveBridge {
        private let svkBase64: String?
        init(svkBase64: String?) { self.svkBase64 = svkBase64 }
        func saveSvk(svk: Data) {}
        func loadSvk() -> Data? {
            guard let b64 = svkBase64, let data = Data(base64Encoded: b64) else { return nil }
            return data
        }
        func deleteSvk() {}
        func hasSvk() -> Bool { svkBase64 != nil }
    }
}
