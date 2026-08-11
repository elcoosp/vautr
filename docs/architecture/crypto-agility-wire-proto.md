# Vautr Cryptographic Agility & Wire Protocol Specification (v2.0 - The Final Form)

This document defines the exact envelope formats, algorithm registries, and migration strategies required to ensure Vautr remains cryptographically agile. Algorithms degrade over time; a system that hardcodes primitives into its domain logic is a system destined for a catastrophic rewrite.

Vautr solves this by enforcing strictly enveloped ciphertexts and a **Decoupled** Cryptographic Suite Registry. Every encrypted payload is self-describing at the domain level, but the domains of Data Encryption, Key Wrapping, and Key Derivation are strictly isolated. This ensures that upgrading the Key Encapsulation Mechanism (KEM) for Post-Quantum (PQ) resistance does not force the re-encryption of terabytes of vault data.

Deviations from this specification will result in cryptographic lock-in, I/O apocalypses during algorithm deprecation, or the inability to securely open vaults across client versions.

---

## 1. The Agility Philosophy (Domain Decoupling)

1.  **No Hardcoded Primitives:** The core application logic never invokes `chacha20poly1305::encrypt` directly. It requests encryption via active Suite identifiers. `vautr-crypto` acts as a strict factory.
2.  **Domain Decoupling (The PQ Survival Rule):** Data Encryption (AEAD), Sharing (KEM), and Key Derivation (KDF) evolve on completely different timelines. Upgrading the KEM to ML-KEM-768 must **never** force the re-encryption of data payloads. We maintain separate registries for each domain.
3.  **Enveloped Ciphertexts:** Every encrypted payload is prefixed with a minimal metadata header. Parsing an envelope does not require possession of the key; decryption does.
4.  **Graceful Degradation:** If a client encounters a Suite it does not support, it must fail securely (mark the item as `ToxicUnsupported`) rather than crash, attempt to downgrade, or skip the item.
5.  **No Unauthenticated Policy in Blobs:** Encrypted blobs must not contain unauthenticated policy metadata (like Suite IDs or Key Gens) that influence decryption logic. This prevents forgery-based DoS attacks. The AEAD Associated Data (AD) is the sole source of truth for integrity; key selection relies on authenticated local state.

---

## 2. The Decoupled Cryptographic Suite Registry

Instead of a single monolithic `CryptoSuite`, Vautr uses three distinct registries. This allows surgical upgrades.

### 2.1 Data Suites (AEAD)
Determines the symmetric encryption of vault items. Changes only if the AEAD cipher is broken (extremely rare).

| Suite ID | AEAD | Status | Notes |
| :--- | :--- | :--- | :--- |
| `0x01` | XChaCha20-Poly1305 | **Active** | Vautr v1.0 default. |

### 2.2 Sharing Suites (KEM)
Determines the asymmetric key encapsulation for sharing. Changes when classical ECC is threatened (e.g., by quantum computers). **Upgrading this does NOT require re-encrypting data payloads, only re-wrapping the SIKs.**

| Suite ID | KEM | Status | Notes |
| :--- | :--- | :--- | :--- |
| `0x01` | X25519 | **Active** | Vautr v1.0 default. |
| `0x02` | ML-KEM-768 | **Planned** | PQ-KEM upgrade path. |
| `0x03` | X25519 + ML-KEM-768 | **Planned** | Hybrid PQ-KEM. |

### 2.3 KDF Suites
Determines the Master Key derivation. Changes if Argon2id is superceded.

| Suite ID | KDF | Status | Notes |
| :--- | :--- | :--- | :--- |
| `0x01` | Argon2id | **Active** | Vautr v1.0 default. |

### 2.4 Account Metadata & Server Enforcement
*   **Account Metadata:** The `sync_meta` table tracks the `active_data_suite`, `active_sharing_suite`, and `active_kdf_suite` for the user. New items use these active suites.
*   **Server Enforcement:** When pushing via `POST /sync/push-batch`, the JSON API payload includes the `data_suite_id`. The server maintains a whitelist of accepted suites. If `0x01` is deprecated, the server rejects new uploads with `422 CryptoSuiteDeprecated`, forcing the client to trigger an Active Data Migration. The server **never** parses the binary envelope blob to enforce this; it relies solely on the JSON metadata.

---

## 3. The Ciphertext Envelope Format

Every encrypted payload is encapsulated in a standard binary envelope. To prevent forgery-based DoS attacks, the envelope header contains *only* structural metadata, not cryptographic policy.

### 3.1 Envelope Structure (Minimal Header)

```text
+-----------------------+----------------------------+
| Header (3 bytes)      | Description                |
+-----------------------+----------------------------+
| Magic (2 bytes)      | 0x5641 ("VA")              |
| Version (1 byte)     | Envelope format version (0x01)|
+-----------------------+----------------------------+
| Body                  | Description                |
+-----------------------+----------------------------+
| Nonce (variable)     | Determined by Data Suite    |
| Ciphertext (variable)| Encrypted domain data       |
| Tag (variable)       | AEAD Auth Tag               |
+-----------------------+----------------------------+
```
*   **Rationale for Stripped Header:** We explicitly exclude `Suite ID` and `enc_key_gen` from the envelope. An attacker could flip bits in an unauthenticated header, causing the client to select the wrong key, fail decryption, and mark the item `Toxic` (DoS). Instead, the client determines the suite and key from its secure, authenticated local `sync_meta` and SQLite metadata, relying on the AEAD Tag to validate the choice.

