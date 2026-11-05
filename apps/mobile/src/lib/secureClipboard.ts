import { Clipboard } from 'react-native';

/**
 * Clipboard helpers for sensitive values (Recovery Key, etc.).
 *
 * VTRFIX-SEC-C05: a naive `Clipboard.setString(secret)` leaves the secret in
 * the system clipboard indefinitely (other apps can read it at will). These
 * helpers schedule an automatic clear shortly after the copy and keep a
 * module-level epoch counter so a *newer* copy is never wiped early by an
 * older pending timer (and vice versa: a stale unmount cleanup can't clear a
 * copy the user made afterwards).
 */

/** How long a sensitive value may live in the clipboard before clearing. */
const SENSITIVE_CLIPBOARD_CLEAR_MS = 45_000;

let copyEpoch = 0;

/**
 * Copy a sensitive value and schedule a clipboard clear.
 * Returns the copy's epoch token (pass it to `clearSensitiveClipboard`).
 */
export function copySensitiveToClipboard(value: string): number {
  copyEpoch += 1;
  const token = copyEpoch;
  void Clipboard.setString(value);
  setTimeout(() => {
    // Only clear if no newer copy superseded this one.
    if (copyEpoch === token) {
      void Clipboard.setString('');
    }
  }, SENSITIVE_CLIPBOARD_CLEAR_MS);
  return token;
}

/**
 * Clear the clipboard if it still holds the copy identified by `token`
 * (used on unmount so the secret doesn't outlive the screen).
 * Passing no token force-clears.
 */
export function clearSensitiveClipboard(token?: number): void {
  if (token === undefined || copyEpoch === token) {
    // Invalidate any pending auto-clear timer for the superseded copy.
    copyEpoch += 1;
    void Clipboard.setString('');
  }
}
