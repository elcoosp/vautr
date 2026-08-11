# Vautr UI State Charts & Interaction Specification (v3.0)

This document defines the exact user experience, state transitions, and security interaction patterns for the Vautr Client. The UI is treated as a thin, deterministic projector of the Core’s state machine. It enforces zero-knowledge boundaries at the component level, mandates explicit resource disposal, and strictly accounts for the asynchronous realities of React lifecycles and Rust FFI boundaries.

This specification bridges the gap between cryptographic architecture and human perception, ensuring that state-of-the-art security feels instantaneous, honest, and frictionless.

---

## 1. Philosophical Pillars

1.  **Radioactive Material:** Decrypted secrets are radioactive. View, use, immediately dispose. The UI never hoards secrets.
2.  **Progressive Disclosure:** Data is only decrypted at the absolute last millisecond (on-view, on-copy). List rendering never triggers decryption.
3.  **Honest Context:** The UI does not lie to the user. If an item is conflicted or toxic, the UI surfaces this context honestly but calmly, avoiding panic while preserving user intent.
4.  **Optimistic by Default, Pessimistic on Failure:** UI updates instantly on user action. If the Core rejects the action (OCC/Epoch mismatch), the UI surgically reverts without destroying the user's ongoing workflow.
5.  **Strict Lifecycle Separation:** Visual state (masked/unmasked) is completely decoupled from memory state (DashMap handle allocation). Visual timers never trigger memory zeroization; only component unmounting does.
6.  **Ephemeral Drafts:** User input is treated as highly sensitive, temporary state. It is persisted only long enough to survive asynchronous Core validation, after which it is strictly zeroized.

---

## 2. Application Lifecycle & Security Gates

### The Foreground/Background Transition
To balance security with UX, the app uses a tiered timeout system enforced by the **Platform Layer** (Swift/Kotlin), not the periodic Safety Reaper.

*   **0 - 30 Seconds Backgrounded:** App resumes instantly. No prompt.
*   **30 Seconds - 5 Minutes Backgrounded:** App requires Biometric unlock (FaceID/TouchID). The Platform Layer intercepts the OS resume event, prompts biometrics, unwraps the SVK, and calls `core.unlock_with_raw_key`. <100ms.
*   **> 5 Minutes Backgrounded (or Biometric Failure):** App requires full Master Password unlock.
*   **Enforcement:** On OS backgrounding event, the Platform Layer explicitly calls `core.release_secret` for any active handles or `core.lock()` for deeper backgrounds. The Core's Safety Reaper acts *only* as a secondary failsafe for app crashes, not OS lifecycle transitions. If a user was viewing a password and switches apps, the password view is instantly masked upon return.

### The Read-Only Gate (`KeyUpdateRequired`)
When a lagged device detects a server-side key rotation, the Core pushes `VaultStateUpdate::KeyUpdateRequired`. 

*   **UX Implementation:** A persistent, non-blocking banner drops from the top: *"Vault updated. Re-authenticate to save changes."*
*   **Allowed Actions:** User can still search, view, and copy passwords. The UI remains read-functional.
*   **Blocked Actions:** "Save", "Create", and "Delete" buttons are disabled/greyed out.
*   **Resolution Flow:** Tapping the banner opens the Master Password modal. Upon successful `unlock_with_password`, the Core syncs the new SVK, pushes `VaultStateUpdate::OverviewUpserted` for rotated items, and the UI seamlessly re-enables mutations.

---

## 3. The Opaque Handle Lifecycle (The Security Core)

This maps the strictest security boundary to UI component lifecycles, strictly separating visual masking from memory zeroization.

### The View Flow (Reveal & Mask)
1.  **Trigger:** User taps the Eye icon next to a password field.
2.  **Action:** UI calls `core.reveal_secret(uuid)`.
3.  **State:** UI receives a `SecretHandle` (stored in a React `useRef`, *never* in global Zustand state).
4.  **Render:** The UI passes the `handle` to the `<SecretTextView nativeID="handle" />` component. The Native Module intercepts this, calls `read_secret(handle)`, and renders the text directly over the React canvas. The JS heap never sees the string.
5.  **Auto-Mask (Visual Only):** A 30-second local timer starts. After 30 seconds, the Native Overlay animates back to masked dots. **Crucial:** This does *not* call `release_secret`. The handle remains valid in memory in case the user copies or interacts.

