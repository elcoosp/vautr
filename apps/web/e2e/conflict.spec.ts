import { expect, type Page, test } from '@playwright/test';

/**
 * e2e conflict-resolution suite (VTR-056). The app talks to the backend via the
 * same-origin `/api` path, which vite proxies to :8080. That proxy is flaky
 * under load on macOS (deterministic 502 on the Nth sync-pull), so we bypass
 * it entirely: a single `beforeEach` route forwards every `/api/**` request
 * straight to the IPv4 backend (127.0.0.1:8080). CORS is permissive on the
 * server, so the direct fetch is allowed. Push-batches are intercepted
 * in-browser and forced to a conflict when `conflictState` is armed.
 */
interface ConflictState {
  watch: Set<string>;
  resolved: Set<string>;
  toxic: boolean;
}
let conflictState: ConflictState | null = null;

test.beforeEach(async ({ page }) => {
  await page.route('**/api/**', async (route) => {
    const original = route.request().url();
    const backend = original
      .replace('http://127.0.0.1:5173/api', 'http://127.0.0.1:8080')
      .replace('http://localhost:5173/api', 'http://127.0.0.1:8080');
    const path = original.replace(/^https?:\/\/[^/]+/, '').replace(/^\/api/, '');
    // Force a conflict on push-batch for the watched uuids.
    if (path.startsWith('/sync/push-batch') && conflictState) {
      const body = (route.request().postDataJSON?.() ?? {}) as {
        items?: { uuid: string }[];
      };
      const uuids = (body.items ?? []).map((i) => i.uuid);
      const results = uuids.map((uuid) => {
        if (!conflictState!.watch.has(uuid)) return { uuid, status: 'success', version: 3 };
        if (conflictState!.resolved.has(uuid)) return { uuid, status: 'success', version: 3 };
        return {
          uuid,
          status: conflictState!.toxic ? 'toxic' : 'conflict',
          version: 3,
        };
      });
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ results }),
      });
      return;
    }
    try {
      const resp = await route.fetch({ url: backend });
      await route.fulfill({ response: resp });
    } catch {
      await route.continue();
    }
  });
});

/**
 * Conflict-resolution modal (VTR-056, data.md §7.2).
 *
 * Drives the real web app at ORIGIN. A pending local edit is produced by the
 * normal add flow, then we trigger a sync (pushing the new item) and intercept
 * the server's `POST /sync/push-batch` to force a `conflict` response, so the
 * client emits `ConflictDetected` and the global modal renders. Covers TDD1–5.
 *
 * The app has no manual "sync" button (it auto-syncs on login / after
 * resolving a conflict), so the suite triggers a push via the dev-only
 * `window.__vautrSync` hook exposed by the web client on localhost.
 *
 * The app is zero-knowledge: the session is minted through the real in-browser
 * OPAQUE register flow (no DB seeding / IndexedDB injection), exactly as a real
 * user would. Run with: `pnpm --filter @vautr/web exec playwright test
 * e2e/conflict.spec.ts`.
 */

const ORIGIN = 'http://localhost:5173';
const PASSWORD = 'correct-horse-battery-staple-conflict';

function randomUsername(): string {
  return `conflict-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@vautr.test`;
}

