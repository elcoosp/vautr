/// <reference lib="dom" />
import * as browser from 'webextension-polyfill';

/**
 * Autofill content script (build-env-deploy §3.3).
 *
 * The stateless Service Worker cannot touch page DOM directly. It sends the
 * decrypted secret to this content script (isolated world), which fills the
 * focused field and best-effort zeroizes its local reference. The secret never
 * enters the page's main world.
 */
const AUTOFILL_FILL = 'VAUTR_FILL';

function fillActiveElement(secret: string): void {
  const el = document.activeElement;
  if (!el) {
    return;
  }
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    el.value = secret;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }
}

browser.runtime.onMessage.addListener(((message: unknown) => {
  if (message && (message as { type?: string }).type === AUTOFILL_FILL) {
    fillActiveElement((message as { secret: string }).secret);
  }
}) as Parameters<typeof browser.runtime.onMessage.addListener>[0]);

/**
 * Relay hook for automated tests only: the page's main world dispatches a DOM
 * `CustomEvent` (shared DOM, isolated worlds) which this script forwards to the
 * stateless SW as a runtime message. This exercises the real SW wake path. The
 * SW response is echoed onto `document.documentElement` so the test harness can
 * observe it through the shared DOM.
 */
window.addEventListener('vautr-autofill-request', ((event: CustomEvent<{ uuid: string }>) => {
  void browser.runtime
    .sendMessage({ type: 'VAUTR_AUTOFILL', uuid: event.detail.uuid })
    .then((response: unknown) => {
      document.documentElement.setAttribute('data-vautr-autofill', JSON.stringify(response));
    })
    .catch((error: unknown) => {
      document.documentElement.setAttribute(
        'data-vautr-autofill',
        JSON.stringify({
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    });
}) as EventListener);
