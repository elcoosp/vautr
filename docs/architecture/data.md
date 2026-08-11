# Vautr Canonical Data Schema

This document defines the exact data structures, type mappings, and serialization contracts across the Vautr 4-layer boundary: **Rust (Core) ⇄ UniFFI (Kotlin/Swift) ⇄ Turbo Module (Native) ⇄ TypeScript (React Native)**. 

Deviations from this specification will result in FFI compilation failures, silent data truncation, or catastrophic security boundary violations.

---

## 1. Universal Boundary Rules

1.  **The `u64` Precision Rule:** JavaScript `number` (IEEE 754 double) loses integer precision above 2^53 - 1. All `u64` types (Sync Epochs, Versions, Receipts, Handles) **MUST** cross the React Native bridge as `string`. The Turbo Module parses the string back into `Long`/`Int64` before calling UniFFI.
2.  **The Date Rule:** All timestamps cross the FFI boundary as `i64` (Unix Epoch Milliseconds). TypeScript reconstructs `new Date(epochMs)`.
3.  **The Enum Rule:** Rust Enums with associated data do not serialize cleanly over standard RN bridges. They **MUST** be flattened into TypeScript Discriminated Unions.
4.  **The Restricted API Rule:** Types marked `[RESTRICTED]` are compiled out of the Mobile/Web builds via `#[cfg(feature = "desktop-api")]`. They must **never** be mapped in the Turbo Module or TypeScript definitions.
5.  **The Integer-Only Progress Rule:** Floating-point math across JS/Rust boundaries introduces non-determinism. All progress metrics **MUST** be integers (e.g., `u8` 0-100 percentage).
6.  **The User-Input Exception:** The UI *must* hold user-typed secrets temporarily. This is the only time plaintext secrets exist in the JS heap. See Section 4.

---

## 2. Primitive Mappings

| Rust Type | UniFFI (Swift) | UniFFI (Kotlin) | Turbo Module TS Type | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `String` | `String` | `String` | `string` | Direct mapping. |
| `bool` | `Bool` | `Boolean` | `boolean` | Direct mapping. |
| `i32` | `Int32` | `Int` | `number` | Safe in JS. |
| `i64` | `Int64` | `Long` | `number` | Used for Epoch ms. Precision safe for dates. |
| `u64` | `UInt64` | `Long` | `string` | **CRITICAL:** Passed as string to JS to prevent precision loss. |
| `u8` | `UInt8` | `Byte` | `number` | Used for Progress %, Totp digits. |
| `Uuid` | `String` | `String` | `string` | Passed as hyphenated string across FFI. |
| `Vec<u8>` | `Data` | `List<Byte>` | `[RESTRICTED]` | Never crosses JS bridge. Handled natively. |
| `Zeroizing<T>`| `T` | `T` | `[RESTRICTED]` | See Sec 4. |

---

## 3. Core Domain Models (The Atomic Aggregates)

These structures represent the encrypted vault data. They map directly to the SQLite schema and the FTS5 index.

### 3.1 `DecryptedOverview`
Rendered in the fast virtualized list. Encrypted by OEK.

**Rust:**
```rust
pub struct DecryptedOverview {
    pub uuid: Uuid,
    pub title: String,
    pub subtitle: String,
    pub icon_key: String,
    pub urls: Vec<String>,
    pub updated_at: i64,
}
```
**TypeScript:**
```typescript
export interface DecryptedOverview {
  uuid: string;
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];
  updatedAt: number; 
}
```

### 3.2 `DecryptedSecret` [RESTRICTED]
Rendered on demand via Opaque Handle. Encrypted by DEK.

**Rust:**
```rust
pub struct DecryptedSecret {
    pub password: Zeroizing<String>,
    pub totp: Option<TotpSecret>,
    pub notes: Zeroizing<String>,
    pub fields: Vec<CustomField>,
}
```
**TypeScript:** `undefined` (Does not exist in JS).
**Handling:** The Turbo Module consumes the UniFFI `DecryptedSecret` natively to service `CoreAction`s and Native Overlays. It is never passed to JS.

### 3.3 `TotpSecret` [RESTRICTED]

**Rust:**
```rust
pub struct TotpSecret {
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u8,
    pub secret_base32: Zeroizing<String>,
}

pub enum TotpAlgorithm { Sha1, Sha256, Sha512 }
```
**TypeScript:** `[RESTRICTED]`

### 3.4 `CustomField` [RESTRICTED]

