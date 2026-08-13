//! Streaming, offline export to CSV or JSON (VTR-058).
//!
//! The writer consumes an iterator of [`ExportRow`] one at a time and flushes
//! each row to disk immediately, so peak memory is bounded by a single row
//! regardless of vault size (TDD #3: no OOM on 10k items).

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::ExportError;
use crate::format::ExportFormat;
use crate::report::ExportReport;
use crate::row::ExportRow;

/// Stream `rows` to `path` in the requested `format`.
///
/// - Writes one row at a time (bounded memory).
/// - Checks `cancel` between rows; if set, stops early and marks the report
///   `cancelled` (TDD #5).
/// - Invokes `progress(items_done, total)` after each row when `total` is known
///   (`0` means unknown/streaming).
pub fn export_rows(
    rows: impl IntoIterator<Item = ExportRow>,
    format: ExportFormat,
    path: impl AsRef<Path>,
    cancel: &AtomicBool,
    progress: impl Fn(u64, u64),
) -> Result<ExportReport, ExportError> {
    let path = path.as_ref();
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);

    let mut items_exported: u64 = 0;
    let mut cancelled = false;

    match format {
        ExportFormat::Csv => {
            let mut csv = csv::Writer::from_writer(&mut writer);
            // Header row.
            csv.write_record([
                "uuid", "title", "username", "password", "urls", "notes", "totp",
            ])?;
            for row in rows {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }
                write_csv_row(&mut csv, &row)?;
                items_exported += 1;
                progress(items_exported, 0);
            }
            csv.flush()?;
        }
        ExportFormat::Json => {
            // Streaming JSON array: open, write each object with separators,
            // close. Avoids building the whole array in memory.
            writer.write_all(b"[")?;
            let mut first = true;
            for row in rows {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }
                if !first {
                    writer.write_all(b",")?;
                }
                first = false;
                let json = serde_json::to_vec(&row)?;
                writer.write_all(&json)?;
                items_exported += 1;
                progress(items_exported, 0);
            }
            writer.write_all(b"]")?;
            writer.flush()?;
        }
    }

    // TDD #5: if cancelled, the partial file still exists but is reported as
    // incomplete. We do NOT delete it (the user may resume / inspect).
    let bytes_written = writer
        .into_inner()
        .map_err(|e| ExportError::Io(e.to_string()))?
        .metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ExportReport {
        items_exported,
        bytes_written,
        format,
        path: path.to_path_buf(),
        cancelled,
    })
}

fn write_csv_row(
    csv: &mut csv::Writer<&mut std::io::BufWriter<std::fs::File>>,
    row: &ExportRow,
) -> Result<(), ExportError> {
    let totp = row
        .totp
        .as_ref()
        .map(|t| {
            format!(
                "otpauth://totp/{}?secret={}",
                row.title,
                t.secret_base32.as_str()
            )
        })
        .unwrap_or_default();
    // csv::Writer auto-quotes/escapes fields with delimiters, quotes, or
    // newlines (RFC 4180), so pass raw values and let it handle quoting.
    let record = [
        row.uuid.to_string(),
        row.title.clone(),
        row.username.clone(),
        row.password.as_str().to_string(),
        row.urls.join(" | "),
        row.notes.as_str().to_string(),
        totp,
    ];
    csv.write_record(record)?;
    Ok(())
}

