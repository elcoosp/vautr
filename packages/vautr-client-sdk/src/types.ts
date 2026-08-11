/**
 * Vautr client SDK domain + wire protocol types (data.md §6, client.md §4).
 *
 * Boundary rules honoured here (data.md §1):
 *  - u64 (handles, receipts, epochs) cross as `string`.
 *  - `DecryptedSecret` NEVER crosses into JS; only opaque handles do.
 *  - progress is an integer 0-100.
 */

/** Rendered in the virtualized list. data.md §3.1. */
export interface DecryptedOverview {
  uuid: string;
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];
  updatedAt: number; // i64 epoch ms
}

export type CoreError =
  | { type: 'CryptoError'; details: CryptoError }
  | { type: 'DbError'; details: DbError }
  | { type: 'EpochMismatch' }
  | { type: 'HandleInvalid' }
  | { type: 'NetworkError'; message: string };

export type CryptoError =
  | { type: 'TagMismatch' }
  | { type: 'KeyGenMismatch' }
  | { type: 'MalformedCiphertext' };

export type DbError =
  | { type: 'ConstraintViolation'; message: string }
  | { type: 'Corruption'; message: string }
  | { type: 'IoError'; message: string };

export interface ConflictEvent {
  uuid: string;
  localVersion: string; // u64 as string
  serverVersion: string; // u64 as string
  isToxic: boolean;
}

export type RevertibleState =
  | { type: 'Saved'; overview: DecryptedOverview }
  | { type: 'Deleted'; overview: DecryptedOverview };

export type VaultStateUpdate =
  | { type: 'SyncStarted' }
  | { type: 'SyncProgress'; progressPercentage: number }
  | { type: 'SyncCompleted' }
  | { type: 'SyncFailed'; error: CoreError }
  | { type: 'OverviewUpserted'; overview: DecryptedOverview }
  | { type: 'OverviewDeleted'; uuid: string }
  | { type: 'ConflictDetected'; event: ConflictEvent }
  | { type: 'KeyUpdateRequired' }
  | { type: 'VaultLocked' }
  | { type: 'NewerVersionAvailable'; uuid: string }
  | { type: 'MutationSucceeded'; receipt: string }
  | {
      type: 'MutationFailed';
      receipt: string;
      error: CoreError;
      originalState: RevertibleState;
    };

/** Opaque u64 surfaced as string. */
export type OpaqueHandle = string;
export type TaskReceipt = string;

export type CoreAction =
  | { type: 'CopyToClipboard'; handle: OpaqueHandle }
  | { type: 'Autofill'; handle: OpaqueHandle };

// ---------------------------------------------------------------------------
// Worker bridge protocol (postMessage, client.md §4)
// ---------------------------------------------------------------------------

/** Main -> worker request. */
export type WorkerRequest =
  | { id: string; method: 'unlock'; args: { rawKey: Uint8Array; localGen: number } }
  | { id: string; method: 'reveal'; args: { uuid: string; encKeyGen: number; payload: Uint8Array } }
  | { id: string; method: 'perform'; args: { action: CoreAction } }
  | { id: string; method: 'release'; args: { handle: OpaqueHandle } }
  | {
      id: string;
      method: 'encrypt';
      args: { uuid: string; encKeyGen: number; plaintext: Uint8Array };
    };

/** Worker -> main response. */
export type WorkerResponse =
  | { id: string; ok: true; result: unknown }
  | { id: string; ok: false; error: string }
  // Platform event routed to the main thread's secure glue. The secret string
  // never enters React state; the glue writes it to the clipboard and zeroizes.
  | { id: 'clipboard'; action: CoreAction; secret: string };

/** Injectable clipboard handler. Returns true if handled. */
export type ClipboardHandler = (action: CoreAction, secret: string) => void;
