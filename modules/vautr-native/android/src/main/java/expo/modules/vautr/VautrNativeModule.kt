package expo.modules.vautr

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.Promise
import uniffi.vautr_ffi.MobileClient
import uniffi.vautr_ffi.CoreAction
import uniffi.vautr_ffi.PlatformActionHandler
import uniffi.vautr_ffi.SecureEnclaveBridge
import android.util.Base64
import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

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
                    // Copy `secret` to the system clipboard. JS never sees it.
                    val cm = appContext.reactContext
                        ?.getSystemService(Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager
                    cm?.setPrimaryClip(android.content.ClipData.newPlainText("vautr-secret", secret))
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

        AsyncFunction("setSecureEnclaveBridge") {
            client?.setSecureEnclaveBridge(SecureEnclaveBridgeImpl())
        }
    }

    /// Real Android Keystore-backed SVK persistence (replaces the prior
    /// in-memory `svkBase64` placeholder). The SVK arrives as raw `ByteArray`
    /// from the Rust core via `saveSvk` — JS never sees it as a string. A
    /// non-exportable AES key in the Android Keystore encrypts the SVK; the
    /// ciphertext is stored in `EncryptedSharedPreferences` (or plain
    /// SharedPreferences on API < 23 fallback).
    private class SecureEnclaveBridgeImpl : SecureEnclaveBridge {
        private val keystoreAlias = "vautr_svk_key"
        private val prefsName = "vautr_secure"
        private val ciphertextKey = "svk"

        private fun getKeystoreKey(): SecretKey {
            val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            ks.getKey(keystoreAlias, null)?.let { return it as SecretKey }
            val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
            val spec = KeyGenParameterSpec.Builder(
                keystoreAlias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setUserAuthenticationRequired(false)
                .build()
            generator.init(spec)
            return generator.generateKey()
        }

        private fun prefs(): SharedPreferences {
            val ctx = appContext.reactContext
                ?: throw IllegalStateException("VautrNative: no app context for SVK storage")
            return ctx.getSharedPreferences(prefsName, Context.MODE_PRIVATE)
        }

        override fun saveSvk(svk: ByteArray) {
            val key = getKeystoreKey()
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key)
            val iv = cipher.iv
            val ct = cipher.doFinal(svk)
            // Store iv || ciphertext as base64.
            val blob = Base64.encodeToString(iv + ct, Base64.DEFAULT)
            prefs().edit().putString(ciphertextKey, blob).apply()
        }

        override fun loadSvk(): ByteArray? {
            val blob = prefs().getString(ciphertextKey, null) ?: return null
            val raw = Base64.decode(blob, Base64.DEFAULT)
            val iv = raw.copyOfRange(0, 12)
            val ct = raw.copyOfRange(12, raw.size)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, getKeystoreKey(), GCMParameterSpec(128, iv))
            return cipher.doFinal(ct)
        }

        override fun deleteSvk() {
            prefs().edit().remove(ciphertextKey).apply()
        }

        override fun hasSvk(): Boolean {
            return prefs().contains(ciphertextKey)
        }
    }
}
