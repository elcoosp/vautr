//! Streaming, zero-trust parsers for CSV and Bitwarden JSON (data-import-seeding.md §2.3).
//!
//! Both parsers yield `ParseRecord` values one at a time and never panic on
//! malformed input — a bad record becomes an `Err(String)` that the pipeline
//! accumulates, while the stream continues.

use std::collections::VecDeque;
use std::io::BufRead;

use serde_json::Value;

use crate::error::Result as CrateResult;
use crate::source::SourceKind;
use crate::RawImportItem;

/// A single parsed record plus its 1-based line/record number in the source.
#[derive(Debug, Clone)]
pub struct ParseRecord {
    pub line_number: u32,
    pub item: RawImportItem,
}

/// Build the appropriate streaming iterator for a source.
///
/// - CSV: a live [`CsvIter`] that pulls records from the reader on demand.
/// - Bitwarden JSON / extracted 1pux: a `serde_json::StreamDeserializer` over
///   the top-level document, with the `items` array drained item-by-item.
pub fn parse_stream(
    reader: Box<dyn BufRead>,
    kind: SourceKind,
) -> CrateResult<Box<dyn Iterator<Item = Result<ParseRecord, String>>>> {
    match kind {
        SourceKind::Csv => Ok(Box::new(CsvIter::new(reader))),
        SourceKind::Pif1Password => {
            // VTRFIX-BUG-M18: parse 1PIF directly (line-delimited JSON records
            // separated by `***...***` marker rows). Previously this was routed
            // to the ZIP parser and always failed.
            let mut text = String::new();
            use std::io::Read;
            let mut reader = reader;
            reader
                .read_to_string(&mut text)
                .map_err(|e| crate::error::ImportFailure::Pipeline(format!("read 1pif: {e}")))?;
            let mut records: VecDeque<Result<ParseRecord, String>> = VecDeque::new();
            let mut line_no = 0u32;
            let mut buf = String::new();
            let flush =
                |buf: &mut String,
                 line: u32,
                 records: &mut VecDeque<Result<ParseRecord, String>>| {
                    let trimmed = buf.trim();
                    if trimmed.is_empty() {
                        return;
                    }
                    match serde_json::from_str::<Value>(trimmed) {
                        Ok(v) => {
                            // Map to the same Bitwarden-shaped `fields` object the
                            // CSV/Bitwarden parsers emit (translate.rs reads
                            // username/password/notes/totp from it).
                            // VTRFIX (silent data loss): the previous mapping
                            // extracted `secureContents.fields` and then discarded
                            // it (`let _ = login`), importing every 1PIF item as a
                            // blank row while the report counted a success.
                            let secure = v.get("secureContents");
                            let fields_arr = secure
                                .and_then(|s| s.get("fields"))
                                .and_then(|f| f.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let mut fields = serde_json::Map::new();
                            for f in &fields_arr {
                                let value = f.get("value").and_then(|v| v.as_str());
                                let Some(value) = value else {
                                    continue;
                                };
                                if value.is_empty() {
                                    continue;
                                }
                                let designation = f
                                    .get("designation")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or_default()
                                    .to_ascii_lowercase();
                                let name = f
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or_default()
                                    .to_ascii_lowercase();
                                if designation == "username" || name == "username" {
                                    fields.insert("username".into(), Value::String(value.into()));
                                } else if designation == "password" || name == "password" {
                                    fields.insert("password".into(), Value::String(value.into()));
                                } else if name.contains("totp") || value.starts_with("otpauth://") {
                                    fields.insert("totp".into(), Value::String(value.into()));
                                }
                            }
                            if let Some(notes) = secure
                                .and_then(|s| s.get("notesPlain"))
                                .and_then(|n| n.as_str())
                                .filter(|n| !n.is_empty())
                            {
                                fields.insert("notes".into(), Value::String(notes.into()));
                            }
                            let url = v
                                .get("location")
                                .and_then(|l| l.as_str())
                                .filter(|l| !l.is_empty())
                                .map(String::from)
                                .or_else(|| {
                                    v.get("urls")
                                        .and_then(|u| u.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|u| u.get("url"))
                                        .and_then(|u| u.as_str())
                                        .filter(|u| !u.is_empty())
                                        .map(String::from)
                                });
                            let title = v
                                .get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or_default()
                                .to_string();
                            records.push_back(Ok(ParseRecord {
                                line_number: line,
                                item: RawImportItem {
                                    source_id: v
                                        .get("uuid")
                                        .and_then(|u| u.as_str())
                                        .map(String::from),
                                    title,
                                    url,
                                    fields: Value::Object(fields),
                                },
                            }));
                        }
                        Err(e) => {
                            records.push_back(Err(format!("line {line}: {e}")));
                        }
                    }
                    buf.clear();
                };
            for line in text.lines() {
                line_no += 1;
                // 1PIF separator: a line of three or more asterisks.
                if line.starts_with("***") {
                    flush(&mut buf, line_no, &mut records);
                    continue;
                }
                buf.push_str(line);
                buf.push('\n');
            }
            flush(&mut buf, line_no, &mut records);
            Ok(Box::new(records.into_iter()))
        }
        SourceKind::BitwardenJson | SourceKind::Zip1pux => {
            let de = serde_json::Deserializer::from_reader(reader);
            let stream = de.into_iter::<Value>();
            let mut records: VecDeque<Result<ParseRecord, String>> = VecDeque::new();
            // VTRFIX-BUG-H11: propagate stream errors instead of `.flatten()`
            // silently dropping the offending line and reporting success.
            for value in stream {
                let value = match value {
                    Ok(v) => v,
                    Err(e) => {
                        records.push_back(Err(format!("line {}: {e}", e.line())));
                        continue;
                    }
                };
                let items = value.get("items").and_then(|v| v.as_array());
                let items = match items {
                    Some(a) => a,
                    None => continue,
                };
                for (i, item) in items.iter().enumerate() {
                    records.push_back(parse_bitwarden_item(item, (i + 1) as u32));
                }
            }
            Ok(Box::new(records.into_iter()))
        }
    }
}

/// Streaming CSV iterator. Each call to `next()` reads exactly one record.
pub struct CsvIter {
    reader: csv::Reader<Box<dyn BufRead>>,
    headers: Vec<String>,
    line: u32,
}

impl CsvIter {
    pub fn new(reader: Box<dyn BufRead>) -> Self {
        let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(reader);
        let headers = reader
            .headers()
            .map(|h| h.iter().map(|s| s.to_ascii_lowercase()).collect())
            .unwrap_or_default();
        Self {
            reader,
            headers,
            line: 1,
        }
    }

    /// VTRFIX-BUG-H10: look up a CSV cell and preserve it verbatim. Only
    /// presentation fields (title/url) are trimmed later by the caller.
    fn field_raw(&self, rec: &csv::StringRecord, keys: &[&str]) -> Option<String> {
        let idx = keys
            .iter()
            .find_map(|k| self.headers.iter().position(|h| h == *k))?;
        rec.get(idx)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }

    /// Look up a CSV cell and trim surrounding whitespace (presentation only).
    fn field_trim(&self, rec: &csv::StringRecord, keys: &[&str]) -> Option<String> {
        self.field_raw(rec, keys)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn map_record(&mut self, rec: &csv::StringRecord) -> ParseRecord {
        let title = self.field_trim(rec, &["name", "title"]).unwrap_or_default();
        let url = self.field_trim(rec, &["url", "uri", "login_uri"]);
        // VTRFIX-BUG-H10: these must be preserved byte-for-byte; a password
        // like " p@ss " is legitimate and would otherwise be silently corrupted.
        let username = self.field_raw(rec, &["username", "user"]);
        let password = self.field_raw(rec, &["password"]);
        let notes = self.field_raw(rec, &["notes", "note"]);
        // VTRFIX-BUG-H08: TOTP must round-trip through import.
        let totp = self.field_raw(rec, &["totp", "totp_secret", "otpauth"]);

        let mut fields = serde_json::Map::new();
        if let Some(u) = username {
            fields.insert("username".into(), Value::String(u));
        }
        if let Some(p) = password {
            fields.insert("password".into(), Value::String(p));
        }
        if let Some(n) = notes {
            fields.insert("notes".into(), Value::String(n));
        }
        if let Some(t) = totp {
            fields.insert("totp".into(), Value::String(t));
        }

        let line = self.line;
        ParseRecord {
            line_number: line,
            item: RawImportItem {
                source_id: None,
                title,
                url,
                fields: Value::Object(fields),
            },
        }
    }
}

impl Iterator for CsvIter {
    type Item = Result<ParseRecord, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut rec = csv::StringRecord::new();
        match self.reader.read_record(&mut rec) {
            Ok(true) => {
                self.line += 1;
                Some(Ok(self.map_record(&rec)))
            }
            Ok(false) => None,
            Err(e) => Some(Err(e.to_string())),
        }
    }
}

/// Map a single Bitwarden JSON item object to a [`RawImportItem`].
fn parse_bitwarden_item(item: &Value, line: u32) -> Result<ParseRecord, String> {
    let source_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let title = item
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let login = item.get("login");
    let url = login
        .and_then(|l| l.get("uris"))
        .and_then(|u| u.as_array())
        .and_then(|arr| arr.first())
        .and_then(|u| u.get("uri"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let username = login
        .and_then(|l| l.get("username"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let password = login
        .and_then(|l| l.get("password"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let notes = item
        .get("notes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    // VTRFIX-BUG-H08: Bitwarden stores TOTP at `login.totp` (either a bare
    // base32 secret or a full otpauth:// URI). Preserve it for the translator.
    let totp = login
        .and_then(|l| l.get("totp"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut fields = serde_json::Map::new();
    if let Some(u) = username {
        fields.insert("username".into(), Value::String(u));
    }
    if let Some(p) = password {
        fields.insert("password".into(), Value::String(p));
    }
    if let Some(n) = notes {
        fields.insert("notes".into(), Value::String(n));
    }
    if let Some(t) = totp {
        fields.insert("totp".into(), Value::String(t));
    }

    Ok(ParseRecord {
        line_number: line,
        item: RawImportItem {
            source_id,
            title,
            url,
            fields: Value::Object(fields),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceKind;
    use std::io::Cursor;

    /// VTRFIX regression: 1PIF records must map `secureContents.fields`
    /// (designation/name/value), `notesPlain`, and the location into the same
    /// Bitwarden-shaped `fields` object every other parser emits — the old
    /// mapping discarded them and imported blank rows.
    #[test]
    fn parses_1pif_fields_not_blank() {
        let rec1 = serde_json::json!({
            "uuid": "A1",
            "title": "GitHub",
            "location": "https://github.com",
            "secureContents": {
                "notesPlain": "work account",
                "fields": [
                    {"designation": "username", "name": "username", "value": "octocat"},
                    {"designation": "password", "name": "password", "value": " hunter2 "},
                    {"name": "TOTP Authenticator", "value": "otpauth://totp/x?secret=ABC"}
                ]
            }
        });
        let sample = format!(
            "******\n{}\n******\n",
            serde_json::to_string(&rec1).unwrap()
        );
        let recs: Vec<Result<ParseRecord, String>> = parse_stream(
            Box::new(Cursor::new(sample.into_bytes())),
            SourceKind::Pif1Password,
        )
        .expect("parse")
        .collect();
        assert_eq!(recs.len(), 1);
        let rec = recs.into_iter().next().unwrap().expect("ok");
        assert_eq!(rec.item.title, "GitHub");
        assert_eq!(rec.item.url.as_deref(), Some("https://github.com"));
        let f = &rec.item.fields;
        assert_eq!(f.get("username").and_then(|v| v.as_str()), Some("octocat"));
        assert_eq!(
            f.get("password").and_then(|v| v.as_str()),
            Some(" hunter2 ")
        );
        assert_eq!(
            f.get("totp").and_then(|v| v.as_str()),
            Some("otpauth://totp/x?secret=ABC")
        );
        assert_eq!(
            f.get("notes").and_then(|v| v.as_str()),
            Some("work account")
        );
    }

    #[test]
    fn parses_csv_stream() {
        let data = "name,url,username,password\nBank,https://bank.example,u1,p1\nMail,https://mail.example,u2,p2\n";
        let iter = parse_stream(Box::new(Cursor::new(data)), SourceKind::Csv).unwrap();
        let recs: Vec<_> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].item.title, "Bank");
        assert_eq!(recs[0].item.url.as_deref(), Some("https://bank.example"));
        // First data row is file line 2 (line 1 is the header).
        assert_eq!(recs[0].line_number, 2);
        assert_eq!(recs[1].item.title, "Mail");
    }

    #[test]
    fn parses_bitwarden_json_stream() {
        let json = r#"{"encrypted":false,"items":[
            {"id":"a1","name":"GitHub","login":{"username":"u","password":"p","uris":[{"uri":"https://github.com"}]}},
            {"id":"a2","name":"Gmail","login":{"username":"x","password":"y"}}
        ]}"#;
        let iter = parse_stream(Box::new(Cursor::new(json)), SourceKind::BitwardenJson).unwrap();
        let recs: Vec<_> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].item.source_id.as_deref(), Some("a1"));
        assert_eq!(recs[0].item.title, "GitHub");
        assert_eq!(recs[0].item.url.as_deref(), Some("https://github.com"));
        // Second item has no uri -> url is None.
        assert_eq!(recs[1].item.url, None);
    }

    #[test]
    fn malformed_json_does_not_panic() {
        let json = "not json at all {{{";
        let iter = parse_stream(Box::new(Cursor::new(json)), SourceKind::BitwardenJson).unwrap();
        let recs: Vec<_> = iter.collect();
        // Empty or error records — never panics.
        let _ = recs;
    }
}
