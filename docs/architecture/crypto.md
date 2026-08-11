# Vautr Cryptographic Specification & Key Tree

This document defines the exact mathematical operations, algorithmic parameters, and data structures required to implement the Vautr Zero-Knowledge architecture. It enforces strict key separation, eliminates precomputation vectors, and correctly leverages hardware-backed OS keystores.

---

## 1. Cryptographic Primitives & Dependencies

All operations reside in `vautr-crypto`. No external crate is allowed to touch plaintext key material.

| Operation | Algorithm | Crate | Notes |
| :--- | :--- | :--- | :--- |
| **KDF** | Argon2id (v1.3) | `argon2` | Memory-hard password derivation. |
| **AEAD** | XChaCha20-Poly1305 | `chacha20poly1305` | Symmetric encryption. 192-bit nonce prevents misuse. |
| **KDF (Key Split)** | HKDF (HMAC-SHA256) | `hmac` / `sha2` | Pseudorandom key expansion & separation. |
| **PAKE** | OPAQUE | `opaque-ke` | Asymmetric auth; server never sees password equivalents. |
| **RNG** | `OsRng` | `rand` | Cryptographically secure randomness. |
| **Memory** | `Zeroizing<T>` | `zeroize` | Secure memory wiping. |

---

## 2. The Key Tree & Derivation

The system uses a layered hierarchy with strict cryptographic isolation. Authentication keys are mathematically detached from encryption keys.

### 2.1 Visual Key Tree

```text
[User Input]
   └── Master Password (MP) ───────────┐
                                       ▼
[Server Provides]                    Argon2id
   └── KDF Salt (32b Random) ────► (MP + KDF_Salt)
                                       │
                                       ▼
                                  Master Key (MK)
                                       │
                          ┌────────────┴────────────┐
                          ▼                         ▼
               HKDF-Extract/Expand             OPAQUE Registration
               (Salt: "Vautr-kek")            (Separate Protocol Flow)
                          │                         │
                          ▼                         ▼
               Key Encryption Key (KEK)       OPAQUE Auth State
               (Encrypts SVK Blob)            (Proves Identity to Server)
                          │
                          ▼
               Decrypts SVK Ciphertext (from Server)
                          │
                          ▼
                Symmetric Vault Key (SVK) ◄─── [Cached via OS Biometrics]
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
    HKDF-Extract/Expand       HKDF-Extract/Expand
    (Salt: "Vautr-oek")      (Salt: "Vautr-dek")
              │                       │
              ▼                       ▼
    Overview Encryption Key    Data Encryption Key
    (OEK - 32b)               (DEK - 32b)
```

### 2.2 Step-by-Step Derivation

#### Step 1: Master Key (MK) Derivation
The Master Password never leaves the device. It is mixed with a unique, random salt.

*   **Algorithm:** Argon2id
*   **Parameters (Baseline):** `m_cost`=64 MiB, `t_cost`=3, `p_cost`=4, `output_len`=32.
*   **KDF Salt:** 32-byte cryptographically secure random value generated at vault creation. Stored unencrypted on the server per-user.
    *   *Rationale:* A random salt completely neutralizes precomputation (rainbow table) attacks.
*   **Output:** `MK: [u8; 32]` (Held in `Zeroizing<[u8; 32]>`)

#### Step 2: Authentication (OPAQUE)
To prove identity without exposing the MK to offline brute-force attacks if the server DB leaks, Vautr uses the OPAQUE PAKE protocol.

*   **Mechanism:** The client and server engage in an OPAQUE login flow.
*   **Key Isolation:** The OPAQUE protocol uses the MP internally but operates entirely independently of the local MK/KEK derivation. The server stores an OPAQUE registration record, *not* a verifiable hash of the MK. A server breach yields zero password-equivalent data.
*   **Output:** Authenticated TLS session.

#### Step 3: Key Encryption Key (KEK) Derivation
The MK is never used directly to encrypt vault data. It acts solely as a KEK to unwrap the actual vault key.

*   **Algorithm:** HKDF (HMAC-SHA256)
*   **Process:**
    1.  `PRK = HKDF-Extract(salt="Vautr-kek-salt", ikm=MK)`
    2.  `KEK = HKDF-Expand(PRK, info="Vautr-kek", L=32)`
*   **Output:** `KEK: [u8; 32]` (Used exclusively to decrypt the SVK blob fetched from the server).

#### Step 4: Symmetric Vault Key (SVK)
The root key for all vault data. Generated randomly upon vault creation.

*   **Generation:** `OsRng.gen::<[u8; 32]>()`
*   **Server Storage:** Encrypted by KEK using XChaCha20-Poly1305.
*   **Client Cache:** Wrapped by the OS Hardware Keystore for Biometric unlock (See Sec 6).
*   **Output:** `SVK: [u8; 32]`

#### Step 5: Contextual Domain Keys (OEK & DEK)
To support the Client Architecture's Split Data Model, we derive separate keys.

*   **Overview Encryption Key (OEK):**
    1.  `PRK = HKDF-Extract(salt="Vautr-oek-salt", ikm=SVK)`
    2.  `OEK = HKDF-Expand(PRK, info="Vautr-oek", L=32)`
*   **Data Encryption Key (DEK):**
    1.  `PRK = HKDF-Extract(salt="vautr-dek-salt", ikm=SVK)`
    2.  `DEK = HKDF-Expand(PRK, info="Vautr-dek", L=32)`

---

## 3. Item Encryption & Associated Data (AD)

The server is an untrusted blob store. AEAD context binding prevents ciphertext swap/replay attacks.

### 3.1 Ciphertext Format