async function registerViaUi(page: Page, username: string): Promise<void> {
  // Idempotent: TDD5 calls addItem multiple times on the same page; once a
  // session exists there's nothing to register again. A fresh page has url
  // `about:blank`, so only skip when we're clearly already authenticated.
  const url = page.url();
  const authed =
    /dashboard|vault|secrets|projects|shares|settings|audit|generator|mfa|tokens|machine-accounts|import-export/.test(
      url,
    );
  if (authed) return;
  await page.goto(`${ORIGIN}/register`);
  await page.getByLabel('Username').fill(username);
  await page.getByLabel('Master password', { exact: true }).fill(PASSWORD);
  await page.getByLabel('Confirm master password', { exact: true }).fill(PASSWORD);
  await page.getByRole('button', { name: /create vault/i }).click();
  try {
    await page.waitForURL('**/dashboard', { timeout: 30_000 });
  } catch {
    const u = page.url();
    const alert = await page
      .getByRole('alert')
      .first()
      .textContent()
      .catch(() => null);
    throw new Error(`did not reach /dashboard; url=${u} alert=${alert}`);
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

/**
 * Arm a forced conflict for the given `uuids`. The actual interception happens
 * in the `beforeEach` route (which bypasses vite's proxy and fulfills
 * push-batch in-browser). We scope to specific uuids so the app's own login
 * sync (which pushes unrelated key/material items) is NOT conflicted. `resolved`
 * uuids return success, so the app's post-resolve re-sync does not re-raise.
 */
async function armConflict(
  _page: Page,
  uuids: string[],
  toxic = false,
): Promise<{ markResolved: (uuid: string) => void }> {
  conflictState = { watch: new Set(uuids), resolved: new Set<string>(), toxic };
  return { markResolved: (uuid: string) => conflictState?.resolved.add(uuid) };
}

/** Trigger a push via the dev-only hook; resolves once the sync settles.
 * Retries a few times: the vite dev proxy can intermittently return a 502
 * (upstream connection reset) on the sync-pull under load. */
async function triggerSync(page: Page): Promise<void> {
  let lastErr: unknown;
  for (let attempt = 0; attempt < 4; attempt++) {
    try {
      await page.evaluate(() => {
        const fn = (window as unknown as { __vautrSync?: () => Promise<void> }).__vautrSync;
        if (!fn) throw new Error('__vautrSync hook unavailable');
        return fn();
      });
      return;
    } catch (err) {
      lastErr = err;
      await page.waitForTimeout(800);
    }
  }
  throw lastErr;
}

/** Create a new secret (left pending / un-synced) and return its uuid. */
async function addItem(page: Page): Promise<string> {
  // Establish a real, fully-functional vault session through the OPAQUE
  // register flow (the app then auto-logs-in and persists its session).
  await registerViaUi(page, randomUsername());
  await dismissOnboarding(page);

  // Navigate to the Vault view (in-app SPA nav; a hard reload would re-lock).
  // Dismiss any transient overlay first, then ensure we're on the Vault. The
  // "Add item" button lives in the Vault header and is always present once
  // authenticated, so prefer clicking it directly over re-navigating.
  for (let i = 0; i < 3; i++) {
    const open = page.getByRole('dialog').first();
    if (await open.isVisible().catch(() => false)) {
      await page.keyboard.press('Escape');
      await page.waitForTimeout(300);
    }
  }
  const addBtn = page.getByRole('button', { name: /add item/i });
  if (!(await addBtn.isVisible().catch(() => false))) {
    // Not on the Vault view — click the Vault nav link.
    await page.getByRole('link', { name: /vault/i }).first().click({ force: true });
    await addBtn.waitFor({ timeout: 15_000 });
  }
  await addBtn.click();
  await page.getByLabel(/title/i).fill('Conflict Item');
  await page.getByLabel(/username/i).fill('user');
  await page.getByLabel(/password/i).fill('s3cr3t');
  await page.getByRole('button', { name: /save/i }).click();
  // After save, VaultView selects the new item (selectedUuid = uuid), so its
  // row is aria-selected="true". The list is sorted by updatedAt desc with a
  // virtualizer, so DOM order is NOT creation order — read the selected row.
  const selected = page.locator('[data-uuid][aria-selected="true"]');
  await selected.first().waitFor({ state: 'visible', timeout: 15_000 });
  return (await selected.first().getAttribute('data-uuid')) ?? 'unknown';
}

test.describe('VTR-056 conflict modal', () => {
  test('TDD1: a valid conflict shows the modal with the correct message + two buttons', async ({
    page,
  }) => {
    const uuid = await addItem(page);
    const { markResolved } = await armConflict(page, [uuid], false);
    await triggerSync(page);

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(/updated on another device/i);
    await expect(dialog.getByRole('button', { name: /keep server version/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /force overwrite with local/i })).toBeVisible();
    markResolved(uuid);
  });

  test('TDD2: "Keep Server Version" overwrites the local item and clears the DashMap entry', async ({
    page,
  }) => {
    const uuid = await addItem(page);
    const { markResolved } = await armConflict(page, [uuid], false);
    await triggerSync(page);

    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /keep server version/i }).click();

    // Modal closes and the conflict is resolved (no longer in the DashMap).
    await expect(page.getByRole('dialog')).toHaveCount(0);
    markResolved(uuid);
  });

  test('TDD3: a toxic conflict mentions "unreadable update" with Keep Local / Overwrite Server', async ({
    page,
  }) => {
    const uuid = await addItem(page);
    const { markResolved } = await armConflict(page, [uuid], true);
    await triggerSync(page);

    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText(/unreadable update/i);
    await expect(dialog.getByRole('button', { name: /keep local/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /overwrite server/i })).toBeVisible();
    markResolved(uuid);
  });

  test('TDD4: dismissing the modal does not resolve the conflict (stays ignored)', async ({
    page,
  }) => {
    const uuid = await addItem(page);
    const { markResolved } = await armConflict(page, [uuid], false);
    await triggerSync(page);

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // Escape closes the modal WITHOUT resolving (the local item is preserved,
    // not overwritten with the server version). The conflict is removed from
    // the queue so it does not linger.
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
    // The local item is still present in the vault (not dropped/overwritten).
    await expect(page.locator(`[data-uuid="${uuid}"]`)).toBeVisible();
    markResolved(uuid);
  });

  test('TDD5: multiple concurrent conflicts queue and show one at a time', async ({ page }) => {
    const a = await addItem(page);
    const b = await addItem(page);
    const c = await addItem(page);
    const { markResolved } = await armConflict(page, [a, b, c], false);
    // One sync pushes all three pending items -> three queued conflicts.
    await triggerSync(page);

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // Only one dialog at a time (no overlap).
    await expect(dialog).toHaveCount(1);
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    await expect(dialog).toBeVisible(); // second conflict
    markResolved(a);
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    await expect(dialog).toBeVisible(); // third conflict
    markResolved(b);
    await dialog.getByRole('button', { name: /keep server version/i }).click();
    markResolved(c);
    await expect(page.getByRole('dialog')).toHaveCount(0); // all resolved
  });
});
