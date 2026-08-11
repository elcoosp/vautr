//! Size-aware parallel encryption (data-import-seeding.md §3.1).
//!
//! The stream of valid `DomainModel`s is chunked at **100 items OR 25MB of
//! plaintext**, and each chunk is dispatched to a `rayon` pool. Every item is
//! encrypted under its own unique random XChaCha20-Poly1305 nonce via
//! `vautr-crypto::aead`, so parallel encryption introduces no nonce-misuse risk.
//!
//! Output pairs are the local-DB entities: a plaintext `item_overview` (hot
//! data, searchable by FTS5) and a DEK-encrypted `item_payload` (cold data).

use rayon::prelude::*;
use sea_orm::Set;
use vautr_crypto::aead;
use vautr_domain::DomainModel;

use crate::error::ImportFailure;
use crate::VaultKeys;

/// The encrypted DB entities for one item (hot overview + cold payload).
#[derive(Debug)]
pub struct EncryptedItem {
    pub overview: vautr_db::entity::item_overview::ActiveModel,
    pub payload: vautr_db::entity::item_payload::ActiveModel,
}

/// Upper bound on items per chunk before it is dispatched to the pool.
pub const MAX_ITEMS_PER_CHUNK: usize = 100;
/// Upper bound on plaintext bytes per chunk (25MB) before dispatch.
pub const MAX_PLAINTEXT_BYTES: usize = 25 * 1024 * 1024;

/// Approximate plaintext size of a domain model (used for size-aware chunking).
pub fn plaintext_size(item: &DomainModel) -> usize {
    serde_json::to_vec(item).map(|v| v.len()).unwrap_or(256)
}

/// Encrypt a single domain model into (overview, payload) DB entities.
pub fn encrypt_item(item: &DomainModel, keys: &VaultKeys) -> Result<EncryptedItem, String> {
    let uuid = item.uuid;
    let gen = item.enc_key_gen;

    // Cold data: DEK-encrypted DecryptedSecret with AD = (uuid, enc_key_gen).
    let secret_json = serde_json::to_vec(&item.secret).map_err(|e| e.to_string())?;
    let payload = aead::encrypt(&keys.dek, &uuid, gen, &secret_json)
        .map_err(|e| format!("encrypt payload: {e}"))?;

    // Hot data: plaintext overview for the local DB / FTS5. The OEK-encrypted
    // overview is what the sync layer later pushes to the server; here we store
    // the plaintext so search stays local (crypto.md §4).
    let urls_json = serde_json::to_string(&item.overview.urls).unwrap_or_else(|_| "[]".to_string());
    let overview = vautr_db::entity::item_overview::ActiveModel {
        uuid: Set(uuid.to_string()),
        version: Set(1),
        enc_key_gen: Set(gen as i64),
        deleted_date: Set(None),
        overview_title: Set(item.overview.title.clone()),
        overview_subtitle: Set(item.overview.subtitle.clone()),
        overview_icon_key: Set(item.overview.icon_key.clone()),
        overview_urls: Set(urls_json),
        created_at: Set(item.metadata.created_at),
        updated_at: Set(item.metadata.updated_at),
    };
    let payload_model = vautr_db::entity::item_payload::ActiveModel {
        uuid: Set(uuid.to_string()),
        payload: Set(payload),
    };

    Ok(EncryptedItem {
        overview,
        payload: payload_model,
    })
}

/// Encrypt a chunk of items in parallel on the `rayon` pool.
pub fn encrypt_chunk(
    chunk: Vec<DomainModel>,
    keys: &VaultKeys,
) -> Result<Vec<EncryptedItem>, ImportFailure> {
    let results: Result<Vec<_>, String> = chunk
        .into_par_iter()
        .map(|d| encrypt_item(&d, keys))
        .collect();
    results.map_err(|e| ImportFailure::Pipeline(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::{self, TranslatedItem};
    use crate::{VaultKeys};
    use std::collections::HashSet;

    fn one_domain(title: &str) -> DomainModel {
        let raw = crate::RawImportItem {
            source_id: None,
            title: title.to_string(),
            url: Some(format!("https://{title}.example")),
            fields: serde_json::json!({"password": "hunter2"}),
        };
        let TranslatedItem { domain, .. } =
            translate::translate(raw, &HashSet::new(), 1).unwrap();
        domain
    }

    #[test]
    fn encrypts_chunk_in_parallel() {
        let keys = VaultKeys::random();
        let chunk: Vec<DomainModel> = (0..250).map(|i| one_domain(&format!("item{i}"))).collect();
        let out = encrypt_chunk(chunk, &keys).unwrap();
        assert_eq!(out.len(), 250);
        // Every payload is non-empty ciphertext (nonce 24 + ct + tag).
        for e in &out {
            let blob = e.payload.payload.clone().unwrap();
            assert!(blob.len() > 24 + 16);
        }
        // Unique nonces -> all ciphertexts differ.
        let first = out[0].payload.payload.clone().unwrap();
        assert!(out
            .iter()
            .skip(1)
            .all(|e| e.payload.payload.clone().unwrap() != first));
    }
}
