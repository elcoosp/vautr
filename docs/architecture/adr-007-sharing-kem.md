# ADR-007: Secure Sharing Key Encapsulation Mechanism (KEM)

- Status: Accepted
- Date: 2026-08-10
- Supersedes: none (gap #5 in gap analysis)
- Related: REQ-SHARE-01..04, docs/architecture/api.md (shares table),
  docs/architecture/server-db.md §3 (`shares` table), crypto.md §2.

## Context

Vautr shares individual items ("secure items", SIK) between users without the
server ever seeing plaintext or a shared symmetric key. SRS REQ-SHARE-01..04
require:

1. Wrap the SIK under the **recipient's SharingPublicKey**.
2. The server stores only `wrapped_sik` + `ephemeral_public_key` (zero-knowledge).
3. Revocation = deleting the `shares` row.
4. No plaintext SIK, no shared long-term secret with the server.

The exact KEM was unspecified (gap #5). This ADR locks it.

## Decision

**Use X25519 ECDH + XChaCha20-Poly1305** for the v1 sharing envelope:

- **Recipient key:** each user has a long-term `SharingKeyPair` (X25519). The
  public key is the `SharingPublicKey` published to peers (out-of-band / via a
  future `/shares/discovery` endpoint — not in scope for the schema migration).
- **Ephemeral sender key:** a fresh X25519 keypair is generated per share.
- **Shared secret:** `ss = X25519(ephemeral_sk, recipient_sharing_pk)`.
- **Envelope key:** `K = HKDF-SHA256(ss, info="Vautr-share", L=32)`.
- **Wrap:** `wrapped_sik = XChaCha20-Poly1305_Encrypt(K, nonce, AD, sik)` where
  `AD = recipient_sharing_pk(32) ‖ item_uuid(16)`.
- **Server stores:** `wrapped_sik` (BLOB) + `ephemeral_public_key`
  (the X25519 ephemeral public key, BLOB) in the `shares` table.
- **Recipient unwraps:** `ss = X25519(recipient_sharing_sk, ephemeral_public_key)`,
  recompute `K`, decrypt with the same AD.

The `item_uuid` in the AD binds the envelope to a specific item, preventing
envelope replay across items.

## Rationale

- X25519 + XChaCha20-Poly1305 is the same primitive family already standardized
  in crypto.md (XChaCha20-Poly1305 AEAD, HKDF-HMAC-SHA256), so it adds **no new
  cryptographic dependency class** beyond `x25519-dalek` (already transitively
  available via the OPAQUE stack and `chacha20poly1305`).
- Ephemeral ECDH (not static-static) means no long-term sender↔recipient shared
  secret is ever formed; each share is independently forward-secret.
- AD binding to `item_uuid` stops ciphertext/substitution across items.
- Simple enough to implement and audit before v1; the interface
  (`share_item` / `unwrap_shared_item`) is KEM-agnostic so a future PQC swap
  (ML-KEM / X-Wing) is a drop-in without changing callers.

## Post-1.0 (reserved, not implemented)

A PQC-capable envelope (ML-KEM-768 or X-Wing hybrid) will implement the same
`share_item`/`unwrap_shared_item` signatures, adding a `kem_alg` tag to the
stored `wrapped_sik` envelope header. Not required for v1 per SRS.

## Consequences

- `vautr-crypto::sharing` owns `SharingKeyPair` gen + `share_item`/`unwrap_shared_item`.
- `vautr-domain` gains `SharedItem`/`SharingPublicKey` types.
- The server `shares` table (server-db.md §3) is now fully specified and usable.
- Sharing remains "Should" priority; nothing in the core sync path depends on it.
