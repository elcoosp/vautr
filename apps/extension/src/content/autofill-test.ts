/// <reference lib="dom" />
import * as browser from 'webextension-polyfill';

/**
 * VTRFIX-SEC-C05: DEV-ONLY autofill relay.
 *
 * This file is included in the manifest ONLY when `NODE_ENV === 'development'`.
 * It restores the page-DOM test hook that the previous production build
 * shipped by mistake: `window.dispatchEvent(new CustomEvent('vautr-autofill-request', ...))`
 * and a `data-vautr-autofill` attribute echo. NEVER ship this to users — it
 * lets any page on the internet ask the SW to decrypt any vault item.
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