### The Disposal Flow (The Unmount Contract)
The most critical UX security rule: **Unmounting strictly equals Zeroization.**

*   **Enforcement:** The `<SecretTextView>` component uses a strict `useEffect` cleanup.
    ```javascript
    useEffect(() => {
      return () => {
        if (handleRef.current) {
          VautrCore.releaseSecret(handleRef.current);
          handleRef.current = null;
        }
      };
    }, []);
    ```
*   **Triggers:** Navigating away, closing the detail view, or the Platform Layer triggering a lock on OS background instantly fires this cleanup, wiping the Core memory.

### The Silent Copy Flow (Zero-String Touch)
Used when copying a password that is not currently visible on screen.
1.  **Trigger:** User taps the Copy icon on a masked field.
2.  **Action:** UI calls `core.reveal_secret(uuid)` to obtain a `SecretHandle`.
3.  **Delegation:** UI immediately calls `core.perform_action(CoreAction.CopyToClipboard { handle })`. The Native Module writes directly to the OS clipboard.
4.  **Immediate Disposal:** UI immediately calls `core.release_secret(handle)`.
5.  **Feedback:** UI shows a brief "Copied" toast.
*Result:* The Native Overlay never mounts. The string never touches the React state. The handle lifetime is milliseconds.

---

## 4. Optimistic UI & Draft Recovery

The UI updates instantly, relying on the Core to validate asynchronously. Because React component local state (`useState`) is destroyed on unmount, we cannot rely on it to survive async failures. We use a secure, ephemeral Draft mechanism.

### The Optimistic Save Flow
1.  **User Action:** User edits a password and clicks "Save".
2.  **Core Call:** The UI calls `core.save_item(...)`. The Core queues the task to the `PersistenceWorker` and immediately returns a `TaskReceipt` (u64).
3.  **Draft Persistence:** Upon receiving the `TaskReceipt`, the UI saves the user's typed plaintext (the "Draft") into an isolated, secure in-memory Zustand slice, keyed by the `TaskReceipt` (u64).
4.  **Immediate UI:** The UI optimistically updates its local Zustand normalized map. The Detail View unmounts (pops back to the list).

### The Draft Recovery Flow (`MutationFailed`)
If the Core pushes `VaultStateUpdate::MutationFailed { receipt, error, original_state }` after the user has already navigated away:

1.  **List Revert:** The UI matches the `receipt` (u64). It extracts `original_state`.
    *   If `RevertibleState::Saved(overview)`, it replaces the item in the Zustand map with the old overview.
    *   If `RevertibleState::Deleted(overview)`, it re-inserts the overview into the map.
2.  **Draft Injection:** The UI retrieves the failed Draft from the secure Zustand slice using the `receipt` (u64).
3.  **UX Recovery:** The UI navigates the user back to a *new instance* of the Detail View. It injects the recovered Draft into the new component's local state.
4.  **Feedback:** The Detail View opens in "Edit Mode" with the user's typed text fully restored, showing a red alert: *"Save failed: [Error Message]. Your local changes are preserved."*

### The Draft Zeroization Contract
Drafts contain plaintext secrets and must be strictly disposed of.

1.  **On Success:** When the UI receives `VaultStateUpdate::MutationSucceeded(receipt)`, it immediately deletes the Draft associated with that `receipt` from the Zustand slice, and explicitly overwrites the string buffers.
2.  **On Lock:** When the UI receives `VaultStateUpdate::VaultLocked`, the entire Draft Zustand slice is atomically wiped and zeroized. No plaintext input survives a vault lock event.

---

## 5. Sync & Conflict Resolution (The Honesty Engine)

The Core Architecture uses a Stateful DashMap to prevent infinite loops. The UI maps these states to passive indicators and honest modals.

### The `ValidIgnored` Indicator (Lifting the Blindfold)
If a lagged device re-authenticates and syncs, the server has a newer, valid version. The DashMap marks it `ValidIgnored`.

