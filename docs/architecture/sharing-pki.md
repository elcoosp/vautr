# Vautr Zero-Knowledge Sharing & PKI Architecture (v2.0 - The Final Form)

This document defines the exact cryptographic mechanisms, key hierarchy, and server protocols required to facilitate end-to-end encrypted sharing of vault items between independent Vautr users. 

In a Zero-Knowledge architecture, sharing is fundamentally a boundary-crossing operation. The sender's Symmetric Vault Key (SVK) cannot leave their domain, and the server cannot re-encrypt the data. Vautr solves this by implementing an Asymmetric Key Infrastructure (PKI) layer using a modern Key Encapsulation Mechanism (KEM) to perform hybrid encryption. The server acts solely as an untrusted relay for encrypted blobs and public keys.

Deviations from this specification will result in the exposure of shared plaintext to the server, the creation of sharing backdoors, or a severely degraded user experience for recipients.

---

## 1. The Sharing Philosophy (The Asymmetric Boundary)

1.  **Strict Domain Isolation:** The Symmetric Vault Key (SVK) is strictly personal. Sharing an item must never expose the sender's SVK or require the recipient's SVK for decryption.
2.  **End-to-End Encrypted Relay:** The server facilitates the exchange of encrypted payloads but possesses zero cryptographic capability to decrypt them.
3.  **Cryptographic User Identity:** Every Vautr user possesses an asymmetric Sharing Keypair, independent of their Master Password, enabling unsolicited inbound shares without prior key exchange.
4.  **Copy-on-Write Sharing:** Shared items are encrypted under a dedicated Symmetric Item Key (SIK). This allows the sender to update the shared item without re-wrapping keys for all recipients, and allows recipients to read updates without re-sharing.

---

## 2. The PKI Key Tree & Directory

The existing Crypto Spec key tree is extended with a dedicated sharing keypair.

### 2.1 Keypair Generation & Storage
*   **Algorithm:** X25519 (Elliptic Curve Diffie-Hellman). Chosen for its small key size, constant-time operations, and resistance to side-channel attacks.
*   **Generation:** The `SharingKeypair` is generated locally via `OsRng` during vault creation, entirely independent of the MP or SVK.
*   **Private Key Storage:** The `SharingPrivateKey` is wrapped (encrypted) by the user's `KEK_MP` (derived from the Master Password), exactly like the SVK. It is stored in the local `sync_meta` table and synced to the server as `WrappedSharingPrivKey`.
*   **Public Key Directory:** The `SharingPublicKey` is uploaded to the Vautr server.
*   **Authentication:** To prevent a malicious actor from uploading a fake Public Key for a user (MITM attack), the Public Key upload is signed by the user's OPAQUE session token, and the server strictly enforces one Public Key per OPAQUE identity.

### 2.2 Key Rotation
If a user's `SharingPrivateKey` is compromised:
1.  The user generates a new `SharingKeypair`.
2.  The new Public Key is uploaded to the server, replacing the old one.
3.  The new Private Key is wrapped and stored.
4.  **Resharing Required:** Because the old Private Key is gone, the user can no longer decrypt items shared with the old Public Key. Senders must re-share items using the new Public Key.

---

## 3. The Hybrid Encryption Sharing Flow (The KEM-DEM Protocol)

When User A shares Item X with User B, Vautr uses a Key Encapsulation Mechanism (KEM) combined with a Data Encapsulation Mechanism (DEM).

### 3.1 Step 1: Lookup & Verification
1.  **Request:** User A requests User B's `SharingPublicKey` from `GET /users/{uuid}/public-key`.
2.  **Verify:** Client verifies the key is validly formatted and not expired.

### 3.2 Step 2: Symmetric Item Key (SIK) Generation
A shared item is never encrypted directly with an asymmetric key. Instead, User A generates a one-time, 256-bit Symmetric Item Key (SIK) specifically for the shared payload.
*   `SIK = OsRng.gen::<[u8; 32]>()`

### 3.3 Step 3: DEM (Data Encryption)
User A decrypts Item X locally using their SVK, then immediately re-encrypts the plaintext using the SIK.
*   `SharedPayload = AEAD_Encrypt(SIK, AD="vautr-share-{share_id}", Plaintext)`
*   This payload is uploaded to the server. Any edits User A makes to Item X in the future are re-encrypted with this same SIK, allowing recipients to see updates without re-wrapping keys.

### 3.4 Step 4: KEM (Key Encapsulation)
User A encapsulates the SIK for User B.
1.  **Ephemeral Key:** A generates an ephemeral X25519 keypair.
2.  **Shared Secret:** A performs Diffie-Hellman: `SharedSecret = EphemeralPrivKey * B_PublicKey`.
3.  **Derive Wrapping Key:** `KEK_SHARE = HKDF(SharedSecret, info="vautr-share-wrap")`.
4.  **Wrap:** `WrappedSIK = AEAD_Encrypt(KEK_SHARE, AD="vautr-share-wrap", SIK)`.
5.  **Output:** A produces `WrappedSIK` and `EphemeralPublicKey`.

### 3.5 Step 5: Dispatch
User A uploads the `SharedPayload`, `WrappedSIK`, and `EphemeralPublicKey` to the server.

---

## 4. The Receiving & Ingestion Flow

User B receives and decrypts the shared item.