**Rust:**
```rust
pub struct CustomField {
    pub name: String,
    pub value: Zeroizing<String>,
    pub field_type: CustomFieldType,
}

pub enum CustomFieldType { Text, Hidden }
```
**TypeScript:** `[RESTRICTED]`

### 3.5 `ItemMetadata`
Unencrypted metadata attached to the atomic aggregate.

**Rust:**
```rust
pub struct ItemMetadata {
    pub created_at: i64,
    pub updated_at: i64,
    pub trashed: bool,
}
```
**TypeScript:**
```typescript
export interface ItemMetadata {
  createdAt: number;
  updatedAt: number;
  trashed: boolean;
}
```

### 3.6 `DomainModel` [RESTRICTED]
The full aggregate root. JS never sees this. Mutations use specific payloads.

**Rust:**
```rust
pub struct DomainModel {
    pub uuid: Uuid,
    pub enc_key_gen: u64,
    pub overview: DecryptedOverview,
    pub secrets: DecryptedSecret,
    pub metadata: ItemMetadata,
}
```

---

## 4. The Mutation Security Contract (The User-Input Exception)

The architecture mandates that `DecryptedSecret` never crosses the JS bridge. However, when a user *creates* or *edits* a secret, the JS layer inherently holds that plaintext state temporarily. 

This is the **User-Input Exception**. It is governed by strict rules to minimize the attack window:

1.  **JS State Isolation:** The React UI must hold user-inputted secrets in isolated, non-serializable state (e.g., uncontrolled inputs or secure refs), **not** in the global Zustand/Redux store.
2.  **Direct Transmission:** When the user clicks "Save", the JS layer passes the plaintext strings directly to the Turbo Module via the `SaveItemPayload`.
3.  **Native Zeroization:** The Turbo Module (Swift/Kotlin) receives these strings, **immediately** constructs the UniFFI `Zeroizing<String>` wrappers, and explicitly nullifies the incoming string references.
4.  **JS Heap Clear:** The JS component must immediately clear its local state/refs upon receiving the `TaskReceipt` from the Turbo Module.

### 4.1 `SaveItemPayload`

**TypeScript (Input to Turbo Module):**
```typescript
export interface SaveItemPayload {
  uuid: string;
  syncEpoch: string; // u64 as string
  
  // Overview fields
  title: string;
  subtitle: string;
  iconKey: string;
  urls: string[];

  // Secret fields (The User-Input Exception)
  password?: string;
  totp?: {
    algorithm: 'Sha1' | 'Sha256' | 'Sha512';
    digits: number; // u8
    period: number; // u8
    secretBase32: string;
  };
  notes?: string;
  customFields?: Array<{
    name: string;
    value: string;
    type: 'Text' | 'Hidden';
  }>;
}
```
*Native Layer Responsibility:* The Turbo Module parses the `syncEpoch` string, constructs the `DomainModel`, maps the plain strings into `Zeroizing<String>`, and calls the UniFFI `save_item` method.

---

## 5. Command & Mutation Types

### 5.1 `SaveCommand`
**Rust:** (Constructed by Native Layer from `SaveItemPayload`)
```rust
pub struct SaveCommand {
    pub item: DomainModel,
    pub sync_epoch: u64,
}
```

### 5.2 `DeleteCommand`
**Rust:**
```rust
pub struct DeleteCommand {
    pub uuid: Uuid,
    pub sync_epoch: u64,
}
```
**TypeScript:**
```typescript
export interface DeleteItemPayload {
  uuid: string;
  syncEpoch: string; // u64 as string
}
```

### 5.3 `RevertibleState`
Used for surgical UI reverts on `MutationFailed`.

**Rust:**
```rust
pub enum RevertibleState {
    Saved(DomainModel),
    Deleted(DecryptedOverview),
}
```
**TypeScript (Discriminated Union):**
```typescript
// SECURITY CONTRACT: This type ONLY reverts the list (Overview).
// It does NOT contain the DecryptedSecret. 
// The UI Detail View must revert its local edit state independently 
// upon receiving a MutationFailed event.
export type RevertibleState =
  | { type: 'Saved'; overview: DecryptedOverview }
  | { type: 'Deleted'; overview: DecryptedOverview };
```

---

## 6. Client API Boundary Types

### 6.1 Opaque Handles & Receipts

| Rust Type | UniFFI | Turbo Module TS Type | Notes |
| :--- | :--- | :--- | :--- |
| `SecretHandle` (`u64`) | `Long` / `Int64` | `string` | Passed as string to JS. |
| `TaskReceipt` (`u64`) | `Long` / `Int64` | `string` | Passed as string to JS. |