*   **UX Implementation:** The list item receives a subtle, animated blue dot or a "Synced newer version" pill. It does *not* pop up a blocking alert.
*   **Detail View:** The detail header shows a banner: *"A newer version is available on another device. [Restore Server Version]"*.
*   **Action:** Tapping "Restore" calls `core.save_item(...)` to overwrite local with server, and the Core removes the UUID from the DashMap.

### The 412 Conflict Modal (Toxic vs. Valid)
If the user edits an ignored item and tries to save, and the server returns `412 Precondition Failed`:

*   **Valid Conflict (`is_toxic == false`):**
    *   *Modal:* "This item was updated on another device with newer data. How would you like to proceed?"
    *   *Options:* `[Keep Server Version]` / `[Force Overwrite with Local]`
*   **Toxic Conflict (`is_toxic == true`):**
    *   *Modal:* "Your edit conflicts with an unreadable update (possibly from a rotated key). How would you like to proceed?"
    *   *Options:* `[Keep Local]` / `[Overwrite Server]`
*   **Resolution:** The UI sends the choice to the Core. The Core executes the OCC curing and removes the item from the DashMap.

---

## 6. Reactive State Subscriptions (The Event Map)

The single source of truth for how Core events map to UI actions. The UI subscribes to `core.watch_state()` via a dedicated Zustand middleware.

| `VaultStateUpdate` Event | UI State Transition / Action | UX Indicator |
| :--- | :--- | :--- |
| **`SyncStarted`** | Set `isSyncing: true` in Zustand. | Subtle pulse on the sync icon. |
| **`SyncProgress(progressPercentage)`** | Update `syncProgress` state. | Linear progress bar under the header. |
| **`SyncCompleted`** | Set `isSyncing: false`, `syncProgress: 0`. | Sync icon turns solid green for 2s. |
| **`SyncFailed(error)`** | Set `isSyncing: false`. | Red banner: *"Sync failed. Will retry automatically."* |
| **`OverviewUpserted(overview)`** | Upsert `overview` into `items` normalized map. | List instantly re-renders/animates the change. |
| **`OverviewDeleted(uuid)`** | Remove `uuid` from `items` map. | Item animates out of the list. |
| **`KeyUpdateRequired`** | Set `isReadOnly: true`. | Drop-down persistent banner: *"Re-authenticate to save."* |
| **`VaultLocked`** | Clear `items` map. **Wipe Draft Slice.** Set `isLocked: true`. | Immediate transition to Lock Screen. |
| **`NewerVersionAvailable { uuid }`** | Update `uuid` metadata in map to flag `hasUpdate: true`. | Blue dot/pill appears on list item and detail view. |
| **`MutationSucceeded(receipt)`** | Discard `receipt` state. **Delete Draft for `receipt`.** | Brief green flash on the saved item. |
| **`MutationFailed { receipt, error, original_state }`** | Revert Zustand `items` map using `original_state`. Trigger Draft Recovery Flow. | Red toast alert. Navigates user back to edit view with Draft injected. |

### Search Interaction (Local FTS5)
*   **Trigger:** User types in search bar.
*   **Debounce:** 30ms debounce. Because the query executes against a local SQLite FTS5 index via a high-performance Rust core, 150ms is too sluggish. 30ms provides native-feeling instant results without causing UI jank on rapid keystrokes.
*   **Action:** Call `core.search(query)`.
*   **Result:** Returns `Vec<DecryptedOverview>`. The UI replaces the list view state with these results. If the query is empty (`""`), it calls `core.search("")` which returns the top 50 recent items.

---

## 7. The First Launch / Vault Creation Flow

The most critical UX moment for Zero-Knowledge apps.

1.  **Email & MP Entry:** User enters email and chooses a Master Password.
2.  **Strength Meter:** Real-time visual feedback (Crack-time estimate, e.g., "Centuries").
3.  **KDF Calibration:** The Core runs a background calibration of Argon2id to target ~300ms derivation time on the specific device.
4.  **Derivation:** Core derives MK, KEK, and SVK. Generates random Salt.
5.  **OPAQUE Registration:** Core executes the PAKE flow with the server.
6.  **Success:** Vault is unlocked. User lands on the empty state screen.
