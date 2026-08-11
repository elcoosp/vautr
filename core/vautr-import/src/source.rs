//! Cross-platform `ImportSource` abstraction (data-import-seeding.md §2.1).
//!
//! The streaming parser reads from a `Box<dyn BufRead>` so that WASM can later
//! provide a JS `File` handle while native code uses a filesystem path. ZIP
//! archives (`.1pux` / `.zip`) are extracted to a sandboxed temp directory and
//! the inner JSON is streamed from there, keeping O(1) peak memory.

use std::cell::RefCell;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::error::{ImportFailure, Result};

/// The source format, which selects the streaming parser in [`crate::parser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Plain-text browser export (Chrome / Safari / Firefox).
    Csv,
    /// Plain Bitwarden JSON export.
    BitwardenJson,
    /// 1Password `.1pux` (or generic `.zip`) archive containing a JSON export.
    Zip1pux,
}

impl SourceKind {
    /// Infer the source kind from a file extension. Unknown extensions default
    /// to CSV rather than failing so a malformed/hostile name never panics.
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match ext.as_str() {
            "csv" => SourceKind::Csv,
            "json" => SourceKind::BitwardenJson,
            "zip" | "1pux" | "1pif" => SourceKind::Zip1pux,
            _ => SourceKind::Csv,
        }
    }
}

/// A source of import records. Each call to [`ImportSource::reader`] returns a
/// fresh streaming handle over the inner CSV/JSON bytes.
pub trait ImportSource {
    /// The format this source contains.
    fn kind(&self) -> SourceKind;

    /// Open a fresh streaming reader over the inner bytes. For ZIP archives the
    /// implementation extracts the inner JSON to a sandboxed temp file first.
    fn reader(&self) -> Result<Box<dyn BufRead>>;
}

/// File-path backed [`ImportSource`] for native desktop / mobile.
///
/// Holds the extracted temp directory (when the source is a ZIP archive) for
/// the lifetime of the source so the extracted JSON remains readable.
pub struct PathImportSource {
    path: PathBuf,
    kind: SourceKind,
    temp_dir: RefCell<Option<tempfile::TempDir>>,
}

impl PathImportSource {
    /// Construct a source from a filesystem path. Does not open the file yet;
    /// kind inference only, so constructing a source never fails.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let kind = SourceKind::from_path(&path);
        Self {
            path,
            kind,
            temp_dir: RefCell::new(None),
        }
    }
}

impl ImportSource for PathImportSource {
    fn kind(&self) -> SourceKind {
        self.kind
    }

    fn reader(&self) -> Result<Box<dyn BufRead>> {
        match self.kind {
            SourceKind::Zip1pux => self.open_zip_reader(),
            _ => {
                let file = File::open(&self.path)
                    .map_err(|e| ImportFailure::Pipeline(format!("open {}: {e}", self.path.display())))?;
                Ok(Box::new(BufReader::new(file)))
            }
        }
    }
}

impl PathImportSource {
    /// Extract the first `.json` entry from the ZIP archive to a temp file and
    /// return a reader over it. The temp dir is retained in `self.temp_dir` so
    /// the extracted file stays alive as long as the source does.
    fn open_zip_reader(&self) -> Result<Box<dyn BufRead>> {
        let file = File::open(&self.path)
            .map_err(|e| ImportFailure::Pipeline(format!("open zip {}: {e}", self.path.display())))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| ImportFailure::Pipeline(format!("read zip archive: {e}")))?;

        let json_index = (0..archive.len()).find(|&i| {
            archive
                .by_index(i)
                .map(|f| f.name().to_ascii_lowercase().ends_with(".json"))
                .unwrap_or(false)
        });
        let index = match json_index {
            Some(i) => i,
            None => {
                return Err(ImportFailure::Pipeline(
                    "archive contains no .json entry".to_string(),
                ))
            }
        };

        let mut entry = archive
            .by_index(index)
            .map_err(|e| ImportFailure::Pipeline(format!("read archive entry: {e}")))?;

        let temp = tempfile::TempDir::new()
            .map_err(|e| ImportFailure::Pipeline(format!("temp dir: {e}")))?;
        let tmp_path = temp.path().join("archive_import.json");
        let mut out = File::create(&tmp_path)
            .map_err(|e| ImportFailure::Pipeline(format!("write temp file: {e}")))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| ImportFailure::Pipeline(format!("extract entry: {e}")))?;

        let extracted = File::open(&tmp_path)
            .map_err(|e| ImportFailure::Pipeline(format!("reopen temp file: {e}")))?;
        // Retain the temp dir so the extracted file outlives the reader.
        *self.temp_dir.borrow_mut() = Some(temp);
        Ok(Box::new(BufReader::new(extracted)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_kind_from_extension() {
        assert_eq!(SourceKind::from_path(Path::new("a.csv")), SourceKind::Csv);
        assert_eq!(
            SourceKind::from_path(Path::new("a.json")),
            SourceKind::BitwardenJson
        );
        assert_eq!(SourceKind::from_path(Path::new("a.1pux")), SourceKind::Zip1pux);
        // Unknown extensions fall back to CSV without panicking.
        assert_eq!(SourceKind::from_path(Path::new("a.data")), SourceKind::Csv);
    }
}
