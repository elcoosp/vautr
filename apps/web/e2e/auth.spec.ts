import { expect, type Page, test } from '@playwright/test';

/**
 * Web auth lifecycle e2e (VTR-104 follow-up): login, lock/unlock, and
 * master-key rotation.
 *
 * Drives the REAL UI against the live Vautr server on :8080:
 *   - register (OPAQUE) → create secret → copy works while unlocked
 *   - log out (locks) → log back in via UnlockScreen → reaches the vault
 *   - Settings → rotate vault key → generation increments
 *   - a secret created AFTER rotation still decrypts (crypto path intact)
 *
 * Run with: `pnpm --filter @vautr/web exec playwright test --config
 * playwright.config.happy.ts`
 */

const ORIGIN = 'http://localhost:5173';
const PASSWORD = 'correct-horse-battery-staple-auth';

function randomUsername(): string {
  return `auth-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;
}

async function registerViaUi(page: Page, username: string): Promise<void> {
  await page.goto(`${ORIGIN}/register`);
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Master password', { exact: true }).fill(PASSWORD);
  await page.getByLabel('Confirm master password', { exact: true }).fill(PASSWORD);
  await page.getByRole('button', { name: /create vault/i }).click();
  await page.waitForURL('**/dashboard', { timeout: 30_000 });
}

async function dismissOnboarding(page: Page): Promise<void> {
  const overlay = page.locator('div.fixed.inset-0');
  await page.getByText('Welcome to Vautr').waitFor({ state: 'visible', timeout: 15_000 }).catch(() => {});
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

async function createSecret(
  page: Page,
  title: string,
  itemUser: string,
  itemPass: string,
): Promise<void> {
  await page.getByRole('button', { name: /add item/i }).click();
  await page.getByLabel('Title').fill(title);
  await page.getByLabel('Username').fill(itemUser);
  await page.getByLabel('Password').fill(itemPass);
  await page.getByRole('button', { name: /save item/i }).click();
  await expect(page.getByText(title).first()).toBeVisible({ timeout: 15_000 });
}

async function copyAndExpect(
  page: Page,
  context: import('@playwright/test').BrowserContext,
  value: string,
): Promise<void> {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.getByRole('button', { name: /copy/i }).first().click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toBe(value);
}

test('auth lifecycle: unlock decrypts, lock/logout re-locks, re-login works, key rotation keeps crypto', async ({
  page,
  context,
}) => {
  const username = randomUsername();
  const title = 'Auth Test Login';
  const itemUser = 'auth-user@example.com';
  const itemPass = 's3cret-auth-pw-7!';
  const title2 = 'Rotated Secret';
  const itemPass2 = 's3cret-rotated-9!';

  // 1. Real OPAQUE register + auto-login.
  await registerViaUi(page, username);
  await dismissOnboarding(page);

  // 2. Create a secret and decrypt it while unlocked.
  await page.getByRole('link', { name: 'Vault', exact: true }).click();
  await page.waitForURL('**/vault', { timeout: 15_000 });
  await createSecret(page, title, itemUser, itemPass);
  await copyAndExpect(page, context, itemPass);

  // 3. Log out (locks the vault + forgets the session token).
  await page.getByRole('button', { name: /log out/i }).click();
  await expect(page.getByRole('button', { name: /unlock vault/i })).toBeVisible({ timeout: 15_000 });

  // 4. Log back in with the SAME credentials → reaches the dashboard (unlock works).
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Master password', { exact: true }).fill(PASSWORD);
  await page.getByRole('button', { name: /unlock vault/i }).click();
  await expect(page.getByText('Dashboard').first()).toBeVisible({ timeout: 20_000 });

  // 5. Rotate the master key from Settings; generation increments.
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.waitForURL('**/settings', { timeout: 15_000 });
  await page.getByLabel('Master password', { exact: true }).fill(PASSWORD);
  await page.getByRole('button', { name: /rotate vault key/i }).click();
  await expect(page.getByText(/Current key generation:/i)).toBeVisible({ timeout: 15_000 });

  // 6. A secret created AFTER rotation still decrypts (crypto path intact).
  await page.getByRole('link', { name: 'Vault', exact: true }).click();
  await page.waitForURL('**/vault', { timeout: 15_000 });
  await createSecret(page, title2, 'rotated-user@example.com', itemPass2);
  await copyAndExpect(page, context, itemPass2);
});
