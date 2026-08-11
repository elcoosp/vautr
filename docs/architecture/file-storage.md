# Vautr Secure File Storage & Attachment Architecture

This document defines the exact cryptographic mechanisms, streaming pipelines, and server protocols required to securely store and sync large binary files (documents, photos, SSH keys) within the Vautr ecosystem.

The standard Vautr sync pipeline is optimized for small, atomic JSON payloads. Using it to sync a 100MB PDF would result in base64 bloat, OOM crashes on WASM/Mobile, SQLite WAL exhaustion, and UI thread starvation. This specification defines a dedicated, memory-bounded, streaming fast-path for binary data that upholds the Zero-Knowledge guarantee without violating the application's RAM constraints or creating sync race conditions.

Deviations from this specification will result in application crashes, server I/O saturation, and severe degradation of the interactive sync experience.

---

## 1. The Binary Boundary (The RAM Limit)

vautr enforces a strict separation between textual metadata and binary attachments to guarantee memory safety.

1.  **The 10MB Rule:** Attachments smaller than 10MB MAY be encrypted and stored as inline BLOBs within the standard `DomainModel` payload. Attachments >= 10MB **MUST** use the Streaming File Pipeline defined in this document.
2.  **Zero-RAM Encryption:** The Streaming Pipeline must never hold the entirety of a plaintext or ciphertext file in application memory. Encryption and decryption must occur in bounded memory chunks.
3.  **Independent Sync Lifecycle:** File transfers are long-running, resumable background tasks. They are strictly decoupled from the interactive `PersistenceWorker` and standard OCC sync loop to prevent queue starvation.

---

## 2. Cryptographic Chunking & Streaming Encryption

To enable resumable uploads/downloads and bound memory usage, files are split into fixed-size chunks, each encrypted independently using XChaCha20-Poly1305.

### 2.1 Chunking Parameters
*   **Chunk Size:** 1 MiB (1,048,576 bytes).
*   **Rationale:** A 1MB chunk bounds peak RAM usage to ~2MB (plaintext + ciphertext buffer) per concurrent transfer. It is small enough for WASM linear memory and mobile devices, yet large enough to avoid excessive network overhead and S3 PUT requests.

### 2.2 Streaming AEAD Construction
To parallelize encryption and ensure integrity, each chunk is treated as an independent AEAD message.
*   **Key Derivation:** A unique File Encryption Key (FEK) is derived for each attachment: `FEK = HKDF(SVK, info="vautr-fek-{file_uuid}")`.
*   **Nonce Management:** XChaCha20 requires a strictly 24-byte (192-bit) nonce. To ensure determinism for resumability while preventing collisions across files and chunks, the nonce is derived statelessly: `Nonce = SHA-256(file_uuid_bytes || chunk_index.to_be_bytes())[..24]`. This guarantees a unique, 24-byte nonce per chunk without requiring persistent state.
*   **Associated Data (AD):** `construct_ad(file_uuid, enc_key_gen, chunk_index)`. This binds the chunk to its specific file, vault epoch, and position, preventing chunk reordering or insertion attacks by a malicious server.
*   **Format:** `Ciphertext = Chunk_Nonce || Encrypted_Data || Auth_Tag`.

### 2.3 The FileManifest & Attachment State
To assemble the chunks, the client requires metadata. This is stored as a JSON structure within the `DomainModel`'s standard payload. Critically, the manifest tracks the upload state to prevent sync race conditions.

```rust
pub struct FileManifest {
    pub file_uuid: Uuid,
    pub total_size: u64,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub enc_key_gen: u64,
    pub content_type: String,
    pub last_modified: i64,
    pub status: AttachmentState,
}

pub enum AttachmentState {
    PendingUpload,  // Local only; chunks are uploading to S3. NOT synced to server.
    Available,      // Upload complete; chunks exist on S3. Synced to server.
}
```

---

## 3. Local Persistence (The Cold Storage Boundary)

Encrypted file chunks bypass the SQLite database entirely to prevent WAL bloat and I/O overhead.

