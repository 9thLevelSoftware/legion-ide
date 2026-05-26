//! Deterministic Phase 8 hosted telemetry fixture spool and exporter.

#![warn(missing_docs)]

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use devil_protocol::{
    HostedTelemetryConsentGrant, HostedTelemetryEndpointDescriptor, HostedTelemetryExportBatch,
    HostedTelemetrySpoolRecord, HostedTelemetryUploadOutcome, WorkspaceId,
    validate_hosted_telemetry_export_batch, validate_hosted_telemetry_spool_record,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Telemetry fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TelemetryFixtureError {
    /// Local telemetry spool is disabled.
    #[error("telemetry spool is disabled")]
    SpoolDisabled,
    /// Hosted export is disabled.
    #[error("hosted telemetry export is disabled")]
    ExportDisabled,
    /// Telemetry metadata validation failed.
    #[error("invalid telemetry metadata: {reason}")]
    InvalidMetadata {
        /// Validation reason.
        reason: String,
    },
    /// Local spool is full.
    #[error("telemetry spool is full")]
    SpoolFull,
}

/// Durable telemetry spool error.
#[derive(Debug, Error)]
pub enum TelemetrySpoolError {
    /// Spool is disabled.
    #[error("telemetry spool is disabled")]
    Disabled,
    /// Spool metadata was invalid.
    #[error("invalid telemetry metadata: {reason}")]
    InvalidMetadata {
        /// Validation reason.
        reason: String,
    },
    /// Spool has reached its configured capacity.
    #[error("telemetry spool is full")]
    Full,
    /// Filesystem operation failed.
    #[error("telemetry spool I/O failed: {message}")]
    Io {
        /// Failure details.
        message: String,
    },
    /// Hosted HTTP export failed.
    #[error("hosted telemetry HTTP export failed: {message}")]
    Http {
        /// Failure details.
        message: String,
    },
}

/// Durable telemetry spool configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetrySpoolConfig {
    /// Whether durable spooling is enabled.
    pub enabled: bool,
    /// Maximum records retained on disk.
    pub max_records: usize,
    /// Maximum records per hosted export batch.
    pub max_batch_records: usize,
}

impl TelemetrySpoolConfig {
    /// Return an enabled durable spool configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TelemetrySpoolConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_records: 1024,
            max_batch_records: 64,
        }
    }
}

/// Durable spool statistics safe for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetrySpoolStats {
    /// Pending record count.
    pub pending_records: usize,
    /// Dropped record count.
    pub dropped_records: u64,
    /// Last retry-after hint in milliseconds.
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedTelemetrySpool {
    schema_version: u16,
    records: VecDeque<HostedTelemetrySpoolRecord>,
    dropped_records: u64,
    retry_after_ms: Option<u64>,
}

/// Telemetry fixture configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryFixtureConfig {
    /// Whether local deterministic spool behavior is enabled.
    pub spool_enabled: bool,
    /// Whether deterministic hosted-export acknowledgement behavior is enabled.
    pub export_enabled: bool,
    /// Maximum in-memory spool records.
    pub max_records: usize,
}

impl TelemetryFixtureConfig {
    /// Return an enabled local-only deterministic fixture configuration.
    pub fn local_enabled() -> Self {
        Self {
            spool_enabled: true,
            ..Self::default()
        }
    }

