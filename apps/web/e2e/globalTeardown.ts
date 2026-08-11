import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execSync } from 'node:child_process';

/**
 * Stops the Vautr server spawned by `globalSetup` (see e2e/globalSetup.ts).
 * Kills the recorded PID and any leftover binary as a fallback.
 */
export default function globalTeardown(): void {
  try {
    const state = JSON.parse(
      readFileSync(join(tmpdir(), 'vautr-webauthn-e2e-state.json'), 'utf8'),
    ) as { pid: number };
    try {
      process.kill(state.pid, 'SIGTERM');
    } catch {
      // already gone
    }
  } catch {
    // no state file
  }
  try {
    execSync('pkill -f "target/debug/vautr-server"', { stdio: 'ignore' });
  } catch {
    // nothing left to kill
  }
}
