//! Diagnostics export with double-opt-in for raw data.
//!
//! Default export is metadata-only: only `summary.toml` paths and audit
//! envelopes are included. Raw bundle files (e.g. `panic.txt`) require BOTH
//! explicit `include_raw: true` **and** `consent.raw_source_allowed: true`.
//! If the flag is set but consent is absent, [`build`][DiagnosticsExportBuilder::build]
//! returns [`ExportError::RawNotAllowed`] — no silent degradation.

use std::path::PathBuf;

use legion_protocol::WorkbenchTelemetryConsent;
use thiserror::Error;

/// Error variants for diagnostics export.
#[derive(Debug, Error)]
pub enum ExportError {
    /// `include_raw` was requested but `consent.raw_source_allowed` is false.
    #[error("raw data export requires raw_source_allowed consent")]
    RawNotAllowed,
    /// A filesystem error occurred while scanning the bundle directory.
    #[error("export I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A builder for assembling a diagnostics export from crash bundles.
///
/// The default build is **metadata-only**: only `summary.toml` paths are
/// included; `raw_paths` in each entry is empty.
pub struct DiagnosticsExportBuilder {
    bundle_dir: PathBuf,
    include_raw: bool,
    consent: WorkbenchTelemetryConsent,
}

impl DiagnosticsExportBuilder {
    /// Create a builder from a crash-bundle directory and the current consent state.
    pub fn new(bundle_dir: PathBuf, consent: WorkbenchTelemetryConsent) -> Self {
        Self {
            bundle_dir,
            include_raw: false,
            consent,
        }
    }

    /// Request that raw bundle files (`panic.txt` etc.) be included.
    ///
    /// Granting this request requires `consent.raw_source_allowed == true`; if
    /// consent is absent [`build`][Self::build] returns [`ExportError::RawNotAllowed`].
    pub fn with_include_raw(mut self, include_raw: bool) -> Self {
        self.include_raw = include_raw;
        self
    }

    /// Build the diagnostics bundle.
    ///
    /// Scans `bundle_dir` for per-crash subdirectories and assembles a
    /// [`DiagnosticsBundle`].
    ///
    /// # Errors
    ///
    /// - [`ExportError::RawNotAllowed`] when `include_raw` is `true` but
    ///   `consent.raw_source_allowed` is `false`.
    /// - [`ExportError::IoError`] when the bundle directory cannot be read.
    pub fn build(self) -> Result<DiagnosticsBundle, ExportError> {
        // Double opt-in check: flag AND consent must both be set for raw data.
        if self.include_raw && !self.consent.raw_source_allowed {
            return Err(ExportError::RawNotAllowed);
        }

        let metadata_only = !self.include_raw;

        let mut entries = Vec::new();

        if self.bundle_dir.exists() {
            for entry in std::fs::read_dir(&self.bundle_dir)? {
                let entry = entry?;
                let crash_dir = entry.path();
                if !crash_dir.is_dir() {
                    continue;
                }

                let crash_id = crash_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_owned)
                    .unwrap_or_default();

                let summary_path = crash_dir.join("summary.toml");

                // Only include entries that have a summary.
                if !summary_path.exists() {
                    continue;
                }

                let raw_paths = if !metadata_only {
                    // Include all non-summary files as raw data.
                    let mut raw = Vec::new();
                    for f in std::fs::read_dir(&crash_dir)? {
                        let f = f?;
                        let p = f.path();
                        if p.is_file() && p != summary_path {
                            raw.push(p);
                        }
                    }
                    raw
                } else {
                    Vec::new()
                };

                entries.push(DiagnosticEntry {
                    crash_id,
                    summary_path,
                    raw_paths,
                });
            }
        }

        Ok(DiagnosticsBundle {
            entries,
            metadata_only,
        })
    }
}

/// An assembled diagnostics export.
#[derive(Debug)]
pub struct DiagnosticsBundle {
    /// Per-crash entries in the bundle.
    pub entries: Vec<DiagnosticEntry>,
    /// `true` when this bundle was built in metadata-only mode (no raw files).
    pub metadata_only: bool,
}

/// A single crash-report entry in a diagnostics bundle.
#[derive(Debug)]
pub struct DiagnosticEntry {
    /// Unique crash identifier (UUID string).
    pub crash_id: String,
    /// Path to the `summary.toml` file for this crash.
    pub summary_path: PathBuf,
    /// Paths to raw data files (e.g. `panic.txt`).
    ///
    /// **Empty when `metadata_only` is `true`.**
    pub raw_paths: Vec<PathBuf>,
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use legion_protocol::WorkbenchTelemetryConsent;
    use std::fs;
    use std::path::Path;

    fn consent_with_raw(raw: bool) -> WorkbenchTelemetryConsent {
        WorkbenchTelemetryConsent {
            enabled: true,
            crash_reports_enabled: true,
            raw_source_allowed: raw,
            consent_label: if raw {
                "raw-allowed".to_string()
            } else {
                "crash-reports".to_string()
            },
            schema_version: 1,
        }
    }

    fn make_bundle_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "legion_export_unit_{prefix}_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn populate_crash(bundle_dir: &Path) -> String {
        let crash_id = uuid::Uuid::new_v4().to_string();
        let crash_dir = bundle_dir.join(&crash_id);
        fs::create_dir_all(&crash_dir).unwrap();
        fs::write(
            crash_dir.join("summary.toml"),
            format!("crash_id = \"{crash_id}\"\n"),
        )
        .unwrap();
        fs::write(
            crash_dir.join("panic.txt"),
            "panic: test\n\nstack backtrace:\n",
        )
        .unwrap();
        crash_id
    }

    #[test]
    fn default_build_is_metadata_only() {
        let dir = make_bundle_dir("default");
        populate_crash(&dir);
        let bundle = DiagnosticsExportBuilder::new(dir, consent_with_raw(false))
            .build()
            .expect("build should succeed");
        assert!(bundle.metadata_only, "should be metadata_only");
        assert!(
            bundle.entries.iter().all(|e| e.raw_paths.is_empty()),
            "no raw paths in metadata-only bundle"
        );
    }

    #[test]
    fn raw_flag_without_consent_errors() {
        let dir = make_bundle_dir("raw_no_consent");
        let result = DiagnosticsExportBuilder::new(dir, consent_with_raw(false))
            .with_include_raw(true)
            .build();
        assert!(
            matches!(result, Err(ExportError::RawNotAllowed)),
            "should return RawNotAllowed, got: {result:?}"
        );
    }

    #[test]
    fn raw_allowed_when_flag_and_consent_set() {
        let dir = make_bundle_dir("raw_with_consent");
        populate_crash(&dir);
        let bundle = DiagnosticsExportBuilder::new(dir, consent_with_raw(true))
            .with_include_raw(true)
            .build()
            .expect("build should succeed with raw consent");
        assert!(!bundle.metadata_only, "should NOT be metadata_only");
        let has_raw = bundle.entries.iter().any(|e| !e.raw_paths.is_empty());
        assert!(has_raw, "should have at least one raw path");
    }
}