    /// Return an enabled deterministic fixture configuration including fake export acknowledgements.
    pub fn export_enabled() -> Self {
        Self {
            spool_enabled: true,
            export_enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TelemetryFixtureConfig {
    fn default() -> Self {
        Self {
            spool_enabled: false,
            export_enabled: false,
            max_records: 128,
        }
    }
}

/// Deterministic metadata-only telemetry spool.
#[derive(Debug, Clone)]
pub struct TelemetryFixtureSpool {
    config: TelemetryFixtureConfig,
    records: VecDeque<HostedTelemetrySpoolRecord>,
}

impl TelemetryFixtureSpool {
    /// Construct a spool from configuration.
    pub fn new(config: TelemetryFixtureConfig) -> Self {
        Self {
            config,
            records: VecDeque::new(),
        }
    }

    /// Add one metadata-only record to the local deterministic spool.
    pub fn enqueue(
        &mut self,
        record: HostedTelemetrySpoolRecord,
    ) -> Result<(), TelemetryFixtureError> {
        if !self.config.spool_enabled {
            return Err(TelemetryFixtureError::SpoolDisabled);
        }
        validate_hosted_telemetry_spool_record(&record).map_err(|err| {
            TelemetryFixtureError::InvalidMetadata {
                reason: err.message,
            }
        })?;
        if self.records.len() >= self.config.max_records {
            return Err(TelemetryFixtureError::SpoolFull);
        }
        self.records.push_back(record);
        Ok(())
    }

    /// Return the current spool length.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return true when no records are queued.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Deterministically acknowledge one hosted telemetry batch without performing network I/O.
    pub fn acknowledge_batch(
        &self,
        batch: HostedTelemetryExportBatch,
    ) -> Result<HostedTelemetryUploadOutcome, TelemetryFixtureError> {
        if !self.config.export_enabled {
            return Err(TelemetryFixtureError::ExportDisabled);
        }
        validate_hosted_telemetry_export_batch(&batch).map_err(|err| {
            TelemetryFixtureError::InvalidMetadata {
                reason: err.message,
            }
        })?;
        Ok(HostedTelemetryUploadOutcome {
            batch_id: batch.batch_id,
            accepted: true,
            retry_after_ms: None,
            status: "accepted-by-fixture".to_string(),
            schema_version: 1,
        })
    }

    /// Purge all queued records for consent revocation handling.
    pub fn purge(&mut self) -> usize {
        let count = self.records.len();
        self.records.clear();
        count
    }
}

/// Hosted telemetry export client abstraction.
pub trait TelemetryExportClient {
    /// Upload one metadata-only batch and return endpoint acknowledgement metadata.
    fn upload(
        &mut self,
        batch: &HostedTelemetryExportBatch,
    ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError>;
}

/// Rustls-only hosted HTTP telemetry exporter configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReqwestTelemetryExportConfig {
    /// Whether hosted HTTP export is enabled.
    pub enabled: bool,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum serialized body size accepted before upload.
    pub max_body_bytes: usize,
}

impl ReqwestTelemetryExportConfig {
    /// Return an enabled hosted HTTP telemetry exporter configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for ReqwestTelemetryExportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: 5_000,
            max_body_bytes: 64 * 1024,
        }
    }
}

/// Hosted HTTP telemetry exporter using a rustls-only reqwest client.
pub struct ReqwestTelemetryExportClient {
    config: ReqwestTelemetryExportConfig,
    client: reqwest::blocking::Client,
}

impl ReqwestTelemetryExportClient {
    /// Construct a rustls-only HTTP exporter.
    pub fn new(config: ReqwestTelemetryExportConfig) -> Result<Self, TelemetrySpoolError> {
        ensure_rustls_crypto_provider()?;
        let client = reqwest::blocking::Client::builder()
            .tls_backend_rustls()
            .https_only(true)
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|err| TelemetrySpoolError::Http {
                message: format!("build reqwest rustls client: {err}"),
            })?;
        Ok(Self { config, client })
    }
}