### 6.2 `CoreAction`
Delegated to the Platform Adapter to prevent JS heap leaks.

**Rust:**
```rust
pub enum CoreAction {
    CopyToClipboard { handle: SecretHandle },
    Autofill { handle: SecretHandle },
}
```
**TypeScript:**
```typescript
export type CoreAction =
  | { type: 'CopyToClipboard'; handle: string }
  | { type: 'Autofill'; handle: string };
```

### 6.3 `VaultStateUpdate`
The reactive event stream pushed from Core to UI.

**Rust:**
```rust
pub enum VaultStateUpdate {
    SyncStarted,
    SyncProgress(u8), // 0-100 integer percentage
    SyncCompleted,
    SyncFailed(CoreError),
    OverviewUpserted(DecryptedOverview),
    OverviewDeleted(Uuid),
    ConflictDetected(ConflictEvent),
    KeyUpdateRequired,
    VaultLocked,
    NewerVersionAvailable { uuid: Uuid },
    MutationSucceeded(TaskReceipt),
    MutationFailed { receipt: TaskReceipt, error: CoreError, original_state: RevertibleState },
}
```
**TypeScript (Discriminated Union):**
```typescript
export type VaultStateUpdate =
  | { type: 'SyncStarted' }
  | { type: 'SyncProgress'; progressPercentage: number } // 0-100 integer
  | { type: 'SyncCompleted' }
  | { type: 'SyncFailed'; error: CoreError }
  | { type: 'OverviewUpserted'; overview: DecryptedOverview }
  | { type: 'OverviewDeleted'; uuid: string }
  | { type: 'ConflictDetected'; event: ConflictEvent }
  | { type: 'KeyUpdateRequired' }
  | { type: 'VaultLocked' }
  | { type: 'NewerVersionAvailable'; uuid: string }
  | { type: 'MutationSucceeded'; receipt: string }
  | { type: 'MutationFailed'; receipt: string; error: CoreError; originalState: RevertibleState };
```

---

## 7. Core & Sync State Types

### 7.1 `DashMapState` & `DashMapEntry` [INTERNAL]
Used by the Core Architecture's Context-Aware Resolution. Not exposed to JS.

### 7.2 `ConflictEvent`
Payload for 412 Resolution UI.

**Rust:**
```rust
pub struct ConflictEvent {
    pub uuid: Uuid,
    pub local_version: u64,
    pub server_version: u64,
    pub is_toxic: bool,
}
```
**TypeScript:**
```typescript
export interface ConflictEvent {
  uuid: string;
  localVersion: string; // u64 as string
  serverVersion: string; // u64 as string
  isToxic: boolean;
}
```

---

## 8. Error Contracts

Errors must be granular to drive correct UI behavior (e.g., re-auth vs. toxic alert).

### 8.1 `CoreError`

**Rust:**
```rust
pub enum CoreError {
    CryptoError(CryptoError),
    DbError(DbError),
    EpochMismatch,
    HandleInvalid,
    NetworkError(String),
}
```
**TypeScript:**
```typescript
export type CoreError =
  | { type: 'CryptoError'; details: CryptoError }
  | { type: 'DbError'; details: DbError }
  | { type: 'EpochMismatch' }
  | { type: 'HandleInvalid' }
  | { type: 'NetworkError'; message: string };
```

### 8.2 `CryptoError`
From the Cryptographic Specification.

**Rust:**
```rust
pub enum CryptoError {
    TagMismatch,
    KeyGenMismatch,
    MalformedCiphertext,
}
```
**TypeScript:**
```typescript
export type CryptoError =
  | { type: 'TagMismatch' }
  | { type: 'KeyGenMismatch' }
  | { type: 'MalformedCiphertext' };
```

### 8.3 `DbError`

**Rust:**
```rust
pub enum DbError {
    ConstraintViolation(String),
    Corruption(String),
    IoError(String),
}
```
**TypeScript:**
```typescript
export type DbError =
  | { type: 'ConstraintViolation'; message: string }
  | { type: 'Corruption'; message: string }
  | { type: 'IoError'; message: string };
```

### 8.4 `AuthError`

**Rust:**
```rust
pub enum AuthError {
    InvalidCredentials,
    BiometricUnavailable,
    KeystoreError(String),
}
```
**TypeScript:**
```typescript
export type AuthError =
  | { type: 'InvalidCredentials' }
  | { type: 'BiometricUnavailable' }
  | { type: 'KeystoreError'; message: string };
```
