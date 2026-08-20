import { type BrowserContext, type Page, expect, test } from '@playwright/test';
import { DatabaseSync } from 'node:sqlite';

/**
 * Web Shares e2e (VTR-104 / ADR-007): a zero-knowledge 1:1 item share, driven
 * entirely through the real web UI against the live backend.
 *
 *   1. Sender registers (real OPAQUE, in-browser wasm) + creates a secret.
 *   2. Sender opens the item, shares it to the recipient's user id.
 *   3. Recipient registers, opens Shares → Inbox, accepts & decrypts, and sees
 *      the original plaintext — proving the end-to-end encrypted handoff with
 *      the server only ever relaying wrapped blobs.
 *
 * The app talks to the backend via the same-origin `/api` path (vite proxies to
 * :8080), exactly like flow.spec.
 *
 * The recipient's user id (a UUID) is read from the running backend's DB file
 * after registration, since the UI only knows the recipient by email.
 *
 * NOTE: This spec currently hangs at the share step — `confirmShare` issues no
 * network request after the click (verified via request logging), i.e. the web
 * client stalls inside local share logic (`ensureSharingKey` / `getItemPlaintext`
 * / wasm `crypto.shareItem`) before reaching the server. The server-side share
 * flow is proven by `core/vautr-server/tests/sharing_e2e.rs`. This is a real
 * web-client defect (VTR-104), surfaced by this e2e, not a harness problem.
 */

const ORIGIN = 'http://localhost:5173';
const DB_PATH = '/tmp/vautr-webauthn-e2e.db';

async function registerViaUi(page: Page, username: string): Promise<void> {
  await page.goto(`${ORIGIN}/register`);
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Master password', { exact: true }).fill('correct-horse-battery-staple');
  await page.getByLabel('Confirm master password', { exact: true }).fill('correct-horse-battery-staple');
  await page.getByRole('button', { name: /create vault/i }).click();
  await page.waitForURL('**/dashboard', { timeout: 30_000 });
  await dismissOnboarding(page);
}

/** Dismiss the first-run onboarding modal if present (client-side, no reload). */
async function dismissOnboarding(page: Page): Promise<void> {
  const overlay = page.locator('div.fixed.inset-0');
  await page
    .getByText('Welcome to Vautr')
    .waitFor({ state: 'visible', timeout: 15_000 })
    .catch(() => {});
  for (let i = 0; i < 12; i++) {
    if (!(await overlay.first().isVisible().catch(() => false))) return;
    const target = overlay
      .getByRole('button')
      .filter({ hasText: /create vault & continue|continue|go to my vault|finish|skip|next/i })
      .first();
    const clicked = await target.click({ timeout: 3_000 }).then(() => true).catch(() => false);
    if (!clicked) return;
    await page.waitForTimeout(600);
  }
}

/** Read a user's UUID from the running backend DB by email. */
function userIdByEmail(email: string): string {
  const db = new DatabaseSync(DB_PATH);
  const row = db.prepare(`SELECT id FROM users WHERE email = ?`).get(email) as
    | { id?: string }
    | undefined;
  db.close();
  if (!row?.id) throw new Error(`[shares-e2e] no user found for ${email}`);
  return row.id;
}

async function createSecret(
  page: Page,
  title: string,
  username: string,
  password: string,
): Promise<void> {
  await page.getByRole('link', { name: 'Vault', exact: true }).first().click();
  await page.waitForURL('**/vault', { timeout: 15_000 });
  await page.getByRole('button', { name: /add item/i }).click();
  await page.getByLabel('Title').fill(title);
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Password').fill(password);
  await page.getByRole('button', { name: /save item/i }).click();
  // Save auto-opens ItemDetail.
  await expect(page.getByText(title).first()).toBeVisible({ timeout: 15_000 });
}

test('sender shares a secret; recipient accepts & decrypts it', async ({ browser }) => {
  const senderEmail = `shares-sender-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;
  const recipEmail = `shares-recip-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;

  // Register BOTH users first (real OPAQUE, in-browser wasm) so we can resolve
  // the recipient's UUID before the sender shares.
  const senderCtx: BrowserContext = await browser.newContext();
  const sender = await senderCtx.newPage();
  sender.on('console', (m) => console.log('SENDER_PAGE:', m.text()));
  sender.on('pageerror', (e) => console.log('SENDER_PAGEERR:', e.message));
  sender.on('response', (r) => {
    const u = r.url();
    if (u.includes('/api/')) console.log('SENDER_RESP:', r.status(), u.replace('http://localhost:5173', ''));
  });
  await registerViaUi(sender, senderEmail);

  const recipCtx: BrowserContext = await browser.newContext();
  const recip = await recipCtx.newPage();
  recip.on('console', (m) => console.log('RECIP_PAGE:', m.text()));
  recip.on('pageerror', (e) => console.log('RECIP_PAGEERR:', e.message));
  recip.on('response', async (r) => {
    const u = r.url();
    if (u.includes('/api/')) {
      const t = await r.text().catch(() => '');
      console.log('RECIP_RESP:', r.status(), u.replace('http://localhost:5173', ''), t.slice(0, 200));
    }
  });
  await registerViaUi(recip, recipEmail);

  const recipId = userIdByEmail(recipEmail);

  // --- Sender: create a secret, then share it to the recipient ---
  await createSecret(sender, 'Shared Login', 'shared-user@example.com', 's3cret-shared-pw-99!');

  const shareDialog = sender.getByRole('dialog', { name: 'Share Shared Login' });
  await sender.getByRole('button', { name: 'Share Shared Login', exact: true }).first().click();
  await expect(shareDialog).toBeVisible({ timeout: 10_000 });
  await shareDialog.getByPlaceholder('recipient username').fill(recipId);
  await shareDialog.getByRole('button', { name: 'Share', exact: true }).click();
  // Dialog closes on success.
  await expect(shareDialog).toBeHidden({ timeout: 30_000 });
  // The item is still visible in the detail view.
  await expect(sender.getByText('Shared Login').first()).toBeVisible({ timeout: 10_000 });

  // --- Recipient: accept & decrypt the pending share ---
  // Navigate via the SPA nav link (preserves the in-memory session) rather
  // than a full page.goto, which would drop the IndexedDB-restored token
  // before the auth guard settles.
  await recip.getByRole('link', { name: 'Shares', exact: true }).first().click();
  await recip.waitForURL('**/shares', { timeout: 15_000 });
  const accept = recip.getByRole('button', { name: 'Accept & decrypt' }).first();
  await expect(accept).toBeVisible({ timeout: 15_000 });
  await accept.click();
  // Decryption is local (wasm). The plaintext password becomes visible.
  await expect(recip.getByText('s3cret-shared-pw-99!').first()).toBeVisible({ timeout: 15_000 });
  // The row transitions to a "Decrypted" state.
  await expect(recip.getByText('Decrypted').first()).toBeVisible({ timeout: 10_000 });

  await senderCtx.close();
  await recipCtx.close();
});
