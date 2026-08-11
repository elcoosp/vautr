# Vautr Data Import & Vault Seeding Architecture

This document defines the exact pipelines, parsing strategies, and high-throughput ingestion mechanisms required to migrate user data from competing password managers into Vautr. 

The standard Vautr mutation pipeline (Optimistic UI -> PersistenceWorker -> Single Sync) is optimized for interactive, low-latency edits. Using it to import 1,000 items would result in 1,000 UI re-renders, 1,000 SQLite transactions, and 1,000 network requests, severely degrading performance and exhausting server rate limits. This specification defines a specialized, high-throughput fast-path that guarantees a seamless onboarding experience without violating cryptographic boundaries, overloading system memory, or causing runtime paradoxes.

Deviations from this specification will result in UI freezes, OOM crashes on large CSVs, cryptographic namespace collisions, and failed migrations.

---

## 1. The Import Philosophy (The UX Boundary)

1.  **Zero-Trust Parsing:** Assume all imported data is malformed, maliciously crafted, or structurally broken. The parser must never panic.
2.  **Atomic Visual Streaming:** An import of 1,000 items must not create 1,000 UI state updates. The UI displays a single, determinate progress bar mapping to the bulk pipeline stages.
3.  **Graceful Degradation:** If 5 items out of 1,000 fail validation, the 995 successful items must still be imported. The pipeline does not halt on individual errors; it accumulates them.
4.  **Dedicated Fast-Path:** Imports bypass the standard `PersistenceWorker` queue and `VaultStateUpdate` event loop to prevent queue starvation for interactive user edits.
5.  **Strict Namespace Isolation:** Competitor item identifiers are inherently unsafe for Vautr's cryptographic context. Vautr must enforce its own UUID namespace.

---

## 2. Supported Formats & The Extraction Pipeline

Vautr supports the most common export formats, parsing them in a memory-bounded streaming fashion. Because exports can be compressed or encrypted, the pipeline requires a cross-platform extraction/authentication phase before streaming begins.

### 2.1 Format Matrix & The `ImportSource` Trait
*   **1Password** (`.1pux`): ZIP archive containing JSON.
*   **Bitwarden** (`.json`): Encrypted or Plain JSON.
*   **Chrome/Safari/Firefox** (`.csv`): Plain text.
*   **Vautr Backup** (`.vautr`): Native encrypted backup (handled via standard Sync flow, not this import pipeline).

To handle ZIP archives without violating O(1) memory on WASM, the Core defines a cross-platform `ImportSource` trait:
*   **Desktop/Mobile:** The implementation takes a file path, uses the `zip` crate to extract the inner JSON to a sandboxed temp directory, and returns a `File` handle for streaming.
*   **WASM/Web:** The implementation takes a JS `File` object. It uses `wasm-bindgen` to stream-read the ZIP archive via chunked `ArrayBuffer` transfers, decompressing on-the-fly in memory using a streaming `DeflateDecoder`, yielding the inner JSON bytes directly to the parser. No full archive is ever held in memory.

### 2.2 The Encrypted Export Auth Boundary
If the user selects an encrypted Bitwarden JSON or 1Password account key export, the pipeline pauses before parsing.
1.  **Prompt:** The UI presents a secure modal requesting the user's Bitwarden/1Password Master Password.
2.  **Derive:** The Core derives the decryption key using the competitor's specific KDF parameters.
3.  **Stream-Decrypt:** The streaming parser decrypts the file on-the-fly into memory. The competitor MP is immediately zeroized.

### 2.3 The Streaming Parser, UUID Mandate, & Deduplication

*   **Tool:** `serde_json::StreamDeserializer` (Rust) / `csv::Reader` (Rust).
*   **Flow:** The file is read in chunks. The stream yields `RawImportItem` structs one by one.
*   **The UUID Mandate:** The Translation Layer maps the `RawImportItem` to a Vautr `DomainModel`. It **MUST** discard the competitor's UUID/ID and generate a cryptographically secure `Uuid::new_v4()`. Preserving competitor UUIDs risks namespace collisions, breaks AEAD Associated Data (AD) context binding, and is strictly forbidden. The original competitor ID is preserved in a dedicated, non-indexed field: `DomainModel::metadata::import_source_id`.
*   **HashSet Deduplication:** To prevent importing duplicates, the Translation Layer checks incoming items against an exact-match set.
    1.  *Pre-Flight Query:* Before the fast-path begins, the Core performs a single `SELECT title, primary_url FROM item_overviews` to build an in-memory `HashSet<(String, String)>`.
    2.  *Check:* The Translation Layer queries this HashSet. If a match exists, the item is flagged as a `DuplicateSkip` (or `DuplicateOverwrite` based on user UX selection) and handled accordingly. FTS5 is never used for deduplication.

---

## 3. The Bulk Ingestion Pipeline (High-Throughput Core)

This is the critical performance path. Once translated into `DomainModel` structs, the items must be encrypted and persisted locally at hardware speed.

