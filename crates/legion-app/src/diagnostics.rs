//! App-side support-bundle surface.
//!
//! [`SupportBundleAssembler`] is the thin product-facing entry point that:
//! - Reads crash reports from the configured crash-bundle directory.
//! - Delegates export logic to [`DiagnosticsExportBuilder`][legion_observability::export::DiagnosticsExportBuilder].
//! - Returns metadata-only projections by default.
//!
//! This module does **not** contain capture logic; see
//! [`legion_observability::crash_capture`] for the panic-hook installation.

use std::path::PathBuf;

use legion_observability::export::{DiagnosticsBundle, DiagnosticsExportBuilder, ExportError};
use legion_protocol::WorkbenchTelemetryConsent;

/// Metadata row returned by [`SupportBundleAssembler::list_crash_reports`].
#[derive(Debug, Clone)]
pub struct CrashReportRow {
    /// Unique crash identifier.
    pub crash_id: String,
    /// ISO 8601 timestamp string, or `"unknown"` if the field was missing.
    pub timestamp: String,
    /// Operating system from `summary.toml`, or `"unknown"`.
    pub os: String,
}

/// Thin app-layer wrapper around the diagnostics export builder.
///
/// Keeps the crash-bundle directory and consent state. All export work
/// delegates to `legion-observability` — this crate only handles path
/// resolution and TOML parsing.
pub struct SupportBundleAssembler {
    bundle_dir: PathBuf,
    consent: WorkbenchTelemetryConsent,
}

impl SupportBundleAssembler {
    /// Create an assembler pointed at `bundle_dir` with the given consent state.
    pub fn new(bundle_dir: PathBuf, consent: WorkbenchTelemetryConsent) -> Self {
        Self {
            bundle_dir,
            consent,
        }
    }

    /// Return a metadata-only list of available crash reports.
    ///
    /// Reads each `summary.toml` and extracts `crash_id`, `timestamp`, and `os`.
    /// Does NOT return raw file contents.
    pub fn list_crash_reports(&self) -> Vec<CrashReportRow> {
        let bundle = match DiagnosticsExportBuilder::new(
            self.bundle_dir.clone(),
            self.consent.clone(),
        )
        .build()
        {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };

        bundle
            .entries
            .into_iter()
            .map(|entry| {
                let (timestamp, os) = parse_summary_toml(&entry.summary_path);
                CrashReportRow {
                    crash_id: entry.crash_id,
                    timestamp,
                    os,
                }
            })
            .collect()
    }

    /// Build a metadata-only diagnostics bundle (no raw data).
    pub fn build_metadata_bundle(&self) -> Result<DiagnosticsBundle, ExportError> {
        DiagnosticsExportBuilder::new(self.bundle_dir.clone(), self.consent.clone()).build()
    }

    /// Build a bundle including raw data.
    ///
    /// Requires BOTH `consent.raw_source_allowed == true` AND `include_raw == true`.
    /// Returns [`ExportError::RawNotAllowed`] if consent is absent.
    pub fn build_raw_bundle(&self) -> Result<DiagnosticsBundle, ExportError> {
        DiagnosticsExportBuilder::new(self.bundle_dir.clone(), self.consent.clone())
            .with_include_raw(true)
            .build()
    }
}

/// Parse a `summary.toml` file and extract `timestamp` and `os` fields.
///
/// The TOML is written by the panic hook as simple `key = "value"` pairs.
/// This parser handles that minimal subset without a full TOML dependency.
fn parse_summary_toml(path: &std::path::Path) -> (String, String) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ("unknown".to_string(), "unknown".to_string()),
    };

    let mut timestamp = "unknown".to_string();
    let mut os = "unknown".to_string();

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = extract_toml_value(line, "timestamp") {
            timestamp = val;
        } else if let Some(val) = extract_toml_value(line, "os") {
            os = val;
        }
    }

    (timestamp, os)
}

/// Extract the value for a `key = "value"` or `key = integer` TOML line.
fn extract_toml_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    if !line.starts_with(&prefix) {
        return None;
    }
    let raw = &line[prefix.len()..];
    // Strip surrounding quotes if present.
    if raw.starts_with('"') && raw.ends_with('"') {
        Some(raw[1..raw.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\"))
    } else {
        Some(raw.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use legion_protocol::WorkbenchTelemetryConsent;

    fn disabled_consent() -> WorkbenchTelemetryConsent {
        WorkbenchTelemetryConsent {
            enabled: false,
            crash_reports_enabled: false,
            raw_source_allowed: false,
            consent_label: "local-only".to_string(),
            schema_version: 1,
        }
    }

    fn crash_consent() -> WorkbenchTelemetryConsent {
        WorkbenchTelemetryConsent {
            enabled: true,
            crash_reports_enabled: true,
            raw_source_allowed: false,
            consent_label: "crash-reports".to_string(),
            schema_version: 1,
        }
    }

    fn make_bundle_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "legion_diagnostics_test_{label}_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4(),
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_crash(bundle_dir: &PathBuf, crash_id: &str, timestamp: u64, os: &str) {
        let crash_dir = bundle_dir.join(crash_id);
        fs::create_dir_all(&crash_dir).unwrap();
        let summary = format!(
            "crash_id = \"{crash_id}\"\ntimestamp = {timestamp}\nversion = \"0.1.0\"\nos = \"{os}\"\narch = \"x86_64\"\npanic_message = \"test\"\npanic_location = \"foo/src/lib.rs:1\"\nsigner_status = \"unsigned-beta\"\n"
        );
        fs::write(crash_dir.join("summary.toml"), summary).unwrap();
    }

    #[test]
    fn list_crash_reports_returns_metadata_rows() {
        let dir = make_bundle_dir("list");
        write_crash(&dir, "crash-aaa", 1_700_000_000, "windows");

        let assembler = SupportBundleAssembler::new(dir, crash_consent());
        let rows = assembler.list_crash_reports();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].crash_id, "crash-aaa");
        assert_eq!(rows[0].timestamp, "1700000000");
        assert_eq!(rows[0].os, "windows");
    }

    #[test]
    fn empty_dir_returns_no_rows() {
        let dir = make_bundle_dir("empty");
        let assembler = SupportBundleAssembler::new(dir, disabled_consent());
        assert!(assembler.list_crash_reports().is_empty());
    }

    #[test]
    fn metadata_bundle_has_no_raw_paths() {
        let dir = make_bundle_dir("meta_bundle");
        write_crash(&dir, "crash-bbb", 1_700_000_001, "linux");

        let assembler = SupportBundleAssembler::new(dir, crash_consent());
        let bundle = assembler.build_metadata_bundle().expect("build should succeed");

        assert!(bundle.metadata_only);
        assert!(bundle.entries.iter().all(|e| e.raw_paths.is_empty()));
    }

    #[test]
    fn raw_bundle_without_consent_returns_error() {
        let dir = make_bundle_dir("raw_denied");
        write_crash(&dir, "crash-ccc", 1_700_000_002, "macos");

        let assembler = SupportBundleAssembler::new(dir, crash_consent()); // raw_source_allowed = false
        let result = assembler.build_raw_bundle();

        assert!(result.is_err(), "should error without raw consent");
    }
}
