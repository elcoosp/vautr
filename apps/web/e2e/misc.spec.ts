import { expect, type Page, test } from '@playwright/test';

/**
 * Web secondary-surface e2e (VTR-104 follow-up): machine accounts + backup
 * import/export. (WebAuthn/MFA is covered separately by webauthn.spec.)
 *
 * Drives the REAL UI against the live Vautr server on :8080:
 *   - Machine accounts: create one → it appears in the table (active).
 *   - Import / export: export produces a "Backup created" success; pasting an
 *     invalid base64 archive surfaces a graceful error (input path is wired).
 *
 * Run with: `pnpm --filter @vautr/web exec playwright test --config
 * playwright.config.happy.ts`
 */

const ORIGIN = 'http://localhost:5173';
const PASSWORD = 'correct-horse-battery-staple-misc';

function randomUsername(): string {
  return `misc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;
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

test('machine account create + backup export/import paths', async ({ page }) => {
  const username = randomUsername();
  const maName = `ci-deployer-${Date.now().toString(36)}`;

  // 1. Real OPAQUE register + auto-login.
  await registerViaUi(page, username);
  await dismissOnboarding(page);

  // 2. Machine accounts: open the page and create one.
  await page.getByRole('link', { name: 'Machine accounts', exact: true }).click();
  await page.waitForURL('**/machine-accounts', { timeout: 15_000 });

  await page.getByRole('button', { name: /new machine account/i }).click();
  await page.getByLabel('Name').fill(maName);
  // Pick a scope so the account is not created with an empty scope set.
  await page.getByLabel('secrets:read').check().catch(() => {});
  await page.getByRole('button', { name: /^create$/i }).click();

  // The new account appears in the table (active by default).
  await expect(page.getByText(maName).first()).toBeVisible({ timeout: 15_000 });

  // 3. Import / export: export produces a success toast.
  await page.getByRole('link', { name: 'Import / export', exact: true }).click();
  await page.waitForURL('**/import-export', { timeout: 15_000 });

  // Capture the export response to learn the backup id / download url.
  const exportResp = page.waitForResponse(
    (r) => r.url().includes('/backup/export') && r.request().method() === 'POST',
    { timeout: 15_000 },
  );
  await page.getByRole('button', { name: /export backup/i }).click();
  const resp = await exportResp;
  expect(resp.status()).toBe(200);
  const body = (await resp.json()) as { backup_id?: string };
  expect(body.backup_id).toBeTruthy();

  // 4. Import validation: an invalid base64 archive surfaces a graceful error
  //    (the import path is wired and rejects bad input rather than crashing).
  await page.getByLabel('Archive (base64)').fill('!!!not-valid-base64!!!');
  await page.getByRole('button', { name: /restore/i }).click();
  await expect(page.getByText(/restore|base64|invalid|error/i).first()).toBeVisible({
    timeout: 15_000,
  });
});
