import { expect, type Page, test } from '@playwright/test';

/**
 * Conflict-resolution modal (VTR-056, data.md §7.2).
 *
 * Drives the real web app at ORIGIN. A pending local edit is produced by the
 * normal add flow, then we intercept the server's `POST /sync/push-batch` and
 * force a 412 / `conflict` response so the client emits `ConflictDetected` and
 * the global modal renders. Covers TDD1–5.
 *
 * Requires the app (`pnpm --filter @vautr/web dev`) and the server running, plus
 * a logged-in session. Run with: `pnpm --filter @vautr/web exec playwright test
 * e2e/conflict.spec.ts`.
 */

const ORIGIN = 'http://localhost:5173';

/** Force the next push-batch to return a conflict for `uuid`. */
async function forceConflict(page: Page, uuid: string, toxic = false): Promise<void> {
  await page.route('**/sync/push-batch', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        results: [
          {
            uuid,
            status: toxic ? 'toxic' : 'conflict',
            version: 3,
          },
        ],
      }),
    });
  });
}

async function loginAndAddItem(page: Page): Promise<string> {
  // Existing e2e helpers log in; here we reuse the app's session from
  // localStorage seeded by a prior registration. The add flow creates a pending
  // item and returns its uuid via the DOM data attribute.
  await page.goto(`${ORIGIN}/secrets`);
  await page.getByRole('button', { name: /add item/i }).click();
  await page.getByLabel(/title/i).fill('Conflict Item');
  await page.getByLabel(/username/i).fill('user');
  await page.getByLabel(/password/i).fill('s3cr3t');
  await page.getByRole('button', { name: /save/i }).click();
  const row = page.getByTestId('secret-row').last();
  return (await row.getAttribute('data-uuid')) ?? 'unknown';
}

test.describe('VTR-056 conflict modal', () => {
  test('TDD1: a valid conflict shows the modal with the correct message + two buttons', async ({
    page,
  }) => {
    const uuid = await loginAndAddItem(page);
    await forceConflict(page, uuid, false);
    await page.getByRole('button', { name: /sync/i }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(/updated on another device/i);
    await expect(dialog.getByRole('button', { name: /keep server version/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /force overwrite with local/i })).toBeVisible();
  });

  test('TDD2: "Keep Server Version" overwrites the local item and clears the DashMap entry', async ({
    page,
  }) => {
    const uuid = await loginAndAddItem(page);
    await forceConflict(page, uuid, false);
    await page.getByRole('button', { name: /sync/i }).click();

    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /keep server version/i }).click();

    // Modal closes and the conflict is resolved (no longer in the DashMap).
    await expect(page.getByRole('dialog')).toHaveCount(0);
    await expect(page.getByTestId(`conflict-row-${uuid}`)).toHaveCount(0);
  });

  test('TDD3: a toxic conflict mentions "unreadable update" with Keep Local / Overwrite Server', async ({
    page,
  }) => {
    const uuid = await loginAndAddItem(page);
    await forceConflict(page, uuid, true);
    await page.getByRole('button', { name: /sync/i }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText(/unreadable update/i);
    await expect(dialog.getByRole('button', { name: /keep local/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /overwrite server/i })).toBeVisible();
  });

  test('TDD4: dismissing the modal does not resolve the conflict (stays ignored)', async ({
    page,
  }) => {
    const uuid = await loginAndAddItem(page);
    await forceConflict(page, uuid, false);
    await page.getByRole('button', { name: /sync/i }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // Close via Escape (no choice made).
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    // Re-triggering a sync with the same pending edit re-shows the conflict
    // because it was dismissed, not resolved (still in the DashMap as ignored).
    await forceConflict(page, uuid, false);
    await page.getByRole('button', { name: /sync/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
  });

  test('TDD5: multiple concurrent conflicts queue and show one at a time', async ({ page }) => {
    const a = await loginAndAddItem(page);
    const b = await loginAndAddItem(page);
    const c = await loginAndAddItem(page);
    // Force all three to conflict on the next sync.
    await page.route('**/sync/push-batch', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          results: [a, b, c].map((uuid) => ({ uuid, status: 'conflict', version: 3 })),
        }),
      });
    });
    await page.getByRole('button', { name: /sync/i }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // Only one dialog at a time (no overlap).
    await expect(dialog).toHaveCount(1);
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    await expect(dialog).toBeVisible(); // second conflict
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    await expect(dialog).toBeVisible(); // third conflict
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    await expect(page.getByRole('dialog')).toHaveCount(0); // all resolved
  });
});