### 3.1 Filesystem Layout
*   **Desktop/Mobile:** Encrypted chunks are stored in a sandboxed application directory: `{AppSandbox}/vautr_files/{file_uuid}/chunk_{index}.enc`.
*   **WASM/Web:** Bounded by browser security, the WASM core uses the Origin Private File System (OPFS) via `wasm-bindgen` to create an identical directory structure, ensuring cross-platform code reuse.

### 3.2 Garbage Collection
When an item is deleted or an attachment is removed, the standard `PersistenceWorker` enqueues a `FileCleanupTask`.
1.  **Task:** Deletes the `{file_uuid}` directory and all associated chunk files.
2.  **Crash Safety:** If the app crashes before cleanup, a startup GC scan compares directories in `vautr_files/` against active `FileManifest` entries in SQLite. Orphaned directories are deleted automatically.

---

## 4. Server Protocol (The Multipart Gateway)

The Vautr server acts as a dumb, encrypted blob store, offloading large file storage to S3-compatible object storage via presigned URLs. The server never possesses the FEK.

### 4.1 Multipart Upload (Resumable & Race-Free)
To prevent lagged devices from downloading manifests for files that haven't finished uploading to S3, the upload and sync commit are strictly ordered.

1.  **Initiate:** Client calls `POST /files/{file_uuid}/upload/initiate` with the `FileManifest` (status `PendingUpload`). Server creates an S3 multipart upload session and returns an `upload_id` and a `Vec<PresignedUrl>` for each chunk.
2.  **Stream Upload:** The `FileTransferWorker` streams local encrypted chunks directly to S3 via `PUT {presigned_url}`.
3.  **Resumption:** If interrupted, the client calls `GET /files/{file_uuid}/upload/status` to see which parts S3 received, resuming from the missing chunk.
4.  **Commit:** Once all chunks are pushed, the client calls `POST /files/{file_uuid}/upload/complete`. The server assembles the S3 object and atomically marks the file as ready.
5.  **Sync Manifest:** Only *after* the commit succeeds, the client updates the local `FileManifest` status to `Available` and enqueues a standard `save_item` via the `PersistenceWorker`. This ensures the server only ever receives manifests for fully realized files.

### 4.2 On-Demand Download
Mobile and Web clients do **not** auto-sync file attachments during standard `GET /sync/pull` to preserve storage and bandwidth. They are downloaded on-demand.
1.  **Request URLs:** User clicks "Download". Client calls `GET /files/{file_uuid}/download`. Server returns a `Vec<PresignedUrl>` for the chunks.
2.  **Stream Download:** The `FileTransferWorker` streams chunks directly from S3 to the local `vautr_files` directory.
3.  **Verification:** After download, the worker verifies the total size and the AEAD tag of the final chunk to ensure completeness.

---

## 5. UX Interaction & The FileTransferWorker

The standard UI event loop cannot handle tasks that take minutes. File transfers are managed by a dedicated worker.

### 5.1 The FileTransferWorker
*   **Queue:** A separate async task queue from the `PersistenceWorker`.
*   **Concurrency:** Max 2 concurrent file transfers (up or down) to avoid saturating the network.
*   **State:** Maintains a state machine (Pending -> Streaming -> Paused -> Completed/Failed) persisted locally to survive app restarts.

### 5.2 Progress Tracking & UI Events
The worker emits granular, non-blocking events to the UI.
*   **Event:** `VaultStateUpdate::FileTransferProgress { file_uuid: Uuid, bytes_transferred: u64, total_bytes: u64 }`.
*   **Throttling:** The worker throttles UI events to a maximum of 4 updates per second to prevent React/WASM render loop starvation.

### 5.3 Backgrounding & Network Loss
*   **Network Loss:** If the socket disconnects, the `FileTransferWorker` pauses the stream. It waits for network restoration and queries the server's resumption endpoint before continuing.
*   **App Backgrounding:** The Platform Layer ensures the `FileTransferWorker` is the last task suspended and the first resumed, utilizing OS background transfer APIs (NSURLSession / WorkManager) where possible to complete uploads even if the UI is killed.

### 5.4 Desktop Auto-Sync
Desktop clients (GPUI) possess larger storage and persistent connections. The `FileTransferWorker` on Desktop automatically downloads all `FileManifest` attachments in the background after a standard `SyncCompleted` event, ensuring local offline availability.
