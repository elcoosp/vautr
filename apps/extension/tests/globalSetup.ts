import { execSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Playwright global setup: build the unpacked extension into `dist/` before the
 * smoke test launches Chromium with `--load-extension`.
 */
export default function globalSetup(): void {
  // tests/globalSetup.ts -> extension root
  const extensionRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
  execSync('pnpm build', { cwd: extensionRoot, stdio: 'inherit' });
}
