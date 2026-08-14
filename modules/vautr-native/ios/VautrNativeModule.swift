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
                    // Copy `secret` to the system pasteboard. JS never sees it.
                    UIPasteboard.general.string = secret
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

        AsyncFunction("setSecureEnclaveBridge") {
            self.client?.setSecureEnclaveBridge(bridge: SecureEnclaveBridgeImpl())
        }
    }

    /// Real Keychain-backed SVK persistence (replaces the prior in-memory
    /// `svkBase64` placeholder). The SVK arrives as raw `Data` from the Rust
    /// core via `saveSvk` — JS never sees it as a string. Stored with
    /// `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly` so it is tied to the
    /// device passcode and never leaves the device or enters an iCloud backup.
    private static let svkKeychainKey = "vautr.svk"

    private class SecureEnclaveBridgeImpl: SecureEnclaveBridge {
        func saveSvk(svk: Data) {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: SecureEnclaveBridgeImpl.svkKeychainKey,
                kSecValueData as String: svk,
                kSecAttrAccessible as String: kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
            ]
            SecItemDelete(query as CFDictionary) // clear any prior value
            let status = SecItemAdd(query as CFDictionary, nil)
            if status != errSecSuccess {
                NSLog("VautrNative: saveSvk failed with status \(status)")
            }
        }

        func loadSvk() -> Data? {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: SecureEnclaveBridgeImpl.svkKeychainKey,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne,
            ]
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            guard status == errSecSuccess, let data = item as? Data else {
                return nil
            }
            return data
        }

        func deleteSvk() {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: SecureEnclaveBridgeImpl.svkKeychainKey,
            ]
            SecItemDelete(query as CFDictionary)
        }

        func hasSvk() -> Bool {
            loadSvk() != nil
        }
    }
}
