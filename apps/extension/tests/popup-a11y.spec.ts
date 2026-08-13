import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { AxeBuilder } from '@axe-core/playwright';
import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { getExtensionId, launchExtensionContext } from './helpers';

/**
 * Accessibility audit of the extension popup (Phase 9 — A11y CI).
 *
 * Runs axe-core against the popup in its canonical states. The locked/auth view
 * needs no backend, so it is always audited. The unlocked vault view requires a
 * live server at `VAUTR_API_URL` (default http://localhost:8080); if none is
 * reachable that part is skipped so the suite stays green in a browser-only CI.
 *
 * Run: `pnpm --filter @vautr/extension test:e2e` (drives `tests/*.spec.ts`).
 */

const SERVER = process.env.VAUTR_API_URL ?? 'http://localhost:8080';

async function serverUp(): Promise<boolean> {
  try {
    const res = await fetch(`${SERVER}/health`, { signal: AbortSignal.timeout(1500) });
    return res.ok;
  } catch {
    return false;
  }
}

test.describe
  .serial('popup accessibility', () => {
    let context: BrowserContext;
    let popup: Page;
    let profileDir: string;
    let extId: string;

    test.beforeAll(async () => {
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-a11y-'));
      context = await launchExtensionContext({ profileDir });
      extId = await getExtensionId(context);
      popup = await context.newPage();
      await popup.goto(`chrome-extension://${extId}/src/popup/popup.html`);
    });

    test.afterAll(async () => {
      await context?.close();
      if (profileDir) rmSync(profileDir, { recursive: true, force: true });
    });

    test('locked auth view has no axe violations', async () => {
      await popup.waitForSelector('#auth-username');
      await expect(popup.getByText('Unlock your vault')).toBeVisible();
      const results = await new AxeBuilder({ page: popup })
        .withTags(['wcag2a', 'wcag2aa'])
        .analyze();
      expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
    });

    test('unlocked vault tabs have no axe violations', async () => {
      test.skip(!(await serverUp()), `no live server at ${SERVER} (skipping unlocked audit)`);
      const username = `a11y-${Date.now()}@vautr.test`;
      const password = 'correct-horse-battery-staple-a11y';
      await popup.getByRole('tab', { name: 'Register' }).click();
      await popup.fill('#auth-username', username);
      await popup.fill('#auth-password', password);
      await popup.getByRole('button', { name: 'Register' }).click();
      await popup.getByText(username, { exact: true }).waitFor({ timeout: 30_000 });
      for (const tab of ['Vault', 'Projects', 'Secrets', 'Generator', 'MFA']) {
        await popup.getByRole('tab', { name: tab }).click();
        const results = await new AxeBuilder({ page: popup })
          .withTags(['wcag2a', 'wcag2aa'])
          .analyze();
        expect(
          results.violations,
          `tab ${tab}: ${JSON.stringify(results.violations, null, 2)}`,
        ).toEqual([]);
      }
    });
  });
