/**
 * Canonical Vautr domain types (data.md §3, §5, §6).
 *
 * Security contract: none of these types carry plaintext secret strings. The
 * `Draft` type is the sole exception and is explicitly scoped to the ephemeral,
 * in-memory draft slice used only to survive async Core validation (data.md §4
 * User-Input Exception, ui-state-charts §4).
 */

/** Rendered in the fast virtualized list. data.md §3.1. */
export interface DecryptedOverview {
  uuid: string;
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];
  updatedAt: number; // i64 epoch ms
}

/** Unencrypted aggregate metadata. data.md §3.5. */
export interface ItemMetadata {
  createdAt: number;
  updatedAt: number;
  trashed: boolean;
}

export type CryptoError =
  | { type: 'TagMismatch' }
  | { type: 'KeyGenMismatch' }
  | { type: 'MalformedCiphertext' };

export type DbError =
  | { type: 'ConstraintViolation'; message: string }
  | { type: 'Corruption'; message: string }
  | { type: 'IoError'; message: string };

export type CoreError =
  | { type: 'CryptoError'; details: CryptoError }
  | { type: 'DbError'; details: DbError }
  | { type: 'EpochMismatch' }
  | { type: 'HandleInvalid' }
  | { type: 'NetworkError'; message: string };

/** Payload for the 412 conflict resolution UI. data.md §7.2. */
export interface ConflictEvent {
  uuid: string;
  localVersion: string; // u64 as string
  serverVersion: string; // u64 as string
  isToxic: boolean;
}

/**
 * Surgical UI revert payload on `MutationFailed`. data.md §5.3.
 * NEVER carries `DecryptedSecret`; only list-level overviews.
 */
export type RevertibleState =
  | { type: 'Saved'; overview: DecryptedOverview }
  | { type: 'Deleted'; overview: DecryptedOverview };

/** The reactive event stream pushed from Core to UI. data.md §6.3. */
export type VaultStateUpdate =
  | { type: 'SyncStarted' }
  | { type: 'SyncProgress'; progressPercentage: number } // u8 0-100
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

/**
 * Opaque handle / task receipt primitive. Both are u64 surfaced to JS as
 * strings (data.md §1 rule 1).
 */
export type OpaqueHandle = string;
export type TaskReceipt = string;

/** Opaque secret handle + action delegation. data.md §6.2. */
export type CoreAction =
  | { type: 'CopyToClipboard'; handle: OpaqueHandle }
  | { type: 'Autofill'; handle: OpaqueHandle };

/**
 * Ephemeral draft. Holds user-typed plaintext ONLY to survive async Core
 * validation. It lives in an in-memory-only slice, is never serialized to
 * localStorage, and is wiped on lock / mutation success (ui-state-charts §4).
 */
export interface Draft {
  uuid: string;
  syncEpoch: string; // u64 as string
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];
  password?: string;
  notes?: string;
}

/** Delete command payload. data.md §5.2. */
export interface DeleteItemPayload {
  uuid: string;
  syncEpoch: string; // u64 as string
}
