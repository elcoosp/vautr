# Vautr Server API Contract

This document defines the exact HTTP REST contract between the Vautr Client Core and the Vautr Server. The server acts as an untrusted, zero-knowledge encrypted blob store with strict Optimistic Concurrency Control (OCC). 

This specification enforces the Core Architecture's I/O bounding, crash-safe rotation, deterministic epoch gating, bulk write efficiency, and distributed observability.

---

## 1. Architectural Principles

1.  **Zero-Knowledge Enforcement:** The server MUST NEVER possess fields that decrypt PII. It only stores UUIDs, versions, encryption key generations (`enc_key_gen`), and opaque base64 ciphertext blobs.
2.  **Strict OCC:** All mutable endpoints require an `If-Match` header (or equivalent batch payload field) containing the u64 version number. All successful mutations return the new u64 version.
3.  **Metadata-First Sync & I/O Bounding:** The server MUST separate unencrypted metadata from encrypted payloads. The client evaluates metadata against the DashMap *before* downloading payloads, strictly bounding I/O on toxic/ignored items.
4.  **Epoch-Gated Writes:** The server enforces the global `min_enc_key_gen`. Any write attempt with an older `enc_key_gen` is rejected.
5.  **Exact-Version Payload Fetching:** Payloads are fetched by specifying the exact expected version to prevent AEAD race conditions.
6.  **JSON Integer Precision:** All u64 integers are transmitted as JSON numbers.

---

## 2. Global HTTP Headers, Limits & Cursors

### Observability Headers
*   **`X-Request-ID`**: Client-generated UUID. Sent on *all* requests.
*   **`X-Correlation-ID`**: Server-generated UUID. Returned on *all* responses.

### Bounding Limits
*   **`POST /sync/pull-payloads` Limit:** Max **100** items. 
*   **`POST /sync/push-batch` Limit:** Max **100** items.

### Cursor Expiration
Sync cursors represent a point-in-time state of the server's database. If a device is offline for an extended period, the server may prune its delta history.
*   If a client requests a `cursor` that the server no longer has history for, the server MUST return a `410 Gone` error with the code `cursor_expired`.
*   *Client Action:* Drop the local cursor and perform a full metadata sync from version 0.

---

## 3. Authentication Flow (OPAQUE over HTTP)

Authentication uses the OPAQUE PAKE protocol. The server never receives the Master Password or Master Key.

### `POST /auth/register/start`
*   **Auth:** None
*   **Request:** `{ "username": "string", "registration_start": "base64" }`
*   **Response 200:** `{ "registration_response": "base64" }`

### `POST /auth/register/finish`
*   **Auth:** None
*   **Request:** `{ "username": "string", "registration_finish": "base64", "server_public_key": "base64" }`
*   **Response 200:** `{ "status": "success" }`

### `POST /auth/login/start`
*   **Auth:** None
*   **Request:** `{ "username": "string", "login_start": "base64" }`
*   **Response 200:** `{ "login_response": "base64" }`

### `POST /auth/login/finish`
*   **Auth:** None
*   **Request:** `{ "username": "string", "login_finish": "base64" }`
*   **Response 200:** `{ "session_token": "string", "expires_at": 1700000000000 }`

---

## 4. Sync Engine Endpoints

### `GET /sync/pull` (Metadata-First)
Fetches metadata changes since the last sync cursor. Does **NOT** return ciphertext payloads. 

*   **Auth:** Bearer Token
*   **Query Params:** `cursor` (u64), `limit` (u32).
*   **Response 200:**
    ```json
    {
      "new_cursor": 150,
      "has_more": true,
      "min_enc_key_gen": 2,
      "items": [
        {
          "uuid": "string",
          "version": 5,
          "enc_key_gen": 2,
          "deleted_date": null
        }
      ]
    }
    ```
*   **Response 410 (Cursor Expired):**
    ```json
    {
      "error": "cursor_expired",
      "message": "The requested sync cursor is no longer available."
    }
    ```
    *Core Logic:* If `local_gen < min_enc_key_gen`, halt sync and trigger `KeyUpdateRequired`. Otherwise, evaluate DashMap state. For the rotating client, this returns items where `item.enc_key_gen < min_enc_key_gen` to identify pending rotation work.

### `POST /sync/pull-payloads` (Exact-Version Selective Download)
Requests ciphertext blobs for items the Core has decided it needs. Prevents AEAD race conditions by requiring the exact expected version.

*   **Auth:** Bearer Token
*   **Request (Max 100 items):**
    ```json
    {
      "items": [
        { "uuid": "uuid1", "version": 5 },
        { "uuid": "uuid2", "version": 12 }
      ]
    }
    ```
*   **Response 200:**
    ```json
    {
      "results": [
        {
          "uuid": "uuid1",
          "status": "payload_delivered",
          "version": 5,
          "enc_key_gen": 2,
          "deleted_date": null,
          "payload": "base64_encrypted_ciphertext_blob"
        },
        {
          "uuid": "uuid2",
          "status": "version_mismatch",
          "version": 13,
          "enc_key_gen": 2,
          "deleted_date": null,
          "payload": null
        }
      ]
    }
    ```
    *Server Logic:* If the server's current version matches the requested version, it returns the payload. If it was updated by another device, it returns `version_mismatch` with the *new metadata* (including `deleted_date`), forcing the client to re-evaluate without throwing a fatal AEAD `TagMismatch` error.