### 3.2 Local Storage & Key Selection
*   **SQLite Metadata:** The `item_overviews` table includes a `data_suite_id` column. This tells the client exactly which Data Suite factory to use for decryption *before* attempting it, optimizing performance.
*   **Fallback Mechanism:** If `data_suite_id` is missing (corruption), the client tries the `active_data_suite`. If `TagMismatch`, it falls back to deprecated suites. The AEAD AD (`uuid || enc_key_gen`) validates integrity.

### 3.3 Sharing Envelope Variant
The `WrappedSIK` uses a slightly extended envelope to include the public key material required for KEM.
```text
+-----------------------+----------------------------+
| Magic (2 bytes)      | 0x5642 ("VB")              |
| Version (1 byte)     | Envelope format version (0x01)|
| Sharing Suite (1 b)  | KEM Suite ID (e.g., 0x01) |
| PubKey Len (2 bytes) | Length of ephemeral pubkey  |
| PubKey (variable)    | Ephemeral Public Key        |
| Nonce (variable)     | Determined by KEM Suite     |
| Ciphertext (variable)| Encrypted SIK               |
| Tag (variable)       | AEAD Auth Tag               |
+-----------------------+----------------------------+
```

---

## 4. Cryptographic Migration (Targeted Upgrade Paths)

Because suites are decoupled, migrations are surgical. Upgrading the KEM does not require re-encrypting data.

### 4.1 Sharing Suite Migration (KEM Upgrade)
When X25519 is deprecated in favor of ML-KEM-768:
1.  **Trigger:** Server deprecates Sharing Suite `0x01`.
2.  **Action:** Client generates a new `SharingKeypair` using Suite `0x02`.
3.  **Re-wrapping:** Client iterates through inbound shares. It unwraps each SIK using the old Private Key, and re-wraps the SIK using the new Suite `0x02` Public Key.
4.  **Result:** O(N) key wraps. **Zero data payload re-encryption.** This takes seconds, even for thousands of shares.

### 4.2 Data Suite Migration (AEAD Upgrade)
Extremely rare. Only if XChaCha20 is broken.
1.  **Passive Migration:** If a vault's `active_data_suite` is deprecated, the system organically upgrades it during normal user behavior. User edits an item; `PersistenceWorker` re-encrypts it with the new Data Suite.
2.  **Active Migration:** For fleet-wide security, an Active Migration forces the re-encryption of the entire vault.
    *   **Process:** The `PersistenceWorker` iterates through the local DB. For each item, it decrypts with the Old Suite, re-encrypts with the New Suite, updates the `data_suite_id` in SQLite, and enqueues a standard `save_item` to the server.
    *   **Resumability:** Identical to Key Rotation, the worker tracks progress via `enc_key_gen`, surviving app crashes.

---

## 5. Wire Protocol Versioning (API Evolution)

As Vautr evolves, the HTTP API must support new features without breaking older clients.

### 5.1 Content Negotiation
API versions are enforced via structured `Content-Type` headers.
*   **Request:** Clients send `Content-Type: application/vnd.vautr.sync.v1+json`.
*   **Response:** Server responds with `Content-Type: application/vnd.vautr.sync.v1+json`.
*   **Server Logic:** If a client sends a version the server no longer supports (e.g., `v0`), the server returns `415 Unsupported Media Type`.

### 5.2 Lagged Client Rejection
To prevent clients with critically old cryptography from corrupting the vault or bypassing security patches:
1.  **Minimum Client Version:** The server maintains a `min_client_version` in the global config.
2.  **Enforcement:** Upon OPAQUE authentication, the client sends its version in a header. If the version is below the minimum, the server returns `426 Upgrade Required` with a JSON body containing the URL to download the latest client. The client must block the user from accessing the vault until they update.

### 5.3 Backward Compatibility Window
The server maintains a sliding window of supported API versions (e.g., current release minus 2 major versions). This gives users a grace period to update their clients without losing access to their data.

---

## 6. Security Considerations & Trade-offs

*   **Envelope Overhead:** The 3-byte header adds negligible overhead. The agility benefits vastly outweigh the bandwidth cost.
*   **Mixed-Suite Complexity:** Supporting mixed-suite vaults increases the complexity of the `vautr-keyring` and `vautr-crypto` factory. However, this is a necessary trade-off to prevent hard-breaking changes and ensure seamless passive migrations.
*   **Suite 0x00 Forbidden:** The `0x00` Suite ID is explicitly forbidden in all registries. It implies "plaintext" or "no encryption." Any blob with this ID must be rejected by the core.
*   **Header Forgery Resilience:** By stripping policy from the unauthenticated header, we eliminate the attack surface where a malicious server or disk corruption tricks the client into marking valid data as `Toxic`. The AEAD Tag is the sole arbiter of integrity.
