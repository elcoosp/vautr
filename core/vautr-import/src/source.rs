//! Cross-platform `ImportSource` abstraction (data-import-seeding.md §2.1).
//!
//! The streaming parser reads from a `Box<dyn BufRead>` so that WASM can later
//! provide a JS `File` handle while native code uses a filesystem path. ZIP
//! archives (`.1pux` / `.zip`) are extracted to a sandboxed temp directory and
//! the inner JSON is streamed from there, keeping O(1) peak memory.

use std::io::BufRead;

use crate::error::Result;

#[cfg(feature = "pipeline")]
use std::cell::RefCell;
#[cfg(feature = "pipeline")]
use std::fs::File;
#[cfg(feature = "pipeline")]
use std::io::BufReader;
#[cfg(feature = "pipeline")]
use std::path::{Path, PathBuf};
#[cfg(feature = "pipeline")]
use crate::error::ImportFailure;

/// The source format, which selects the streaming parser in [`crate::parser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Plain-text browser export (Chrome / Safari / Firefox).
    Csv,
    /// Plain Bitwarden JSON export.
    BitwardenJson,
    /// 1Password `.1pux` (or generic `.zip`) archive containing a JSON export.
    Zip1pux,
    /// VTRFIX-BUG-M18: 1Password Interchange Format (`.1pif`) — line-delimited
    /// JSON records separated by `***...***` marker rows. NOT a ZIP archive.
    Pif1Password,
}

impl SourceKind {
    /// Infer the source kind from a filename. Unknown extensions default to
    /// CSV rather than failing so a malformed/hostile name never panics.
    ///
    /// VTRFIX-FEAT-H03: takes `&str` so this compiles on wasm32 (no `std::path`).
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".csv") {
            SourceKind::Csv
        } else if lower.ends_with(".json") {
            SourceKind::BitwardenJson
        } else if lower.ends_with(".zip") || lower.ends_with(".1pux") {
            SourceKind::Zip1pux
        } else if lower.ends_with(".1pif") {
            SourceKind::Pif1Password
        } else {
            SourceKind::Csv
        }
    }
}

#[cfg(feature = "pipeline")]
impl SourceKind {
    /// Infer the source kind from a file extension (native).
    pub fn from_path(path: &Path) -> Self {
        Self::from_name(&path.to_string_lossy())
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
#[cfg(feature = "pipeline")]
pub struct PathImportSource {
    path: PathBuf,
    kind: SourceKind,
    temp_dir: RefCell<Option<tempfile::TempDir>>,
}

#[cfg(feature = "pipeline")]
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

#[cfg(feature = "pipeline")]
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

#[cfg(feature = "pipeline")]
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
    fn infers_kind_from_name() {
        assert_eq!(SourceKind::from_name("a.csv"), SourceKind::Csv);
        assert_eq!(SourceKind::from_name("a.json"), SourceKind::BitwardenJson);
        assert_eq!(SourceKind::from_name("a.1pux"), SourceKind::Zip1pux);
        assert_eq!(SourceKind::from_name("a.1pif"), SourceKind::Pif1Password);
        // Unknown extensions fall back to CSV without panicking.
        assert_eq!(SourceKind::from_name("a.data"), SourceKind::Csv);
    }
}
