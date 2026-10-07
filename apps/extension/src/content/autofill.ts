/// <reference lib="dom" />
import * as browser from 'webextension-polyfill';

/**
 * Autofill content script (build-env-deploy §3.3).
 *
 * VTRFIX-SEC-C05: this script NO LONGER accepts page-dispatched DOM events.
 * A previous version listened for a `vautr-autofill-request` CustomEvent on
 * the shared `window`, letting ANY website on the page trigger a decryption
 * of any vault item by UUID. The relay is now scoped to a dev-only companion
 * script (`autofill-test.ts`), and the service worker additionally verifies
 * that autofill requests originate from the popup (not from content scripts).
 *
 * The only action this script performs is filling the currently-focused
 * input when the SW explicitly asks it to (`VAUTR_FILL`). The secret is
 * passed directly from the SW (isolated world) and never touches the page's
 * main world.
 */
const AUTOFILL_FILL = 'VAUTR_FILL';

function fillActiveElement(secret: string): void {
  const el = document.activeElement;
  if (!el) {
    return;
  }
  // VTRFIX-SEC-M22: refuse hidden, disabled, readonly, or off-screen targets.
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    if (el.disabled || el.readOnly) return;
    if (el.offsetParent === null) return; // not visible
    const t = (el as HTMLInputElement).type;
    if (t && !['text', 'password', 'email', 'tel', 'url', 'search'].includes(t)) return;
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
