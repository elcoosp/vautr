/// <reference lib="dom" />
import * as browser from 'webextension-polyfill';

/**
 * Autofill content script (build-env-deploy §3.3).
 *
 * VTRFIX-SEC-C05: this script NO LONGER accepts page-dispatched DOM events.
 * VTRFIX-FEAT-M05: it now passively detects login forms and offers a
 * shadow-DOM badge. Clicking the badge asks the popup for candidate items
 * that match the current origin; the popup handles the actual fill via
 * VAUTR_FILL (already hardened in SEC-C05/M22).
 *
 * The only action this script performs is: (a) badge visibility on password
 * focus, (b) filling the focused input when the SW tells it to (`VAUTR_FILL`).
 * The secret never enters the page's main world.
 */
const AUTOFILL_FILL = 'VAUTR_FILL';

const BADGE_ID = 'vautr-autofill-badge-root';

/** VTRFIX-FEAT-M05: find a visible password input and its nearby username. */
function findLoginForms(): {
  username: HTMLInputElement | null;
  password: HTMLInputElement;
}[] {
  const out: { username: HTMLInputElement | null; password: HTMLInputElement }[] = [];
  for (const el of Array.from(document.querySelectorAll('input[type=password]'))) {
    if (!(el instanceof HTMLInputElement)) continue;
    if (el.disabled || el.readOnly) continue;
    if (el.offsetParent === null) continue;
    // A username field is the closest prior input of a plausible type.
    let sibling: Element | null = el.previousElementSibling;
    let username: HTMLInputElement | null = null;
    while (sibling && !username) {
      if (sibling instanceof HTMLInputElement) {
        const t = sibling.type;
        if (
          ['text', 'email', 'tel', 'url', ''].includes(t) &&
          !sibling.disabled &&
          !sibling.readOnly &&
          sibling.offsetParent !== null
        ) {
          username = sibling;
        }
      }
      sibling = sibling.previousElementSibling;
    }
    out.push({ username, password: el });
  }
  return out;
}

function fillActiveElement(secret: string): void {
  const el = document.activeElement;
  if (!el) {
    return;
  }
  // VTRFIX-SEC-M22: refuse hidden, disabled, readonly, or off-screen targets.
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    if (el.disabled || el.readOnly) return;
    if (el.offsetParent === null) return;
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

// ---------------------------------------------------------------------------
// VTRFIX-FEAT-M05: passive badge
// ---------------------------------------------------------------------------

/** Mount a single shadow-DOM host for the badge. */
function mountBadgeHost(): ShadowRoot | null {
  if (document.getElementById(BADGE_ID)) {
    return (document.getElementById(BADGE_ID) as HTMLElement).shadowRoot;
  }
  const host = document.createElement('div');
  host.id = BADGE_ID;
  host.style.position = 'fixed';
  host.style.zIndex = '2147483647';
  host.style.top = '0';
  host.style.left = '0';
  host.style.pointerEvents = 'none';
  document.documentElement.appendChild(host);
  return host.attachShadow({ mode: 'closed' });
}

function showBadge(password: HTMLInputElement): void {
  const root = mountBadgeHost();
  if (!root) return;
  root.innerHTML = '';
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.textContent = 'Vautr';
  btn.style.cssText = [
    'position: fixed',
    'padding: 4px 8px',
    'font: 12px system-ui',
    'background: #111',
    'color: #fff',
    'border: 0',
    'border-radius: 4px',
    'cursor: pointer',
    'pointer-events: auto',
    'box-shadow: 0 2px 6px rgba(0,0,0,.25)',
  ].join(';');

  const rect = password.getBoundingClientRect();
  btn.style.top = `${rect.top + window.scrollY + 4}px`;
  btn.style.left = `${rect.right + window.scrollX + 6}px`;

  btn.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    // Ask the popup (via the SW) to open the picker for this origin. The
    // SW replies only with the candidate list; the fill itself stays in
    // the popup's own trusted context (SEC-C05).
    void browser.runtime.sendMessage({
      type: 'VAUTR_MATCH_REQUEST',
      origin: location.origin,
    });
    hideBadge();
  });

  root.appendChild(btn);
}

function hideBadge(): void {
  const root = document.getElementById(BADGE_ID);
  if (root?.shadowRoot) {
    root.shadowRoot.innerHTML = '';
  }
}

document.addEventListener(
  'focusin',
  (e) => {
    const t = e.target;
    if (!(t instanceof HTMLInputElement)) return;
    if (t.type !== 'password') return;
    if (t.disabled || t.readOnly || t.offsetParent === null) return;
    const forms = findLoginForms();
    if (!forms.some((f) => f.password === t)) return;
    showBadge(t);
  },
  true,
);

document.addEventListener(
  'focusout',
  () => {
    // Give the click handler a tick to fire before tearing the badge down.
    window.setTimeout(() => hideBadge(), 150);
  },
  true,
);

document.addEventListener('scroll', () => hideBadge(), true);