### 4.1 Decapsulation (Unwrapping the SIK)
1.  **Fetch:** B downloads `WrappedSIK` and `EphemeralPublicKey`.
2.  **Shared Secret:** B performs Diffie-Hellman: `SharedSecret = B_SharingPrivKey * EphemeralPublicKey`.
3.  **Derive Wrapping Key:** `KEK_SHARE = HKDF(SharedSecret, info="vautr-share-wrap")`.
4.  **Unwrap:** `SIK = AEAD_Decrypt(KEK_SHARE, AD="vautr-share-wrap", WrappedSIK)`.

### 4.2 SIK Caching & Local Ingestion (The UX Boundary)
To prevent requiring the recipient's `SharingPrivateKey` for every view (which would force constant MP entry), the SIK is cached locally.
1.  **Cache SIK:** The client wraps the unwrapped SIK with the recipient's personal `KEK_MP` and stores it as `WrappedSIK_Personal` in the local SQLite `sync_meta` table.
2.  **Ingestion (Linked Mode):** The decrypted item is stored in B's local DB, but it remains encrypted under the SIK in the local cold storage. B can now view and autofill the item using standard MP/Biometric unlock (which unwraps the `KEK_MP`, which unwraps the cached SIK).
3.  **Claiming (Copy to Vault):** If B clicks "Move to My Vault," the client decrypts the item with the SIK and re-encrypts it with B's OEK/DEK. The SIK link is broken; B now owns the item, and A's future updates will not affect B's copy.

---

## 5. Server APIs & Sharing Metadata

The server acts as a relay. It must never possess the SIK, the Shared Secret, or the plaintext.

*   **Directory:** `GET /users/{uuid}/public-key` -> Returns `SharingPublicKey`.
*   **Initiate Share:** `POST /shares/`
    *   **Payload:** `sender_uuid`, `recipient_uuid`, `item_uuid`, `WrappedSIK`, `EphemeralPublicKey`.
    *   **Metadata:** The server is permitted to store `sender_uuid`, `recipient_uuid`, and `item_uuid` to facilitate delivery and revocation. This metadata is protected by TLS in transit and must be encrypted at rest on the server.
*   **Upload Shared Payload:** `POST /shares/{share_id}/payload` -> Uploads the `SharedPayload` (encrypted by SIK).
*   **Inbox:** `GET /shares/inbox` -> Returns metadata and `WrappedSIK` for pending shares.
*   **Revoke Share (1:1):** `DELETE /shares/{share_id}` -> Removes the share and payload. To guarantee cryptographic revocation (preventing offline access by a cached SIK), the sender's client automatically triggers an SIK rotation for the item, re-wrapping for remaining recipients.

---

## 6. Sharing Groups (1:N)

Scaling 1:1 KEM to 1:N (Groups/Families) requires avoiding N re-encryptions of the payload. We use a unified Group SIK model, eliminating unnecessary group keypair indirection.

### 6.1 The Group SIK
A group is treated as a shared context.
1.  **Group SIK:** Upon creation, a `GroupSIK` is generated.
2.  **Payload Encryption:** Items shared to the group are encrypted with the `GroupSIK`.
3.  **Member Wrapping:** The Admin wraps the `GroupSIK` individually for each member using their personal `SharingPublicKey` (exactly like 1:1 sharing, but wrapping the Group SIK instead of an Item SIK). These wrapped keys are stored on the server.

### 6.2 Adding a Member
When Admin adds User C to the group:
1.  Admin fetches User C's `SharingPublicKey`.
2.  Admin wraps the `GroupSIK` with User C's Public Key.
3.  Admin uploads the `WrappedGroupSIK` to the server. User C can now unwrap the Group SIK, and use it to read the group payload. No payload re-encryption required.

### 6.3 Removing a Member (Cryptographic Revocation)
Removing a member requires SIK rotation to enforce forward secrecy.
1.  **Generate New SIK:** Admin generates a new `GroupSIK`.
2.  **Re-encrypt Payload:** Admin re-encrypts the shared items with the new SIK.
3.  **Re-wrap:** Admin wraps the new SIK for all remaining members.
4.  **Dispatch:** Admin uploads the new payload and wrapped keys, and deletes the removed member's access.
5.  **Performance:** Offloaded to the `PersistenceWorker` to prevent UI freezes during large group management.

---

## 7. Security Considerations & Trade-offs

*   **Forward Secrecy of Delivery (Bounded PFS):** The use of an `EphemeralPublicKey` in the KEM ensures that if User B's `SharingPrivateKey` is compromised, the attacker cannot decrypt past `WrappedSIK` blobs intercepted from the server. However, because the SIK is static to facilitate updates without re-wrapping for all recipients, a compromised cached SIK compromises the data stream for that item. True data stream PFS would require SIK rotation on every edit, which is deferred due to performance constraints.
*   **Share Revocation Latency:** While 1:1 revocation triggers an SIK rotation, there is a latency window where a revoked recipient's offline client might still read the cached SIK. This is an accepted trade-off; immediate revocation relies on server ACLs, while eventual cryptographic revocation relies on the SIK rotation.
*   **Metadata Leakage:** The server learns the graph of who is sharing with whom (`sender_uuid`, `recipient_uuid`). This is an inherent trade-off for server-mediated delivery. vautr does not reveal *what* is being shared (titles, URLs).
