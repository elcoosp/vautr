import { expect, type Page, test } from '@playwright/test';

/**
 * Web happy-path flow e2e (the flagship SPA client).
 *
 * Drives the REAL register/login UI (in-browser OPAQUE wasm) and then the full
 * vault user flow against the live Vautr server on :8080:
 *
 *   register (OPAQUE) → first-run onboarding → open Vault → add a secret → it
 *   auto-opens in ItemDetail (which reveals on mount) → Copy yields the
 *   plaintext.
 *
 * No DB seeding or IndexedDB injection: the app mints and persists its own
 * session through the normal register flow, exactly as a real user would.
 *
 * Run with: `pnpm --filter @vautr/web exec playwright test --config
 * playwright.config.happy.ts`
 */

const ORIGIN = 'http://localhost:5173';
const PASSWORD = 'correct-horse-battery-staple-flow';

function randomUsername(): string {
  return `flow-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;
}

async function registerViaUi(page: Page, username: string): Promise<void> {
  await page.goto(`${ORIGIN}/register`);
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Master password', { exact: true }).fill(PASSWORD);
  await page.getByLabel('Confirm master password', { exact: true }).fill(PASSWORD);
  await page.getByRole('button', { name: /create vault/i }).click();
  // Register auto-logs-in and navigates to /dashboard.
  try {
    await page.waitForURL('**/dashboard', { timeout: 30_000 });
  } catch {
    const url = page.url();
    const alert = await page
      .getByRole('alert')
      .first()
      .textContent()
      .catch(() => null);
    throw new Error(`did not reach /dashboard; url=${url} alert=${alert}`);
  }
}

/** Drive the first-run onboarding modal to completion (client-side, no reload). */
async function dismissOnboarding(page: Page): Promise<void> {
  const overlay = page.locator('div.fixed.inset-0');
  await page
    .getByText('Welcome to Vautr')
    .waitFor({ state: 'visible', timeout: 15_000 })
    .catch(() => {});
  for (let i = 0; i < 12; i++) {
    if (
      !(await overlay
        .first()
        .isVisible()
        .catch(() => false))
    )
      return;
    const target = overlay
      .getByRole('button')
      .filter({ hasText: /create vault & continue|continue|go to my vault|finish|skip|next/i })
      .first();
    const clicked = await target
      .click({ timeout: 3_000 })
      .then(() => true)
      .catch(() => false);
    if (!clicked) return;
    await page.waitForTimeout(600);
  }
}

test('register → onboarding → create secret → reveal (copy) roundtrip', async ({
  page,
  context,
}) => {
  const username = randomUsername();
  const title = 'Flow Test Login';
  const itemUser = 'flow-user@example.com';
  const itemPass = 's3cret-flow-pw-42!';

  // 1. Real OPAQUE register + auto-login.
  await registerViaUi(page, username);

  // 2. First-run onboarding (welcome → create vault → kit → secret → done).
  await dismissOnboarding(page);

  // 3. Open the Vault via the in-app nav link (a hard reload would re-lock).
  await page.getByRole('link', { name: 'Vault', exact: true }).click();
  await page.waitForURL('**/vault', { timeout: 15_000 });

  // 4. Add a secret.
  await page.getByRole('button', { name: /add item/i }).click();
  await page.getByLabel('Title').fill(title);
  await page.getByLabel('Username').fill(itemUser);
  await page.getByLabel('Password').fill(itemPass);
  await page.getByRole('button', { name: /save item/i }).click();

  // 5. Save auto-opens ItemDetail (reveals on mount). The title + username
  //    (non-secret overview fields) are shown in plaintext.
  await expect(page.getByText(title).first()).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText(itemUser).first()).toBeVisible();

  // 6. Copy exposes the real plaintext (the UI masks in-DOM by design; only
  //    Copy yields the value — ZK: plaintext never lingers in the DOM).
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.getByRole('button', { name: /copy/i }).first().click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toBe(itemPass);
});
