//! Translation layer (data-import-seeding.md §2.3): raw → Vautr `DomainModel`.
//!
//! Enforces the **UUID Mandate**: competitor identifiers are discarded and a
//! fresh `Uuid::new_v4()` is generated per item. The competitor id is preserved
//! only as opaque provenance in [`TranslatedItem::source_id`].
//!
//! Deduplication uses a pre-flight `HashSet<(title, url)>` built from the local
//! DB (see [`crate::preflight`]); FTS5 is never used for dedup. Records are
//! translated independently and failures accumulate as [`ImportError`]s rather
//! than aborting the pipeline.

use std::collections::HashSet;

use serde_json::Value;
use uuid::Uuid;
use vautr_domain::{
    CustomField, CustomFieldType, DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata,
    TotpAlgorithm, TotpSecret,
};
use zeroize::Zeroizing;

use crate::error::{ImportError, ImportFailureReason};
use crate::RawImportItem;

/// A successfully translated domain model plus its preserved source provenance.
#[derive(Debug, Clone)]
pub struct TranslatedItem {
    pub domain: DomainModel,
    pub source_id: Option<String>,
}

/// Extract a string field from the record's `fields` JSON object.
fn field_str(fields: &Value, key: &str) -> Option<String> {
    fields.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Translate one raw record. Returns an [`ImportError`] for per-record failures
/// (missing title, duplicate) so the pipeline can accumulate them.
pub fn translate(
    raw: RawImportItem,
    existing: &HashSet<(String, String)>,
    line: u32,
) -> Result<TranslatedItem, ImportError> {
    let item_identifier = format!("Title: {}", raw.title);

    let title = raw.title.trim().to_string();
    if title.is_empty() {
        return Err(ImportError {
            line_number: line,
            item_identifier,
            reason: ImportFailureReason::MissingRequiredField("title".to_string()),
        });
    }

    let url = raw.url.clone().unwrap_or_default();
    let dedup_key = (title.clone(), url.clone());

    // Exact-match dedup against the pre-flight set.
    if existing.contains(&dedup_key) {
        return Err(ImportError {
            line_number: line,
            item_identifier,
            reason: ImportFailureReason::DuplicateSkip,
        });
    }

    let password = field_str(&raw.fields, "password").unwrap_or_default();
    let notes = field_str(&raw.fields, "notes").unwrap_or_default();
    let username = field_str(&raw.fields, "username");

    // The UUID Mandate: always a fresh, random v4.
    let uuid = Uuid::new_v4();
    let enc_key_gen: u64 = 1;
    let now = crate::now_unix();

    let mut custom_fields = Vec::new();
    if let Some(u) = username.clone() {
        custom_fields.push(CustomField {
            name: "username".to_string(),
            value: Zeroizing::new(u),
            field_type: CustomFieldType::Text,
        });
    }

    // TOTP optional; tolerate a malformed secret (bad base32) as a per-item
    // validation failure rather than a panic.
    // VTRFIX-BUG-H08: TOTP must never be silently dropped. Accept either a
    // bare base32 secret (length >= 8) or a full `otpauth://` URI. A non-empty
    // value that fails validation becomes a per-item error, not a silent skip.
    let mut totp = None;
    if let Some(secret) = field_str(&raw.fields, "totp") {
        let s = secret.trim();
        if !s.is_empty() {
            if s.starts_with("otpauth://") {
                // Store the URI verbatim — downstream clients parse it.
                totp = Some(TotpSecret {
                    algorithm: TotpAlgorithm::Sha1,
                    digits: 6,
                    period: 30,
                    secret_base32: Zeroizing::new(s.to_string()),
                });
            } else if s.len() >= 8 {
                totp = Some(TotpSecret {
                    algorithm: TotpAlgorithm::Sha1,
                    digits: 6,
                    period: 30,
                    secret_base32: Zeroizing::new(s.to_string()),
                });
            } else {
                return Err(ImportError {
                    line_number: line,
                    item_identifier: item_identifier.clone(),
                    reason: ImportFailureReason::SchemaMismatch(format!(
                        "invalid totp secret ({} chars; need >= 8 or otpauth:// URI)",
                        s.len()
                    )),
                });
            }
        }
    }

    let urls: Vec<String> = if url.is_empty() {
        Vec::new()
    } else {
        vec![url]
    };

    let overview = DecryptedOverview {
        uuid,
        title: title.clone(),
        // VTRFIX-BUG-H09: username is the canonical subtitle everywhere else
        // in the app; keep it in sync so import -> export round trips.
        subtitle: username.clone().unwrap_or_default(),
        icon_key: "generic".to_string(),
        urls,
        updated_at: now,
    };

    let secret = DecryptedSecret {
        password: Zeroizing::new(password),
        totp,
        notes: Zeroizing::new(notes),
        fields: custom_fields,
    };

    let metadata = ItemMetadata {
        created_at: now,
        updated_at: now,
        trashed: false,
    };

    Ok(TranslatedItem {
        domain: DomainModel {
            uuid,
            enc_key_gen,
            overview,
            secret,
            metadata,
        },
        source_id: raw.source_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ImportFailureReason;

    fn raw(title: &str, url: Option<&str>) -> RawImportItem {
        RawImportItem {
            source_id: Some("competitor-id".to_string()),
            title: title.to_string(),
            url: url.map(|s| s.to_string()),
            fields: Value::Object(Default::default()),
        }
    }

    #[test]
    fn empty_title_is_missing_required_field() {
        let existing = HashSet::new();
        let err = translate(raw("  ", None), &existing, 1).unwrap_err();
        assert!(matches!(
            err.reason,
            ImportFailureReason::MissingRequiredField(_)
        ));
        assert_eq!(err.line_number, 1);
    }

    #[test]
    fn duplicate_is_skipped() {
        let mut existing = HashSet::new();
        existing.insert(("Bank".to_string(), "https://bank.example".to_string()));
        let err = translate(raw("Bank", Some("https://bank.example")), &existing, 5).unwrap_err();
        assert_eq!(err.reason, ImportFailureReason::DuplicateSkip);
    }

    #[test]
    fn uuid_mandate_generates_fresh_random_uuid() {
        let existing = HashSet::new();
        let a = translate(raw("Alpha", Some("https://a.example")), &existing, 1)
            .unwrap()
            .domain
            .uuid;
        let b = translate(raw("Beta", Some("https://b.example")), &existing, 2)
            .unwrap()
            .domain
            .uuid;
        assert_ne!(a, b);
        // Source provenance preserved separately.
    }

    #[test]
    fn source_id_preserved_as_provenance() {
        let existing = HashSet::new();
        let t = translate(raw("Gamma", Some("https://g.example")), &existing, 1).unwrap();
        assert_eq!(t.source_id.as_deref(), Some("competitor-id"));
    }
}
