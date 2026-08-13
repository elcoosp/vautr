import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { BrowserContext, Page } from '@playwright/test';
import { test } from '@playwright/test';
import { getExtensionId, launchExtensionContext } from './helpers';

/**
 * Canonical-state screenshot gallery (Phase 9 — Storybook/gallery stand-in).
 *
 * Captures one PNG per canonical popup screen state so visual drift between
 * clients can be reviewed in PRs. The locked/auth view needs no backend; the
 * unlocked tabs require a live server at `VAUTR_API_URL` (default
 * http://localhost:8080) and are skipped otherwise.
 *
 * Output: `apps/extension/tests/screenshots/*.png`.
 * Run: `pnpm --filter @vautr/extension test:e2e`.
 */

const SERVER = process.env.VAUTR_API_URL ?? 'http://localhost:8080';
const OUT = join(import.meta.dirname, 'screenshots');

async function serverUp(): Promise<boolean> {
  try {
    const res = await fetch(`${SERVER}/health`, { signal: AbortSignal.timeout(1500) });
    return res.ok;
  } catch {
    return false;
  }
}

test.describe
  .serial('popup screenshots', () => {
    let context: BrowserContext;
    let popup: Page;
    let profileDir: string;
    let extId: string;

    test.beforeAll(async () => {
      profileDir = mkdtempSync(join(tmpdir(), 'vautr-shots-'));
      context = await launchExtensionContext({ profileDir });
      extId = await getExtensionId(context);
      popup = await context.newPage();
      await popup.goto(`chrome-extension://${extId}/src/popup/popup.html`);
    });

    test.afterAll(async () => {
      await context?.close();
      if (profileDir) rmSync(profileDir, { recursive: true, force: true });
    });

    test('capture locked auth view', async () => {
      await popup.waitForSelector('#auth-username');
      await popup.screenshot({ path: join(OUT, 'locked-auth.png') });
    });

    test('capture unlocked tabs', async () => {
      test.skip(!(await serverUp()), `no live server at ${SERVER} (skipping unlocked shots)`);
      const username = `shots-${Date.now()}@vautr.test`;
      const password = 'correct-horse-battery-staple-shots';
      await popup.getByRole('tab', { name: 'Register' }).click();
      await popup.fill('#auth-username', username);
      await popup.fill('#auth-password', password);
      await popup.getByRole('button', { name: 'Register' }).click();
      await popup.getByText(username, { exact: true }).waitFor({ timeout: 30_000 });
      for (const tab of ['Vault', 'Projects', 'Secrets', 'Generator', 'MFA']) {
        await popup.getByRole('tab', { name: tab }).click();
        await popup.screenshot({ path: join(OUT, `tab-${tab.toLowerCase()}.png`) });
      }
    });
  });