```text
+-------------------+---------------------+--------------------+
| Nonce (24 bytes)  | Ciphertext (N bytes) | Tag (16 bytes)     |
+-------------------+---------------------+--------------------+
```

### 3.2 Associated Data (AD) Binding

```rust
fn construct_ad(uuid: &Uuid, enc_key_gen: u64) -> [u8; 24] {
    let mut ad = [0u8; 24];
    ad[..16].copy_from_slice(uuid.as_bytes());
    ad[16..24].copy_from_slice(&enc_key_gen.to_be_bytes());
    ad
}
```

### 3.3 Granular Decryption Errors
Decryption must never fail silently or with a generic error. `vautr-crypto` must return:
*   `CryptoError::TagMismatch` (Wrong key or tampered ciphertext).
*   `CryptoError::KeyGenMismatch` (Correct key family, but wrong epoch).
*   `CryptoError::MalformedCiphertext` (Incorrect byte lengths).
*   *Core Note:* `TagMismatch` or `KeyGenMismatch` maps to the `ToxicIgnored` state in the DashMap.

---

## 4. Zero-Knowledge Metadata Enforcement

1.  **Sync Payload:** `PUT /items/{uuid}` only pushes `uuid`, `enc_key_gen`, `version`, and the `Ciphertext Blob`. Title, Username, URLs, Notes are strictly encrypted inside the blob using OEK/DEK.
2.  **Local SQLite:** Local DB stores `DecryptedOverview` and `DecryptedSecret` in plaintext (protected by OS full-disk encryption). FTS5 operates strictly on this local, decrypted data. Sync never touches this data.

---

## 5. Key Rotation & Crash-Resumable Batching

When the SVK is rotated (e.g., compromised MP), the `PersistenceWorker` must re-encrypt the vault.

**Resumable Rotation Flow:**
1.  **Trigger:** New SVK generated.
2.  **Batch Fetch:** Worker queries local SQLite for items where `enc_key_gen == old_gen` LIMIT 100.
3.  **Re-encrypt:** Decrypt with OEK/DEK derived from `OldSVK`. Encrypt with OEK/DEK derived from `NewSVK`.
4.  **Local Commit:** Update the local SQLite item with the new ciphertext and `enc_key_gen = new_gen`. *This makes the rotation crash-safe.*
5.  **Server Sync:** Push the updated batch to the server.
6.  **Loop:** If the app crashes, the Worker resumes on the next launch and only fetches items still matching `old_gen`.

---

## 6. OS Keystore Integration (Biometrics)

The SVK can be cached for instant biometric unlock. This requires hardware-backed keys. **No keys derived from the Master Password are used for biometric wrapping.**

**Wrapping the SVK (On MP Unlock):**
1.  **Generate Hardware Key:** Request the OS Keystore (Secure Enclave / TEE) to generate a new AES-256 key, restricted to require Biometric (FaceID/TouchID) unlock.
2.  **Wrap:** Pass the raw `SVK` bytes to the OS Keystore API. The OS encrypts the SVK using the hardware-backed key.
3.  **Store:** Save the resulting `WrappedSVKBlob` in standard SharedPreferences/UserDefaults.

**Unwrapping the SVK (On Biometric Unlock):**
1.  User authenticates via FaceID/TouchID.
2.  **Unwrap:** Pass `WrappedSVKBlob` to the OS Keystore API. The hardware decrypts it and returns the raw `SVK` bytes to memory.
3.  **Unlock:** Pass raw `SVK` bytes to `core.unlock_with_raw_key(Zeroizing::new(SVK))`.

---

## 7. Emergency Recovery Key Derivation (KEK_RK)

The Recovery Key is a 24-word BIP-39 mnemonic (BR-7, REQ-RECOVERY-01/02). It must recover the SVK **without** the Master Password. The derivation path (closes gap #4):

1.  **Mnemonic → seed:** `seed = BIP39_Seed(mnemonic, passphrase="")` (64 bytes; no passphrase per BR-7).
2.  **KEK_RK:** `KEK_RK = HKDF-Extract/Expand(SHA256, ikm=seed, salt="Vautr-kek-rk-salt", info="Vautr-kek-rk", L=32)`.
3.  **SVK wrap:** The SVK is sealed under `KEK_RK` using XChaCha20-Poly1305 with AD = `(server_user_id, enc_key_gen=0)`, yielding `WrappedSVK_RK`.
4.  **Server storage:** `WrappedSVK_RK` is persisted in `users.svk_ciphertext_blob_rk` (see `docs/architecture/server-db.md` §3). The MP-derived `WrappedSVK` lives in `svk_ciphertext_blob`; the two are independently unwrappable.
5.  **Recovery:** User enters the 24 words → `KEK_RK` re-derived → `WrappedSVK_RK` decrypted → SVK recovered. A wrong mnemonic yields a `TagMismatch` (no SVK).

Implemented in `vautr-crypto::recovery` (`derive_kek_rk`, `wrap_svk_with_rk`, `unwrap_svk_with_rk`) with a round-trip test.

---

## 8. Memory Safety & Zeroization Rules

1.  **All Keys (MK, KEK, SVK, OEK, DEK):** Must be held in `Zeroizing<[u8; 32]>`.
2.  **`mlock` (Memory Locking):** The `VautrClient` must invoke `mlock()` on the pages holding the `DashMap<SecretHandle, ActiveSecret>`. This prevents the OS from paging decrypted secrets to the SSD/HDD swap file.
3.  **RAII Enforcement:** The `ActiveSecret` struct explicitly overwrites the inner `DecryptedSecret` bytes on `Drop`.