### 3.1 Size-Aware Parallel Encryption (`rayon`)
Encrypting 1,000 items sequentially on a single thread is slow. Vautr uses data parallelism, but strictly bounds memory consumption to prevent OOM on attachment-heavy vaults.

*   **Tool:** `rayon` crate.
*   **Strategy:** The stream of valid `DomainModel` structs is collected into chunks. 
*   **Size-Aware Chunking:** A chunk is closed and dispatched to the encryption pool if it reaches **100 items OR 25MB of total plaintext size**. This prevents a 100-item batch of 5MB photos from consuming 500MB of RAM.
*   **Execution:** Each item is encrypted using its own unique XChaCha20-Poly1305 nonce (via `OsRng`) and the current OEK/DEK. The output is a `Vec<(ItemOverview, ItemPayload)>` of encrypted ciphertexts.
*   **Safety:** Because each item uses a unique, random nonce, parallel encryption is mathematically safe and introduces no AEAD nonce-misuse risks.

### 3.2 Bulk SQLite Ingestion (The Drop/Rebuild Strategy)
SQLite does not natively support disabling FTS5 sync triggers. Dropping and recreating triggers programmatically is fragile. To achieve maximum bulk I/O throughput, Vautr uses a drop/rebuild strategy.

1.  **Drop FTS5 Index:** The Core executes `DROP TABLE IF EXISTS items_fts`.
2.  **Single Massive Transaction:** The entire import batch is wrapped in a single `BEGIN IMMEDIATE` / `COMMIT` transaction.
3.  **Bulk Insert:** SeaORM `insert_many()` is used to bulk insert the `ItemOverview` and `ItemPayload` entities.
4.  **Commit & Recreate:** After the transaction commits, the Core executes the migration script to `CREATE VIRTUAL TABLE items_fts` and then `INSERT INTO items_fts(items_fts) VALUES ('rebuild')` to atomically rebuild the search index from scratch.
5.  **Performance:** This reduces a 5-second interactive I/O storm into a ~50ms bulk transaction and index rebuild.

### 3.3 UI Event Suppression
During the ingestion pipeline, the Core suppresses the standard `OverviewUpserted` events.
*   **Action:** The `ImportWorker` communicates directly with the local DB, bypassing the `DashMap` and the standard event bus.
*   **Completion Event:** Once the SQLite bulk transaction commits and the FTS5 rebuild finishes, the Core fires a single `VaultStateUpdate::ImportCompleted(count)` event.
*   **UI Response:** The UI receives this single event, calls `client.search("")` to fetch the newly populated list, and renders the vault instantly.

---

## 4. The First Sync (Seeding the Server)

With 1,000 items securely in the local SQLite DB, they must be pushed to the server.

### 4.1 Batch Chunking & Concurrency
The `ImportWorker` hands off the pending UUIDs to the `SyncEngine`.
*   **Chunking:** The SyncEngine queries the local DB for items where `local_version != server_version`. It chunks them into batches of 100.
*   **Controlled Concurrency:** It dispatches up to 3 concurrent `POST /sync/push-batch` requests. This saturates the network link without overwhelming the server or exhausting the mobile/WASM connection pool.

### 4.2 Rate Limiting & Backoff
If the server returns `429 Too Many Requests`:
*   **Action:** The SyncEngine applies exponential backoff across all concurrent batch streams. 
*   **Resume:** Because the local SQLite DB tracks `local_version` and `server_version`, the sync engine can resume seeding exactly where it left off, even if the app is killed.

---

## 5. Error Reporting & Partial Success

The UI must clearly communicate the results of a messy import without overwhelming the user.

### 5.1 The Import Report
When the pipeline completes, it returns a strongly typed `ImportReport` struct across the FFI boundary.

```rust
pub struct ImportReport {
    pub total_parsed: u32,
    pub success_count: u32,
    pub skipped_count: u32,
    pub errors: Vec<ImportError>,
}

pub struct ImportError {
    pub line_number: u32,
    pub item_identifier: String, // e.g., "Title: Bank of America"
    pub reason: ImportFailureReason,
}

pub enum ImportFailureReason {
    SchemaMismatch(String),
    ValidationFailed(String),
    MissingRequiredField(String),
    DuplicateSkip,          // Skipped due to existing Title/URL match
    DecryptionFailed,       // Wrong competitor MP provided
}
```

### 5.2 UX Interaction
1.  **Progress Bar:** The UI displays a determinate progress bar based on the streaming parser's position, segmented into "Parsing", "Encrypting", and "Syncing".
2.  **Success State:** If `skipped_count == 0`, display: "Successfully imported 1,000 items."
3.  **Partial Success State:** If `skipped_count > 0`, display: "Imported 995 items. 5 items were skipped." Provide a "View Details" button that expands the `ImportError` vector in a scrollable modal.
4.  **No Blocking:** The user can navigate away from the Import screen during the "Syncing" phase. The `SyncEngine` will continue pushing batches in the background.