impl TelemetryExportClient for ReqwestTelemetryExportClient {
    fn upload(
        &mut self,
        batch: &HostedTelemetryExportBatch,
    ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError> {
        if !self.config.enabled {
            return Err(TelemetrySpoolError::Disabled);
        }
        validate_hosted_telemetry_export_batch(batch).map_err(|err| {
            TelemetrySpoolError::InvalidMetadata {
                reason: err.message,
            }
        })?;
        let body = serde_json::to_vec(batch).map_err(|err| TelemetrySpoolError::Http {
            message: format!("encode hosted telemetry batch: {err}"),
        })?;
        if body.len() > self.config.max_body_bytes {
            return Err(TelemetrySpoolError::InvalidMetadata {
                reason: "hosted telemetry batch exceeds configured body limit".to_string(),
            });
        }
        let response = self
            .client
            .post(&batch.endpoint.endpoint_label)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .map_err(|err| TelemetrySpoolError::Http {
                message: format!("send hosted telemetry batch: {err}"),
            })?;
        let status = response.status();
        if status.is_success() {
            let outcome = response
                .json::<HostedTelemetryUploadOutcome>()
                .map_err(|err| TelemetrySpoolError::Http {
                    message: format!(
                        "parse hosted telemetry ack for HTTP {}: {err}",
                        status.as_u16()
                    ),
                })?;
            if outcome.batch_id != batch.batch_id {
                return Err(TelemetrySpoolError::InvalidMetadata {
                    reason: "hosted telemetry ack batch id did not match upload".to_string(),
                });
            }
            Ok(outcome)
        } else {
            Ok(HostedTelemetryUploadOutcome {
                batch_id: batch.batch_id.clone(),
                accepted: false,
                retry_after_ms: response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|seconds| seconds.saturating_mul(1_000)),
                status: format!("http_{}", status.as_u16()),
                schema_version: 1,
            })
        }
    }
}

fn ensure_rustls_crypto_provider() -> Result<(), TelemetrySpoolError> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(()) => Ok(()),
        Err(_) if rustls::crypto::CryptoProvider::get_default().is_some() => Ok(()),
        Err(_) => Err(TelemetrySpoolError::Http {
            message: "install rustls crypto provider".to_string(),
        }),
    }
}

/// File-backed durable metadata-only telemetry spool.
#[derive(Debug)]
pub struct FileBackedTelemetrySpool {
    path: PathBuf,
    config: TelemetrySpoolConfig,
    state: PersistedTelemetrySpool,
}

impl FileBackedTelemetrySpool {
    /// Open a durable telemetry spool at `path`.
    pub fn open(
        path: impl AsRef<Path>,
        config: TelemetrySpoolConfig,
    ) -> Result<Self, TelemetrySpoolError> {
        let path = path.as_ref().to_path_buf();
        let state = if path.exists() {
            let text = fs::read_to_string(&path).map_err(io_error)?;
            serde_json::from_str(&text).map_err(|err| TelemetrySpoolError::Io {
                message: format!("decode durable spool: {err}"),
            })?
        } else {
            PersistedTelemetrySpool {
                schema_version: 1,
                ..PersistedTelemetrySpool::default()
            }
        };
        Ok(Self {
            path,
            config,
            state,
        })
    }

    /// Add one metadata-only record to the durable spool.
    pub fn enqueue(
        &mut self,
        record: HostedTelemetrySpoolRecord,
    ) -> Result<(), TelemetrySpoolError> {
        if !self.config.enabled {
            return Err(TelemetrySpoolError::Disabled);
        }
        validate_hosted_telemetry_spool_record(&record).map_err(|err| {
            TelemetrySpoolError::InvalidMetadata {
                reason: err.message,
            }
        })?;
        if self.state.records.len() >= self.config.max_records {
            self.state.dropped_records = self.state.dropped_records.saturating_add(1);
            self.flush()?;
            return Err(TelemetrySpoolError::Full);
        }
        self.state.records.push_back(record);
        self.flush()
    }

    /// Build a pending export batch without removing records.
    pub fn pending_batch(
        &self,
        consent: HostedTelemetryConsentGrant,
        endpoint: HostedTelemetryEndpointDescriptor,
        max_records: usize,
    ) -> Result<HostedTelemetryExportBatch, TelemetrySpoolError> {
        let max_records = max_records
            .min(self.config.max_batch_records)
            .min(self.state.records.len());
        let records = self
            .state
            .records
            .iter()
            .filter(|record| record.workspace_id == consent.workspace_id)
            .take(max_records)
            .cloned()
            .collect::<Vec<_>>();
        let batch = HostedTelemetryExportBatch {
            batch_id: telemetry_batch_id(consent.workspace_id, &records),
            endpoint,
            consent,
            records,
            schema_version: 1,
        };
        validate_hosted_telemetry_export_batch(&batch).map_err(|err| {
            TelemetrySpoolError::InvalidMetadata {
                reason: err.message,
            }
        })?;
        Ok(batch)
    }

