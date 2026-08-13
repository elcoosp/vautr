//! `FileTransferWorker` — drives the multipart attachment protocol
//! (file-storage.md §4-5) against a [`FileTransport`].
//!
//! The worker:
//!   1. derives the per-file FEK (`HKDF(SVK, "vautr-fek-{file_uuid}")`, §2.2),
//!   2. streams plaintext → chunk ciphertext via `vautr-files` (§2),
//!   3. initiates an upload, PUTs each encrypted chunk, then completes (§4.1),
//!   4. emits `VaultStateUpdate::FileTransferProgress` **throttled to ≤4/s** (§5.2),
//!   5. supports on-demand download + decrypt (§4.2).
//!
//! Transfers are decoupled from the interactive `PersistenceWorker` so large
//! binaries never starve the UI queue (§1.3).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_crypto::aead;
use vautr_files::manifest::FileManifest;
use vautr_files::{decrypt_file_stream, encrypt_file_stream};

use crate::event_bus::{EventBus, VaultStateUpdate};

/// Max UI progress events per second (file-storage.md §5.2).
const MAX_EVENTS_PER_SEC: u64 = 4;
/// Throttle window derived from the rate limit.
const THROTTLE: Duration = Duration::from_millis(1000 / MAX_EVENTS_PER_SEC);

/// Handle type for the file transport.
pub type FileTransportHandle = Arc<dyn FileTransport>;

/// The blob-store boundary (§4): a dumb relay that never sees the FEK.
pub trait FileTransport: Send + Sync {
    /// Create an upload session for a `PendingUpload` manifest (§4.1 step 1).
    fn initiate_upload(
        &self,
        manifest: &FileManifest,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Push one encrypted chunk to the object store (§4.1 step 2).
    fn upload_chunk(
        &self,
        file_uuid: Uuid,
        index: u32,
        ciphertext: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Atomically mark a fully-uploaded file `Available` (§4.1 step 4).
    fn complete_upload(
        &self,
        file_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
    /// Fetch one encrypted chunk for on-demand download (§4.2).
    fn fetch_chunk(
        &self,
        file_uuid: Uuid,
        index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send>>;
}

/// Derive the per-file encryption key (§2.2): `FEK = HKDF(SVK, "vautr-fek-{file_uuid}")`.
fn derive_fek(svk: &[u8; 32], file_uuid: &Uuid) -> Zeroizing<[u8; 32]> {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, svk);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(format!("vautr-fek-{file_uuid}").as_bytes(), &mut *okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Drive multipart uploads/downloads with progress throttling (§5).
pub struct FileTransferWorker {
    transport: FileTransportHandle,
    bus: EventBus,
    last_progress: Mutex<Option<Instant>>,
    events_emitted: AtomicU64,
}

impl FileTransferWorker {
    /// Build a worker over a file transport + event bus.
    pub fn new(transport: FileTransportHandle, bus: EventBus) -> Self {
        Self {
            transport,
            bus,
            last_progress: Mutex::new(None),
            events_emitted: AtomicU64::new(0),
        }
    }

    /// Total progress events emitted so far (introspection/tests).
    pub fn events_emitted(&self) -> u64 {
        self.events_emitted.load(Ordering::SeqCst)
    }

    /// Throttle gate: returns `true` only when the throttle window has elapsed.
    fn throttled(&self) -> bool {
        let now = Instant::now();
        let mut last = self.last_progress.lock().unwrap();
        match *last {
            Some(t) if now.duration_since(t) < THROTTLE => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }

    fn emit_progress(&self, file_uuid: Uuid, bytes_transferred: u64, total_bytes: u64) {
        if self.throttled() {
            self.events_emitted.fetch_add(1, Ordering::SeqCst);
            self.bus.publish(VaultStateUpdate::FileTransferProgress {
                file_uuid,
                bytes_transferred,
                total_bytes,
            });
        }
    }

    /// Upload `plaintext` as a new attachment (§4.1). Returns the finalized
    /// manifest (status `Available`).
    pub async fn upload_bytes(
        &self,
        svk: &[u8; 32],
        plaintext: &[u8],
        content_type: &str,
        last_modified: i64,
    ) -> Result<FileManifest, String> {
        let file_uuid = Uuid::new_v4();
        let manifest = FileManifest::new(
            file_uuid,
            plaintext.len() as u64,
            0, // FEK is independent of enc_key_gen; placeholder 0 per §2.2
            content_type.to_string(),
            last_modified,
        );
        manifest
            .validate()
            .map_err(|e| format!("manifest validate: {e}"))?;

        // 1. Encrypt the whole plaintext stream to chunk ciphertext.
        let fek = derive_fek(svk, &file_uuid);
        let mut ct = Vec::new();
        encrypt_file_stream(
            futures_util::io::Cursor::new(plaintext),
            &mut ct,
            &fek,
            &manifest,
        )
        .await
        .map_err(|e| format!("encrypt stream: {e}"))?;

        // 2. Initiate the multipart session.
        self.transport
            .initiate_upload(&manifest)
            .await
            .map_err(|e| format!("initiate upload: {e}"))?;

        // 3. Stream each encrypted chunk (nonce(24) || ciphertext || tag(16)).
        let mut offset = 0usize;
        let total = manifest.total_size;
        for index in 0..manifest.total_chunks {
            let plaintext_len = manifest.chunk_len(index) as usize;
            let envelope_len = aead::NONCE_LEN + plaintext_len + aead::TAG_LEN;
            let end = (offset + envelope_len).min(ct.len());
            let envelope = ct[offset..end].to_vec();
            offset = end;
            self.transport
                .upload_chunk(file_uuid, index, envelope)
                .await
                .map_err(|e| format!("upload chunk {index}: {e}"))?;
            // Throttled progress: bytes transferred = plaintext bytes pushed.
            let transferred = ((index as u64 + 1) * manifest.chunk_size as u64).min(total);
            self.emit_progress(file_uuid, transferred, total);
        }

        // 4. Complete the upload → manifest becomes Available.
        self.transport
            .complete_upload(file_uuid)
            .await
            .map_err(|e| format!("complete upload: {e}"))?;

        Ok(manifest)
    }

    /// Download an attachment and decrypt it back to plaintext (§4.2).
    pub async fn download(
        &self,
        svk: &[u8; 32],
        manifest: &FileManifest,
    ) -> Result<Vec<u8>, String> {
        let fek = derive_fek(svk, &manifest.file_uuid);
        let mut ciphertext = Vec::new();
        for index in 0..manifest.total_chunks {
            let chunk = self
                .transport
                .fetch_chunk(manifest.file_uuid, index)
                .await
                .map_err(|e| format!("fetch chunk {index}: {e}"))?;
            ciphertext.extend_from_slice(&chunk);
            let transferred =
                ((index as u64 + 1) * manifest.chunk_size as u64).min(manifest.total_size);
            self.emit_progress(manifest.file_uuid, transferred, manifest.total_size);
        }
        let mut plaintext = Vec::new();
        decrypt_file_stream(
            futures_util::io::Cursor::new(&ciphertext),
            &mut plaintext,
            &fek,
            manifest,
        )
        .await
        .map_err(|e| format!("decrypt stream: {e}"))?;
        Ok(plaintext)
    }
}
