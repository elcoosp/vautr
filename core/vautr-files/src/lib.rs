//! Streaming, memory-bounded file encryption for Vautr attachments.
//!
//! Spec: [`docs/architecture/file-storage.md`]. Binary attachments >= 10 MiB
//! are split into 1 MiB chunks, each encrypted independently with
//! XChaCha20-Poly1305 using a deterministic per-chunk nonce and a 3-field
//! Associated Data binding `(file_uuid, enc_key_gen, chunk_index)`.
//!
//! Memory is bounded to roughly two chunk buffers (plaintext + ciphertext)
//! regardless of file size: the stream is read, encrypted, and written chunk by
//! chunk. No whole-file buffering.
//!
//! **Nonce policy (deviation from crypto.md §2.8):** the per-chunk nonce is
//! *deterministic* (`SHA-256(file_uuid ‖ chunk_index)[..24]`), which lets a
//! resumable upload restart from `FileManifest.total_chunks` without persisting
//! per-chunk nonces. This is safe because the nonce is bound to
//! `(file_uuid, chunk_index)` and the key (`FEK`) is unique per file. See
//! `vautr_crypto::aead` for the full rationale. The randomized-nonce AEAD path
//! remains the default for item payloads.

pub mod error;
pub mod file;
pub mod manifest;
pub mod pdf;

pub use error::{FileError, Result};
pub use file::{decrypt_file_stream, encrypt_file_stream};
pub use manifest::{AttachmentState, FileManifest};