    /// Remove accepted records from the durable spool.
    pub fn mark_uploaded(
        &mut self,
        accepted_record_ids: &[String],
    ) -> Result<usize, TelemetrySpoolError> {
        let pending_ids = self
            .state
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<HashSet<_>>();
        if accepted_record_ids
            .iter()
            .any(|record_id| !pending_ids.contains(record_id.as_str()))
        {
            return Err(TelemetrySpoolError::InvalidMetadata {
                reason: "hosted telemetry ack contained record id outside pending spool"
                    .to_string(),
            });
        }
        let before = self.state.records.len();
        self.state
            .records
            .retain(|record| !accepted_record_ids.contains(&record.record_id));
        let removed = before.saturating_sub(self.state.records.len());
        self.state.retry_after_ms = None;
        self.flush()?;
        Ok(removed)
    }

    /// Record retry metadata without dropping records.
    pub fn mark_retry(&mut self, retry_after_ms: Option<u64>) -> Result<(), TelemetrySpoolError> {
        self.state.retry_after_ms = retry_after_ms;
        self.flush()
    }

    /// Purge all records for a workspace after consent revocation.
    pub fn purge_for_workspace(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<usize, TelemetrySpoolError> {
        let before = self.state.records.len();
        self.state
            .records
            .retain(|record| record.workspace_id != workspace_id);
        let removed = before.saturating_sub(self.state.records.len());
        self.flush()?;
        Ok(removed)
    }

    /// Return metadata-only durable spool statistics.
    pub fn stats(&self) -> TelemetrySpoolStats {
        TelemetrySpoolStats {
            pending_records: self.state.records.len(),
            dropped_records: self.state.dropped_records,
            retry_after_ms: self.state.retry_after_ms,
        }
    }

    fn flush(&self) -> Result<(), TelemetrySpoolError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let text =
            serde_json::to_string_pretty(&self.state).map_err(|err| TelemetrySpoolError::Io {
                message: format!("encode durable spool: {err}"),
            })?;
        write_file_atomically(&self.path, text.as_bytes()).map_err(io_error)
    }
}

/// Hosted telemetry exporter that consumes a durable metadata spool.
pub struct HostedTelemetryExporter<C> {
    client: C,
}

impl<C: TelemetryExportClient> HostedTelemetryExporter<C> {
    /// Construct an exporter from an upload client.
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Export one batch and update durable spool state.
    pub fn export_once(
        &mut self,
        spool: &mut FileBackedTelemetrySpool,
        consent: HostedTelemetryConsentGrant,
        endpoint: HostedTelemetryEndpointDescriptor,
        max_records: usize,
    ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError> {
        let batch = spool.pending_batch(consent, endpoint, max_records)?;
        let outcome = self.client.upload(&batch)?;
        if outcome.batch_id != batch.batch_id {
            return Err(TelemetrySpoolError::InvalidMetadata {
                reason: "hosted telemetry ack batch id did not match pending batch".to_string(),
            });
        }
        if outcome.accepted {
            let accepted = batch
                .records
                .iter()
                .map(|record| record.record_id.clone())
                .collect::<Vec<_>>();
            spool.mark_uploaded(&accepted)?;
        } else {
            spool.mark_retry(outcome.retry_after_ms)?;
        }
        Ok(outcome)
    }
}

fn telemetry_batch_id(workspace_id: WorkspaceId, records: &[HostedTelemetrySpoolRecord]) -> String {
    let first = records
        .first()
        .map(|record| record.record_id.as_str())
        .unwrap_or("empty");
    let last = records
        .last()
        .map(|record| record.record_id.as_str())
        .unwrap_or(first);
    let first_sequence = records
        .first()
        .map(|record| record.event_sequence.0)
        .unwrap_or(0);
    let last_sequence = records
        .last()
        .map(|record| record.event_sequence.0)
        .unwrap_or(first_sequence);
    format!(
        "batch:{}:{}:{}:{}:{}:{}",
        workspace_id.0,
        records.len(),
        first_sequence,
        last_sequence,
        safe_batch_component(first),
        safe_batch_component(last)
    )
}

fn safe_batch_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("telemetry-spool");
    parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
}