### `POST /sync/push-batch` (Bulk Mutation & Rotation Endpoint)
Creates, updates, or tombstones an array of items. **Crucial for crash-safe key rotation** to prevent thousands of sequential HTTP requests. OCC is evaluated per-item within the batch.

*   **Auth:** Bearer Token
*   **Request (Max 100 items):**
    ```json
    {
      "items": [
        {
          "uuid": "uuid1",
          "target_version": 5,
          "enc_key_gen": 3,
          "payload": "base64_encrypted_ciphertext_blob",
          "deleted_date": null
        },
        {
          "uuid": "uuid2",
          "target_version": 8,
          "enc_key_gen": 3,
          "payload": null,
          "deleted_date": 1700000000000
        }
      ]
    }
    ```
*   **Response 200 (Multi-Status):**
    ```json
    {
      "results": [
        {
          "uuid": "uuid1",
          "status": "success",
          "version": 6,
          "enc_key_gen": 3,
          "updated_at": 1700000001000
        },
        {
          "uuid": "uuid2",
          "status": "conflict",
          "current_server_state": {
            "version": 9,
            "enc_key_gen": 2
          }
        }
      ]
    }
    ```
    *Server Logic:* 
    1. Validates `enc_key_gen >= min_enc_key_gen` for all items. If any fail, the entire batch is rejected with `422` to prevent partial epoch writes.
    2. Evaluates OCC per item (`target_version == server_version`).
    3. Commits successful items in a single atomic database transaction.
    4. Returns an array of results. The client processes successes, and feeds conflicts into the DashMap / 412 resolution logic.

### `PUT /items/{uuid}` (Standard OCC Curing)
Single-item update endpoint. Used for standard user edits or resolving conflicts.

*   **Auth:** Bearer Token
*   **Headers:** `If-Match: "<target_version>"`
*   **Request:** `{ "enc_key_gen": 2, "payload": "base64" }`
*   **Response 200 (Success):** `{ "uuid": "string", "version": 6, "enc_key_gen": 2, "updated_at": 1700000000000 }`
*   **Response 412:** `{ "error": "precondition_failed", "current_server_state": { "version": 6, "enc_key_gen": 2 } }`
*   **Response 422:** `{ "error": "key_generation_too_old", "min_enc_key_gen": 3 }`

### `DELETE /items/{uuid}` (Single-Item Tombstoning)
Soft-deletes an item. Required for the Validated Reaper.

*   **Auth:** Bearer Token
*   **Headers:** `If-Match: "<target_version>"`
*   **Response 200 (Success):** `{ "uuid": "string", "version": 7, "deleted_date": 1700000000000 }`
*   **Response 404:** `{ "error": "not_found" }` *(Reaper: drop quarantine entry).*
*   **Response 412:** `{ "error": "precondition_failed", "current_server_state": { "version": 8, "enc_key_gen": 3 } }` *(Reaper: reset TTL).*

---

## 5. Account & Key Management Endpoints

### `GET /account/status`
*   **Auth:** Bearer Token
*   **Response 200:** `{ "min_enc_key_gen": 2, "svk_ciphertext_blob": "base64" }`

### `POST /account/rotate-key` (Crash-Safe & Idempotent)
Updates the global epoch *before* the rotating client pushes re-encrypted items via `POST /sync/push-batch`.

*   **Auth:** Bearer Token
*   **Request:** `{ "new_min_enc_key_gen": 3, "new_svK_ciphertext_blob": "base64" }`
*   **Response 200:** `{ "status": "success", "min_enc_key_gen": 3 }`
*   **Idempotency Contract:** If the server's `min_enc_key_gen` already equals `new_min_enc_key_gen`, return `200 OK`.
*   **Crash-Safe Rotation Flow:**
    1. Client derives New SVK (gen 3).
    2. Client calls `POST /account/rotate-key`. Server `min_enc_key_gen` is now 3.
    3. Lagged devices sync, see `min_enc_key_gen=3` in `GET /sync/pull`, trigger Read-Only gate.
    4. Rotating client pushes re-encrypted items via `POST /sync/push-batch` (100 at a time) with `enc_key_gen=3`.
    5. **Resumability:** If the rotating client crashes, it restarts, calls `GET /sync/pull`, identifies items still at `enc_key_gen < 3`, and continues batching.

---

## 6. Error & Status Code Contracts

```json
{
  "error": "string_enum_code",
  "message": "Human readable description (dev logs only)",
  "context": { /* Optional dynamic object */ }
}
```

| HTTP Status | Error Enum | Core Mapping | Action |
| :--- | :--- | :--- | :--- |
| **400** | `payload_limit_exceeded` | `CoreError::InvalidRequest` | Client bug. Reduce batch size. |
| **401** | `unauthorized` | `AuthError` | Trigger re-auth flow. |
| **404** | `not_found` | `CoreError::NotFound` | Reaper: Drop quarantine entry. |
| **410** | `cursor_expired` | `CoreError::SyncResetRequired` | Drop local cursor, full sync from 0. |
| **412** | `precondition_failed` | `CoreError::OCCConflict` | Trigger DashMap / Context-Sensitive 412 logic. |
| **422** | `key_generation_too_old` | `CoreError::EpochMismatch` | Trigger `KeyUpdateRequired` Read-Only gate. |
| **429** | `rate_limited` | `CoreError::NetworkError` | Exponential backoff. |
| **5xx** | `internal_server_error` | `CoreError::NetworkError` | Backoff and retry. |
