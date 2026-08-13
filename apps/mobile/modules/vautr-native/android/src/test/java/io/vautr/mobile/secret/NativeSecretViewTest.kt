/*
 * VTR-048 — Kotlin tests for the native secret overlay (TDD1 + TDD3).
 *
 * These run on the JVM unit-test runner (Robolectric or plain JUnit with a
 * `TextView` shadow). They prove:
 *   - TDD1: `onAction(RenderInOverlay, secret)` renders `secret` in the overlay
 *           `TextView` (plaintext reaches native, never JS).
 *   - TDD3: detaching the view calls `releaseSecret(handle)`, zeroizing the
 *           in-memory secret in Rust.
 *
 * `VautrClient` is a mock here; in the app it is the UniFFI-generated client.
 */

package io.vautr.mobile.secret

import io.vautr.mobile.ffi.CoreAction
import io.vautr.mobile.ffi.OpaqueHandle
import io.vautr.mobile.ffi.VautrClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

/** Records the handle passed to `releaseSecret` so tests can assert TDD3. */
private class FakeVautrClient : VautrClient() {
    var releasedHandle: OpaqueHandle? = null
        private set

    override fun releaseSecret(handle: OpaqueHandle) {
        releasedHandle = handle
    }
}

class NativeSecretViewTest {

    private lateinit var client: FakeVautrClient
    private lateinit var view: NativeSecretView
    private lateinit var handler: SecretOverlayActionHandler

    @Before
    fun setUp() {
        client = FakeVautrClient()
        view = NativeSecretView(null).apply { this.client = client }
        handler = SecretOverlayActionHandler(view)
    }

    /** TDD1: the plaintext secret is rendered into the overlay from native. */
    @Test
    fun rendersPlaintextInOverlay() {
        val handle: OpaqueHandle = "123"
        val secret = "hunter2-plaintext"

        handler.onAction(CoreAction.RenderInOverlay(handle), secret)

        assertEquals(secret, view.text.toString())
    }

    /** TDD3: detaching the overlay releases the active handle (zeroization). */
    @Test
    fun releasesSecretOnDetach() {
        val handle: OpaqueHandle = "123"
        handler.onAction(CoreAction.RenderInOverlay(handle), "hunter2-plaintext")

        view.onDetachedFromWindow()

        assertEquals(handle, client.releasedHandle)
        assertNull(view.text.toString().ifEmpty { null })
    }

    /** TDD3: releasing twice is safe (Rust rejects the already-released handle). */
    @Test
    fun releaseIsIdempotent() {
        val handle: OpaqueHandle = "123"
        handler.onAction(CoreAction.RenderInOverlay(handle), "s")
        view.releaseActiveSecret()
        client.releasedHandle = null
        view.releaseActiveSecret()

        assertNull(client.releasedHandle)
    }
}