fn write_file_atomically(path: &Path, body: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temp = atomic_temp_path(path);
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(body)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        atomic_replace(&temp, path)?;
        sync_parent_directory_when_supported(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let ok = unsafe {
        MoveFileExW(
            wide(temp).as_ptr(),
            wide(target).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(unix)]
fn sync_parent_directory_when_supported(parent: &Path) -> std::io::Result<()> {
    OpenOptions::new().read(true).open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory_when_supported(_parent: &Path) -> std::io::Result<()> {
    Ok(())
}

fn io_error(err: std::io::Error) -> TelemetrySpoolError {
    TelemetrySpoolError::Io {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use devil_protocol::{
        CausalityId, CorrelationId, EventSequence, HostedTelemetryCategory,
        HostedTelemetryConsentGrant, HostedTelemetryEndpointDescriptor, PrincipalId,
        PrivacyClassification, RedactionHint, WorkspaceId,
    };

    use super::*;

    fn record() -> HostedTelemetrySpoolRecord {
        HostedTelemetrySpoolRecord {
            record_id: "spool-1".to_string(),
            workspace_id: WorkspaceId(1),
            category: HostedTelemetryCategory::Diagnostics,
            classification: PrivacyClassification::Metadata,
            metadata_summary: "event_count=1".to_string(),
            event_sequence: EventSequence(1),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_2000_0000_0001,
            )),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn record_with_id(id: &str) -> HostedTelemetrySpoolRecord {
        HostedTelemetrySpoolRecord {
            record_id: id.to_string(),
            ..record()
        }
    }

    fn endpoint() -> HostedTelemetryEndpointDescriptor {
        HostedTelemetryEndpointDescriptor {
            endpoint_id: "test".to_string(),
            endpoint_label: "https://telemetry.invalid".to_string(),
            region: "local".to_string(),
            allowlisted: true,
            schema_version: 1,
        }
    }

    fn consent() -> HostedTelemetryConsentGrant {
        HostedTelemetryConsentGrant {
            principal_id: PrincipalId("tester".to_string()),
            workspace_id: WorkspaceId(1),
            categories: vec![HostedTelemetryCategory::Diagnostics],
            endpoint: endpoint(),
            expires_at: None,
            correlation_id: CorrelationId(1),
            schema_version: 1,
        }
    }

    fn temp_spool_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "devil-telemetry-{name}-{}.json",
            uuid::Uuid::now_v7()
        ))
    }

    struct AcceptingClient;

    impl TelemetryExportClient for AcceptingClient {
        fn upload(
            &mut self,
            batch: &HostedTelemetryExportBatch,
        ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError> {
            Ok(HostedTelemetryUploadOutcome {
                batch_id: batch.batch_id.clone(),
                accepted: true,
                retry_after_ms: None,
                status: "accepted".to_string(),
                schema_version: 1,
            })
        }
    }

    struct RetryClient;

    impl TelemetryExportClient for RetryClient {
        fn upload(
            &mut self,
            batch: &HostedTelemetryExportBatch,
        ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError> {
            Ok(HostedTelemetryUploadOutcome {
                batch_id: batch.batch_id.clone(),
                accepted: false,
                retry_after_ms: Some(1_000),
                status: "retry".to_string(),
                schema_version: 1,
            })
        }
    }

    struct MismatchedAckClient;

    impl TelemetryExportClient for MismatchedAckClient {
        fn upload(
            &mut self,
            _batch: &HostedTelemetryExportBatch,
        ) -> Result<HostedTelemetryUploadOutcome, TelemetrySpoolError> {
            Ok(HostedTelemetryUploadOutcome {
                batch_id: "different-batch".to_string(),
                accepted: true,
                retry_after_ms: None,
                status: "accepted".to_string(),
                schema_version: 1,
            })
        }
    }

    fn batch() -> HostedTelemetryExportBatch {
        let endpoint = HostedTelemetryEndpointDescriptor {
            endpoint_id: "test".to_string(),
            endpoint_label: "https://telemetry.invalid".to_string(),
            region: "local".to_string(),
            allowlisted: true,
            schema_version: 1,
        };
        HostedTelemetryExportBatch {
            batch_id: "batch-1".to_string(),
            endpoint: endpoint.clone(),
            consent: HostedTelemetryConsentGrant {
                principal_id: PrincipalId("tester".to_string()),
                workspace_id: WorkspaceId(1),
                categories: vec![HostedTelemetryCategory::Diagnostics],
                endpoint,
                expires_at: None,
                correlation_id: CorrelationId(1),
                schema_version: 1,
            },
            records: vec![record()],
            schema_version: 1,
        }
    }

    #[test]
    fn telemetry_spool_is_default_off() {
        let mut spool = TelemetryFixtureSpool::new(TelemetryFixtureConfig::default());
        assert!(matches!(
            spool.enqueue(record()),
            Err(TelemetryFixtureError::SpoolDisabled)
        ));
    }

    #[test]
    fn telemetry_spool_accepts_metadata_and_purges_on_revoke() {
        let mut spool = TelemetryFixtureSpool::new(TelemetryFixtureConfig::local_enabled());
        spool.enqueue(record()).expect("enqueue");
        assert_eq!(spool.len(), 1);
        assert_eq!(spool.purge(), 1);
        assert!(spool.is_empty());
    }

    #[test]
    fn telemetry_export_requires_explicit_fixture_enablement() {
        let spool = TelemetryFixtureSpool::new(TelemetryFixtureConfig::local_enabled());
        assert!(matches!(
            spool.acknowledge_batch(batch()),
            Err(TelemetryFixtureError::ExportDisabled)
        ));

        let spool = TelemetryFixtureSpool::new(TelemetryFixtureConfig::export_enabled());
        let outcome = spool.acknowledge_batch(batch()).expect("ack");
        assert!(outcome.accepted);
    }

    #[test]
    fn telemetry_spool_rejects_raw_content_classification() {
        let mut spool = TelemetryFixtureSpool::new(TelemetryFixtureConfig::local_enabled());
        let invalid = HostedTelemetrySpoolRecord {
            classification: PrivacyClassification::RawContent,
            metadata_summary: "raw_source=fn main".to_string(),
            ..record()
        };
        assert!(matches!(
            spool.enqueue(invalid),
            Err(TelemetryFixtureError::InvalidMetadata { .. })
        ));
    }

    #[test]
    fn file_backed_spool_survives_reopen_and_purges_on_revoke() {
        let path = temp_spool_path("survives-reopen");
        {
            let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
                .expect("open spool");
            spool.enqueue(record()).expect("enqueue");
            assert_eq!(spool.stats().pending_records, 1);
        }
        let mut reopened = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("reopen spool");
        assert_eq!(reopened.stats().pending_records, 1);
        assert_eq!(
            reopened
                .purge_for_workspace(WorkspaceId(1))
                .expect("purge workspace"),
            1
        );
        assert_eq!(reopened.stats().pending_records, 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_backed_spool_rejects_raw_or_sensitive_records() {
        let path = temp_spool_path("rejects-raw");
        let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("open spool");
        let invalid = HostedTelemetrySpoolRecord {
            classification: PrivacyClassification::RawContent,
            metadata_summary: "raw_source=fn main".to_string(),
            ..record()
        };
        assert!(matches!(
            spool.enqueue(invalid),
            Err(TelemetrySpoolError::InvalidMetadata { .. })
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn exporter_marks_accepted_records_and_keeps_retry_records() {
        let path = temp_spool_path("exporter");
        let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("open spool");
        spool.enqueue(record_with_id("record-1")).expect("enqueue");
        let mut retry = HostedTelemetryExporter::new(RetryClient);
        let outcome = retry
            .export_once(&mut spool, consent(), endpoint(), 1)
            .expect("retry export");
        assert!(!outcome.accepted);
        assert_eq!(spool.stats().pending_records, 1);
        assert_eq!(spool.stats().retry_after_ms, Some(1_000));

        let mut accepted = HostedTelemetryExporter::new(AcceptingClient);
        let outcome = accepted
            .export_once(&mut spool, consent(), endpoint(), 1)
            .expect("accepted export");
        assert!(outcome.accepted);
        assert_eq!(spool.stats().pending_records, 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn pending_batch_ids_change_after_partial_upload() {
        let path = temp_spool_path("batch-ids");
        let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("open spool");
        spool
            .enqueue(record_with_id("record-1"))
            .expect("enqueue 1");
        let mut second = record_with_id("record-2");
        second.event_sequence = EventSequence(2);
        spool.enqueue(second).expect("enqueue 2");

        let first_batch = spool
            .pending_batch(consent(), endpoint(), 1)
            .expect("first batch");
        spool
            .mark_uploaded(&["record-1".to_string()])
            .expect("mark first uploaded");
        let second_batch = spool
            .pending_batch(consent(), endpoint(), 1)
            .expect("second batch");

        assert_ne!(first_batch.batch_id, second_batch.batch_id);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_backed_spool_rejects_unknown_uploaded_record_ids() {
        let path = temp_spool_path("unknown-ack");
        let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("open spool");
        spool.enqueue(record_with_id("record-1")).expect("enqueue");
        assert!(matches!(
            spool.mark_uploaded(&["record-missing".to_string()]),
            Err(TelemetrySpoolError::InvalidMetadata { .. })
        ));
        assert_eq!(spool.stats().pending_records, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn exporter_rejects_mismatched_batch_ack_without_dropping_records() {
        let path = temp_spool_path("mismatched-ack");
        let mut spool = FileBackedTelemetrySpool::open(&path, TelemetrySpoolConfig::enabled())
            .expect("open spool");
        spool.enqueue(record_with_id("record-1")).expect("enqueue");
        let mut exporter = HostedTelemetryExporter::new(MismatchedAckClient);
        assert!(matches!(
            exporter.export_once(&mut spool, consent(), endpoint(), 1),
            Err(TelemetrySpoolError::InvalidMetadata { .. })
        ));
        assert_eq!(spool.stats().pending_records, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reqwest_exporter_is_default_off_and_rustls_configured() {
        let mut client = ReqwestTelemetryExportClient::new(ReqwestTelemetryExportConfig::default())
            .expect("build rustls reqwest client");
        assert!(matches!(
            client.upload(&batch()),
            Err(TelemetrySpoolError::Disabled)
        ));
    }

    #[test]
    fn reqwest_exporter_rejects_oversized_batches_before_http() {
        let mut client = ReqwestTelemetryExportClient::new(ReqwestTelemetryExportConfig {
            enabled: true,
            timeout_ms: 100,
            max_body_bytes: 8,
        })
        .expect("build rustls reqwest client");
        assert!(matches!(
            client.upload(&batch()),
            Err(TelemetrySpoolError::InvalidMetadata { .. })
        ));
    }

    #[test]
    fn spool_enforces_max_records_and_drop_summary() {
        let path = temp_spool_path("full");
        let mut spool = FileBackedTelemetrySpool::open(
            &path,
            TelemetrySpoolConfig {
                enabled: true,
                max_records: 1,
                max_batch_records: 1,
            },
        )
        .expect("open spool");
        spool
            .enqueue(record_with_id("record-1"))
            .expect("first enqueue");
        assert!(matches!(
            spool.enqueue(record_with_id("record-2")),
            Err(TelemetrySpoolError::Full)
        ));
        assert_eq!(spool.stats().dropped_records, 1);
        let _ = fs::remove_file(path);
    }
}
