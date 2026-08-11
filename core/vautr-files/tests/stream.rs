//! TDD tests for VTR-025 (streaming file encryption, 1 MiB chunks).
//!
//! Covers the issue's mandated scenarios:
//! 1. Encrypt 3.5 MiB random file, decrypt, assert byte-exact roundtrip.
//! 2. Encrypt a 1-byte file (single partial chunk).
//! 3. Corrupt a single chunk in the ciphertext -> decrypt fails with TagMismatch.
//! 4. Resume: re-encrypting the same plaintext yields byte-identical ciphertext
//!    (so a crashed upload can restart from `total_chunks`).
//! 5. Property test: deterministic nonce never repeats for the same file_uuid
//!    across different chunk indices.

use futures_util::io::Cursor;
use proptest::prelude::*;
use uuid::Uuid;
use vautr_crypto::aead;
use vautr_files::manifest::FileManifest;
use vautr_files::{decrypt_file_stream, encrypt_file_stream, FileError, Result};

fn fek() -> [u8; 32] {
    [0x42u8; 32]
}

fn make_manifest(total_size: u64) -> FileManifest {
    FileManifest::new(
        Uuid::new_v4(),
        total_size,
        2,
        "application/octet-stream".into(),
        1_700_000_000_000,
    )
}

async fn roundtrip(plaintext: &[u8]) -> Result<()> {
    let manifest = make_manifest(plaintext.len() as u64);
    manifest.validate().map_err(|_| FileError::ManifestGeometry("validate".into()))?;
    let mut ct = Vec::new();
    encrypt_file_stream(Cursor::new(plaintext), &mut ct, &fek(), &manifest).await?;
    let mut pt = Vec::new();
    decrypt_file_stream(Cursor::new(&ct), &mut pt, &fek(), &manifest).await?;
    assert_eq!(pt, plaintext, "decrypted plaintext must match original");
    Ok(())
}

#[tokio::test]
async fn t1_3_5_mib_roundtrip() {
    let mut data = vec![0u8; 3 * 1024 * 1024 + 512 * 1024]; // 3.5 MiB
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i * 31 + 7) as u8;
    }
    roundtrip(&data).await.expect("3.5 MiB roundtrip");
}

#[tokio::test]
async fn t2_one_byte_file() {
    roundtrip(&[0xAB]).await.expect("1-byte roundtrip");
}

#[tokio::test]
async fn t2_zero_byte_file() {
    roundtrip(&[]).await.expect("0-byte roundtrip");
}

#[tokio::test]
async fn t3_corrupt_chunk_fails_tagmismatch() {
    let plaintext = vec![0xC3u8; 2 * 1024 * 1024 + 13]; // crosses a chunk boundary
    let manifest = make_manifest(plaintext.len() as u64);
    let mut ct = Vec::new();
    encrypt_file_stream(Cursor::new(&plaintext), &mut ct, &fek(), &manifest)
        .await
        .unwrap();

    // Flip a byte inside the first chunk's ciphertext body (past the 24-byte
    // nonce and before the 16-byte tag).
    let corrupt_at = aead::NONCE_LEN + 5;
    ct[corrupt_at] ^= 0xFF;

    let err = decrypt_file_stream(Cursor::new(&ct), &mut Vec::new(), &fek(), &manifest)
        .await
        .unwrap_err();
    assert_eq!(err, FileError::TagMismatch, "corrupt chunk must fail auth");
}

#[tokio::test]
async fn t4_resume_deterministic_ciphertext() {
    let plaintext = vec![0x5Au8; 5 * 1024 * 1024 + 777]; // 5 MiB-ish, multi-chunk
    let manifest = make_manifest(plaintext.len() as u64);

    let mut ct1 = Vec::new();
    encrypt_file_stream(Cursor::new(&plaintext), &mut ct1, &fek(), &manifest)
        .await
        .unwrap();
    let mut ct2 = Vec::new();
    encrypt_file_stream(Cursor::new(&plaintext), &mut ct2, &fek(), &manifest)
        .await
        .unwrap();

    assert_eq!(ct1, ct2, "same plaintext + manifest => byte-identical ciphertext (resumable)");
}

#[tokio::test]
async fn t3_wrong_key_fails() {
    let plaintext = vec![0u8; 1024 * 1024 + 3];
    let manifest = make_manifest(plaintext.len() as u64);
    let mut ct = Vec::new();
    encrypt_file_stream(Cursor::new(&plaintext), &mut ct, &fek(), &manifest)
        .await
        .unwrap();
    let wrong = [0x00u8; 32];
    let err = decrypt_file_stream(Cursor::new(&ct), &mut Vec::new(), &wrong, &manifest)
        .await
        .unwrap_err();
    assert_eq!(err, FileError::TagMismatch);
}

#[tokio::test]
async fn t3_truncated_stream_fails() {
    let plaintext = vec![0u8; 3 * 1024 * 1024];
    let manifest = make_manifest(plaintext.len() as u64);
    let mut ct = Vec::new();
    encrypt_file_stream(Cursor::new(&plaintext), &mut ct, &fek(), &manifest)
        .await
        .unwrap();
    ct.truncate(ct.len() / 2); // drop the tail
    let err = decrypt_file_stream(Cursor::new(&ct), &mut Vec::new(), &fek(), &manifest)
        .await
        .unwrap_err();
    assert!(
        matches!(err, FileError::MalformedChunk | FileError::LengthIntegrity),
        "truncated stream must fail: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Property test: deterministic per-chunk nonce is unique per (file_uuid, index)
// and stable across calls.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn t5_nonce_unique_per_chunk(file_uuid in "[0-9a-f]{32}", idx_a in 0u32..1000, idx_b in 0u32..1000) {
        let uuid = Uuid::parse_str(&file_uuid).unwrap_or_else(|_| Uuid::new_v4());
        let na = aead::chunk_nonce(&uuid, idx_a);
        let nb = aead::chunk_nonce(&uuid, idx_b);
        // Determinism
        prop_assert_eq!(na, aead::chunk_nonce(&uuid, idx_a));
        // Uniqueness when indices differ
        if idx_a != idx_b {
            prop_assert_ne!(na, nb, "nonce must differ for different chunk indices");
        }
    }

    #[test]
    fn t5_nonce_unique_per_file(idx in 0u32..1000, a in "[0-9a-f]{32}", b in "[0-9a-f]{32}") {
        let ua = Uuid::parse_str(&a).unwrap_or_else(|_| Uuid::new_v4());
        let ub = Uuid::parse_str(&b).unwrap_or_else(|_| Uuid::new_v4());
        if ua != ub {
            prop_assert_ne!(
                aead::chunk_nonce(&ua, idx),
                aead::chunk_nonce(&ub, idx),
                "nonce must differ across files"
            );
        }
    }
}

// `tokio` is pulled in via dev-dependency for the async test runtime (see #[tokio::test]).

