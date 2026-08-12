import { test, expect, type Page, type BrowserContext } from '@playwright/test';
import { DatabaseSync } from 'node:sqlite';

/**
 * MLP Web vault e2e (Wave B1).
 *
 * Drives the web vault against the LIVE vautr-server (managed by Playwright on
 * :8080 with a fresh temp DB) and the built app (on :5173):
 *
 *   register → login → create project → add member (CanView) → reveal secret
 *   → assert UI   PLUS   denied reveal without the `secrets:reveal` grant.
 *
 * Each user runs in its own browser context (isolated IndexedDB), so the
 * client's single stored KDF salt never crosses accounts. The only harness
 * shortcut is reading a freshly-registered member's UUID from the server DB
 * (the API has no directory lookup).
 */

const WEB = 'http://localhost:5173';
const DB = process.env.VAUTR_TEST_DB || '/tmp/vautr-web-e2e.db';
const PASSWORD = 'Correct-Horse-9!Battery-2026';

const base = Date.now().toString(36);
const OWNER = `owner-${base}@vautr.test`;
const MEMBER = `member-${base}@vautr.test`;
const PROJECT_NAME = `E2E Project ${base}`;
const SECRET_KEY = `DB_URL_${base}`;
const SECRET_VALUE = `secret-${base}`;

/** Read a user's uuid by email from the live server DB (harness-only). */
function userIdFor(email: string): string {
  const db = new DatabaseSync(DB);
  try {
    const row = db.prepare('SELECT id FROM users WHERE email = ?').get(email) as
      | { id?: string }
      | undefined;
    return row?.id ?? '';
  } finally {
    db.close();
  }
}

async function register(page: Page, username: string, password: string): Promise<void> {
  await page.goto(`${WEB}/register`);
  await page.fill('#username', username);
  await page.fill('#master-password', password);
  await page.fill('#confirm-password', password);
  await page.click('button[type=submit]');
  await page.waitForURL('**/dashboard', { timeout: 30_000 });
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
}

async function createProject(page: Page): Promise<string> {
  await page.getByRole('link', { name: 'New project' }).click();
  await expect(page.getByRole('heading', { name: 'New project' })).toBeVisible();
  await page.fill('#project-name', PROJECT_NAME);
  await page.getByRole('button', { name: 'Create project' }).click();
  await page.waitForURL(/\/projects\/[0-9a-f-]{36}$/, { timeout: 15_000 });
  const m = page.url().match(/\/projects\/([0-9a-f-]{36})$/);
  const uuid = m?.[1] ?? '';
  expect(uuid).toMatch(/[0-9a-f-]{36}/);
  return uuid;
}

async function addSecretAndReveal(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'New secret' }).click();
  await page.fill('#secret-key', SECRET_KEY);
  await page.fill('#secret-value', SECRET_VALUE);
  await page.getByRole('button', { name: 'Create secret' }).click();
  await expect(page.getByRole('cell', { name: SECRET_KEY })).toBeVisible();
  await page.getByRole('button', { name: 'Reveal' }).click();
  await expect(page.getByText(`${SECRET_KEY}: ${SECRET_VALUE}`)).toBeVisible();
}

async function addMember(page: Page, memberUuid: string): Promise<void> {
  await page.getByRole('tab', { name: 'Members' }).click();
  await page.getByRole('button', { name: 'Add member' }).click();
  await page.fill('#member-uuid', memberUuid);
  // Default permission is Can View; keep it.
  await page.getByRole('dialog').getByRole('button', { name: 'Add member' }).click();
  await expect(page.getByText(memberUuid)).toBeVisible();
}

test('web vault: project + member flow, reveal granted and denied', async ({ browser }) => {
  const ctxOwner: BrowserContext = await browser.newContext();
  const pageOwner = await ctxOwner.newPage();

  // --- 1. Register the owner, create a project, add + reveal a secret ---
  await register(pageOwner, OWNER, PASSWORD);
  await createProject(pageOwner);
  await addSecretAndReveal(pageOwner);

  // --- 2. Register the member in its own context; read its uuid ---
  const ctxMember: BrowserContext = await browser.newContext();
  const pageMember = await ctxMember.newPage();
  await register(pageMember, MEMBER, PASSWORD);
  const memberUuid = userIdFor(MEMBER);
  expect(memberUuid).toMatch(/[0-9a-f-]{36}/);

  // --- 3. Owner adds the member with CanView (client-side, already on project) ---
  await addMember(pageOwner, memberUuid);

  // --- 4. Member opens the project and is DENIED reveal (no secrets:reveal) ---
  // Client-side navigation (no full reload, so the vault stays unlocked).
  await pageMember.getByRole('link', { name: 'Projects' }).click();
  await expect(pageMember.getByRole('heading', { name: 'Projects' })).toBeVisible();
  await pageMember.getByRole('link', { name: PROJECT_NAME }).click();
  await expect(pageMember.getByRole('cell', { name: SECRET_KEY })).toBeVisible();
  await pageMember.getByRole('button', { name: 'Reveal' }).click();
  await expect(pageMember.getByRole('alert')).toContainText(/secrets:reveal|reveal/i);
  // The plaintext must NOT be exposed to a member without the grant.
  await expect(pageMember.getByText(SECRET_VALUE)).toHaveCount(0);

  await ctxOwner.close();
  await ctxMember.close();
});
