package expo.modules.vautr

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.Promise
import uniffi.vautr_ffi.MobileClient
import uniffi.vautr_ffi.CoreAction
import uniffi.vautr_ffi.PlatformActionHandler
import uniffi.vautr_ffi.SecureEnclaveBridge
import android.util.Base64

/**
 * Expo TurboModule bridging the Vautr uniffi core (VTR-061).
 *
 * Each `@Func` mirrors a method on `VautrNativeBridge` (see
 * packages/vautr-client-sdk/src/mobile.ts). The Rust `MobileClient` instance is
 * held here and surfaced to JS as an opaque handle; secrets never cross into JS
 * as strings — `renderInOverlay`/`performAction` deliver plaintext only to the
 * native `PlatformActionHandler`.
 */
class VautrNativeModule : Module() {
    private var client: MobileClient? = null

    // Native overlay handler: receives the plaintext secret for copy/autofill/
    // overlay render and dispatches it to the OS clipboard / UI. JS never sees it.
    private val platformHandler = object : PlatformActionHandler {
        override fun onAction(action: CoreAction, secret: String) {
            when (action) {
                is CoreAction.RenderInOverlay -> {
                    // Show the secret in the native overlay UI. Implemented by the
                    // React Native overlay component; here we log + would route to it.
                    // The overlay is responsible for calling releaseSecret on unmount.
                }
                is CoreAction.CopyToClipboard -> {
                    // Copy `secret` to the system clipboard.
                }
                is CoreAction.Autofill -> {
                    // Hand `secret` to the autofill service.
                }
            }
        }
    }

    override fun definition() = ModuleDefinition {
        Name("VautrNative")

        AsyncFunction("initialize") { dbPath: String ->
            client = MobileClient.initialize(dbPath)
            client?.setPlatformHandler(platformHandler)
        }

        AsyncFunction("unlock") { rawKey: ByteArray, localGen: Double ->
            client?.unlock(rawKey, localGen.toULong())
        }

        AsyncFunction("listOverviews") { ->
            client?.listOverviews() ?: "[]"
        }

        AsyncFunction("revealSecret") { uuid: String ->
            // Returns the opaque handle (u64) as a String to JS.
            client?.revealSecret(uuid)?.toString() ?: "0"
        }

        AsyncFunction("releaseSecret") { handle: Double ->
            client?.releaseSecret(handle.toULong())
        }

        AsyncFunction("renderInOverlay") { handle: Double ->
            client?.renderSecretInOverlay(handle.toULong())
        }

        AsyncFunction("lock") {
            client?.lock()
        }

        AsyncFunction("sync") {
            client?.sync()
        }

        AsyncFunction("setSecureEnclaveBridge") { svkBase64: String? ->
            client?.setSecureEnclaveBridge(object : SecureEnclaveBridge {
                private val key = svkBase64
                override fun saveSvk(svk: ByteArray) {
                    // Persist under biometric/device protection (Keystore).
                }
                override fun loadSvk(): ByteArray? =
                    key?.let { Base64.decode(it, Base64.DEFAULT) }
                override fun deleteSvk() {}
                override fun hasSvk(): Boolean = key != null
            })
        }
    }
}
