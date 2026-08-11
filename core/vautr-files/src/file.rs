//! Memory-bounded streaming encryption/decryption of file attachments.
//!
//! Each chunk is an independent AEAD message:
//! `Ciphertext = Nonce(24) || Encrypted_Data || Auth_Tag(16)`
//! with `Nonce = SHA-256(file_uuid ‖ chunk_index)[..24]` and
//! `AD = construct_file_ad(file_uuid, enc_key_gen, chunk_index)`.
//!
//! Buffering is bounded to ~2 MiB (one plaintext chunk + one ciphertext chunk)
//! regardless of file size. Encryption/decryption proceed chunk-by-chunk over
//! async streams, so the plaintext and ciphertext are never both fully resident
//! in memory.

use crate::error::{FileError, Result};
use crate::manifest::FileManifest;
use futures_util::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;
use vautr_crypto::aead;

const PLAINTEXT_CHUNK: usize = 1 << 20; // 1 MiB, matches manifest::CHUNK_SIZE

/// Encrypt `input` to `output` in 1 MiB chunks under `key` (the per-file FEK).
///
/// `manifest` supplies `file_uuid`, `enc_key_gen`, and `total_size`. The
/// manifest's `total_chunks` is recomputed from `total_size`; the function does
/// not rely on a pre-stored chunk count, which is what makes resumption from a
/// partially-written stream work: a restarted encryption of the same plaintext
/// produces byte-identical ciphertext.
///
/// Memory is bounded: at most one plaintext chunk and one ciphertext chunk are
/// held at once.
pub async fn encrypt_file_stream(
    mut input: impl AsyncRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
    key: &[u8; 32],
    manifest: &FileManifest,
) -> Result<()> {
    manifest.validate()?;

    let file_uuid: &Uuid = &manifest.file_uuid;
    let enc_key_gen = manifest.enc_key_gen;
    let mut chunk_index: u32 = 0;
    let mut remaining = manifest.total_size;
    let mut plaintext = vec![0u8; PLAINTEXT_CHUNK];

    loop {
        let to_read = (remaining as usize).min(PLAINTEXT_CHUNK);
        if to_read == 0 {
            break;
        }
        let n = input.read_exact_or_eof(&mut plaintext[..to_read]).await?;
        if n == 0 {
            // No more data. If `remaining` was non-zero this means the stream
            // ended early; surface it as a length-integrity error.
            if remaining != 0 {
                return Err(FileError::LengthIntegrity);
            }
            break;
        }
        let chunk = &plaintext[..n];
        let nonce = aead::chunk_nonce(file_uuid, chunk_index);
        let ad = aead::construct_file_ad(file_uuid, enc_key_gen, chunk_index);
        let envelope = aead::encrypt_with_nonce(key, &nonce, &ad, chunk)?;
        output.write_all(&envelope).await?;
        output.flush().await?;

        remaining -= n as u64;
        chunk_index += 1;
    }

    if remaining != 0 {
        return Err(FileError::LengthIntegrity);
    }
    Ok(())
}

/// Decrypt `input` to `output` using `manifest`.
///
/// `manifest.total_size` is used to verify the final plaintext length. A
/// corrupted chunk fails fast with [`FileError::TagMismatch`] at that chunk;
/// truncation or trailing garbage fails with [`FileError::LengthIntegrity`].
pub async fn decrypt_file_stream(
    mut input: impl AsyncRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
    key: &[u8; 32],
    manifest: &FileManifest,
) -> Result<()> {
    manifest.validate()?;

    let file_uuid: &Uuid = &manifest.file_uuid;
    let enc_key_gen = manifest.enc_key_gen;
    let mut chunk_index: u32 = 0;
    let mut remaining = manifest.total_size;
    let mut header = [0u8; aead::NONCE_LEN];
    let mut ciphertext = Vec::with_capacity(PLAINTEXT_CHUNK + aead::TAG_LEN);

    loop {
        if remaining == 0 {
            // Ensure there is no trailing ciphertext beyond the declared size.
            let mut probe = [0u8; 1];
            match input.read(&mut probe).await {
                Ok(0) => break,
                Ok(_) => return Err(FileError::LengthIntegrity),
                Err(e) => return Err(FileError::Io(e.to_string())),
            }
        }

        // On-wire chunk = Nonce(24) || (Plaintext || Tag). Read the nonce first.
        if let Err(_) = input.read_exact(&mut header).await {
            // Stream ended where a chunk was expected.
            return Err(FileError::MalformedChunk);
        }

        // The body after the nonce is exactly (plaintext_len + TAG_LEN).
        let plaintext_budget = manifest.chunk_len(chunk_index) as usize;
        let ct_budget = plaintext_budget + aead::TAG_LEN;
        ciphertext.clear();
        ciphertext.resize(ct_budget, 0);
        if let Err(_) = input.read_exact(&mut ciphertext).await {
            return Err(FileError::MalformedChunk);
        }

        let nonce = aead::chunk_nonce(file_uuid, chunk_index);
        let ad = aead::construct_file_ad(file_uuid, enc_key_gen, chunk_index);
        let mut envelope = Vec::with_capacity(header.len() + ciphertext.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);
        let plaintext_chunk = aead::decrypt_with_ad(key, &ad, &envelope)?;

        if plaintext_chunk.len() as u64 != manifest.chunk_len(chunk_index) {
            return Err(FileError::LengthIntegrity);
        }
        output.write_all(&plaintext_chunk).await?;
        output.flush().await?;

        remaining -= plaintext_chunk.len() as u64;
        chunk_index += 1;
    }

    if remaining != 0 {
        return Err(FileError::LengthIntegrity);
    }
    Ok(())
}

/// Read exactly `buf.len()` bytes, or fewer if EOF is reached first.
///
/// Returns the number of bytes read. Only errors on real I/O failures.
trait AsyncReadExactOrEof {
    async fn read_exact_or_eof(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
}

impl<R: AsyncRead + Unpin> AsyncReadExactOrEof for R {
    async fn read_exact_or_eof(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut filled = 0;
        while filled < buf.len() {
            let n = self.read(&mut buf[filled..]).await?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        Ok(filled)
    }
}
