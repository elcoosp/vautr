//! Export target format (VTR-058).

use std::str::FromStr;

/// The plaintext export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Comma-separated values, one vault item per row.
    Csv,
    /// A JSON array of item objects.
    Json,
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::Csv
    }
}

impl FromStr for ExportFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "csv" => Ok(ExportFormat::Csv),
            "json" => Ok(ExportFormat::Json),
            other => Err(format!("unknown export format: {other}")),
        }
    }
}
