//! `FileManifest`: the metadata describing an encrypted attachment.
//!
//! Lives inside an item's standard `DomainModel` payload (file-storage.md §2.3).
//! The manifest tracks chunk geometry and upload state so the `FileTransferWorker`
//! can resume interrupted transfers without re-reading the plaintext.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Chunk size for streaming encryption (file-storage.md §2.1).
pub const CHUNK_SIZE: u32 = 1 << 20; // 1 MiB

/// Upload/sync lifecycle of an attachment (file-storage.md §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentState {
    /// Local only; chunks are uploading to object storage. NOT synced to server.
    PendingUpload,
    /// Upload complete; chunks exist on object storage. Synced to server.
    Available,
}

/// Metadata describing a single encrypted attachment.
///
/// Mirrors `docs/architecture/file-storage.md` §2.3 `FileManifest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl FileManifest {
    /// Build a manifest for a file of `total_size` bytes.
    ///
    /// `total_chunks` is `ceil(total_size / chunk_size)`, with a 0-byte file
    /// occupying exactly one (empty) chunk so the chunk index space is never
    /// empty.
    pub fn new(
        file_uuid: Uuid,
        total_size: u64,
        enc_key_gen: u64,
        content_type: String,
        last_modified: i64,
    ) -> Self {
        let chunk_size = CHUNK_SIZE;
        let total_chunks = total_size.div_ceil(chunk_size as u64).max(1) as u32;
        Self {
            file_uuid,
            total_size,
            chunk_size,
            total_chunks,
            enc_key_gen,
            content_type,
            last_modified,
            status: AttachmentState::PendingUpload,
        }
    }

    /// Size in bytes of chunk `index` (0-based). The final chunk may be smaller
    /// than `chunk_size`; the last derived chunk for a whole file is always
    /// `total_size - index*chunk_size`.
    pub fn chunk_len(&self, index: u32) -> u64 {
        let chunk_size = self.chunk_size as u64;
        let offset = index as u64 * chunk_size;
        (self.total_size.saturating_sub(offset)).min(chunk_size)
    }

    /// Validate internal geometry (chunk_size, total_chunks, total_size agree).
    pub fn validate(&self) -> Result<(), crate::error::FileError> {
        if self.chunk_size == 0 {
            return Err(crate::error::FileError::ManifestGeometry(
                "chunk_size must be non-zero".into(),
            ));
        }
        let expected = self.total_size.div_ceil(self.chunk_size as u64).max(1) as u32;
        if self.total_chunks != expected {
            return Err(crate::error::FileError::ManifestGeometry(format!(
                "total_chunks {} != expected {} for total_size {} / chunk_size {}",
                self.total_chunks, expected, self.total_size, self.chunk_size
            )));
        }
        Ok(())
    }
}