/// Convenience: build a shared cancel flag.
pub fn cancel_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Total items in an iterator without consuming it (for progress totals).
pub fn count_total(rows: &[ExportRow]) -> u64 {
    rows.len() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::row::TotpExport;
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use tempfile::NamedTempFile;
    use uuid::Uuid;
    use zeroize::Zeroizing;

    fn sample_row(i: u64) -> ExportRow {
        ExportRow {
            uuid: Uuid::new_v4(),
            title: format!("Site {i}"),
            username: format!("user{i}@example.com"),
            password: Zeroizing::new(format!("pass-{i}-secret")),
            urls: vec![
                format!("https://site{i}.example.com"),
                format!("https://m{i}.example.com"),
            ],
            notes: Zeroizing::new(format!("notes for {i}")),
            totp: Some(TotpExport {
                algorithm: "Sha1".into(),
                digits: 6,
                period: 30,
                secret_base32: Zeroizing::new("JBSWY3DPEHPK3PXP".into()),
            }),
        }
    }

    // TDD #1: 100 items -> CSV, then re-parse the CSV and assert fields match.
    #[test]
    fn tdd1_csv_roundtrip_preserves_all_fields() {
        let rows: Vec<ExportRow> = (0..100).map(sample_row).collect();
        let expected = rows.clone();

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let no_cancel = AtomicBool::new(false);
        let report =
            export_rows(rows, ExportFormat::Csv, path, &no_cancel, |_, _| {}).expect("export csv");
        assert_eq!(report.items_exported, 100);

        let mut buf = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
        let mut rdr = csv::Reader::from_reader(buf.as_bytes());
        let parsed: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(parsed.len(), 100, "100 data rows");
        for (i, rec) in parsed.iter().enumerate() {
            assert_eq!(rec.get(1).unwrap(), expected[i].title);
            assert_eq!(rec.get(2).unwrap(), expected[i].username);
            assert_eq!(rec.get(3).unwrap(), expected[i].password.as_str());
            assert_eq!(rec.get(4).unwrap(), expected[i].urls.join(" | "));
            assert_eq!(rec.get(5).unwrap(), expected[i].notes.as_str());
            assert!(rec.get(6).unwrap().contains("JBSWY3DPEHPK3PXP"));
        }
    }

    // TDD #2: JSON export validates the schema (uuid, title, username, password,
    // urls, notes, totp).
    #[test]
    fn tdd2_json_schema_has_required_fields() {
        let rows = vec![sample_row(1)];
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let no_cancel = AtomicBool::new(false);
        export_rows(rows, ExportFormat::Json, path, &no_cancel, |_, _| {}).unwrap();

        let mut buf = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&buf).unwrap();
        assert_eq!(parsed.len(), 1);
        let obj = &parsed[0];
        for key in [
            "uuid", "title", "username", "password", "urls", "notes", "totp",
        ] {
            assert!(obj.get(key).is_some(), "missing field {key}");
        }
        assert!(obj["urls"].is_array());
        assert!(obj["totp"].get("secret_base32").is_some());
    }

    // TDD #3: 10k items export via a streaming iterator (no holding all in a
    // Vec) and complete; the writer consumes one row at a time.
    #[test]
    fn tdd3_streaming_export_10k_items() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let no_cancel = AtomicBool::new(false);
        let report = export_rows(
            (0..10_000).map(sample_row),
            ExportFormat::Csv,
            path,
            &no_cancel,
            |_, _| {},
        )
        .expect("export 10k");
        assert_eq!(report.items_exported, 10_000);

        let mut buf = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
        let lines = buf.lines().count();
        assert_eq!(lines, 10_001, "header + 10k rows");
        assert!(report.bytes_written > 0);
    }

    // TDD #5: cancellation flag stops the export early and marks it cancelled.
    #[test]
    fn tdd5_cancel_flag_stops_export() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let cancel = AtomicBool::new(false);

        let progress = |done: u64, _total: u64| {
            if done >= 10 {
                cancel.store(true, Ordering::Relaxed);
            }
        };

        let report = export_rows(
            (0..10_000).map(sample_row),
            ExportFormat::Csv,
            path,
            &cancel,
            progress,
        )
        .expect("export with cancel");
        assert!(report.cancelled, "report marked cancelled");
        assert!(report.items_exported < 10_000, "stopped early");
        assert!(
            report.items_exported >= 10,
            "got at least up to the cancel point"
        );
    }

    // CSV cell escaping (RFC 4180): comma, quote, newline.
    #[test]
    fn csv_cell_escaping_handles_special_chars() {
        let row = ExportRow {
            uuid: Uuid::new_v4(),
            title: "He said \"hello\", world".into(),
            username: "a,b".into(),
            password: Zeroizing::new("p".into()),
            urls: vec![],
            notes: Zeroizing::new("line1\nline2".into()),
            totp: None,
        };
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let no_cancel = AtomicBool::new(false);
        export_rows(vec![row], ExportFormat::Csv, path, &no_cancel, |_, _| {}).unwrap();

        let mut buf = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();
        let mut rdr = csv::Reader::from_reader(buf.as_bytes());
        let rec = rdr.records().next().unwrap().unwrap();
        assert_eq!(rec.get(1).unwrap(), "He said \"hello\", world");
        assert_eq!(rec.get(2).unwrap(), "a,b");
        assert_eq!(rec.get(5).unwrap(), "line1\nline2");
    }
}
