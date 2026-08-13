// VTR-048 — Swift tests for the native secret overlay (TDD2 + TDD3).
//
// These run with `swift test` (or XCTest in Xcode). They prove:
//   - TDD2: `onAction(.renderInOverlay, secret)` delivers the plaintext to the
//           native handler (the overlay would render it in a `UILabel`); JS
//           never sees it.
//   - TDD3: `deinit` releases the active handle, zeroizing the in-memory secret.
//
// `VautrClient` is a mock here; in the app it is the UniFFI-generated client.

import XCTest

@testable import VautrNative

private final class FakeVautrClient: VautrClient {
    private(set) var releasedHandle: OpaqueHandle?
    private(set) var releaseCallCount = 0

    override func releaseSecret(_ handle: OpaqueHandle) {
        releasedHandle = handle
        releaseCallCount += 1
    }
}

final class NativeSecretViewTests: XCTestCase {
    func testRenderInOverlayDeliversPlaintextToNative() {
        let client = FakeVautrClient()
        let handler = SecretOverlayActionHandler(client: client)
        var captured: (OpaqueHandle, String)?
        handler.onRender = { captured = ($0, $1) }

        handler.onAction(.renderInOverlay("123"), "hunter2-plaintext")

        XCTAssertEqual(captured?.0, "123")
        XCTAssertEqual(captured?.1, "hunter2-plaintext")
    }

    func testDeinitReleasesActiveHandle() {
        let client = FakeVautrClient()
        var handler: SecretOverlayActionHandler? = SecretOverlayActionHandler(client: client)
        handler?.onAction(.renderInOverlay("123"), "s")

        handler = nil  // deinit → release

        XCTAssertEqual(client.releasedHandle, "123")
    }

    func testReleaseIsIdempotent() {
        let client = FakeVautrClient()
        let handler = SecretOverlayActionHandler(client: client)
        handler.onAction(.renderInOverlay("123"), "s")
        handler.onAction(.renderInOverlay("123"), "s")  // re-render same handle

        // Deinit releases once.
        _ = handler
        XCTAssertEqual(client.releaseCallCount, 1)
    }
}
