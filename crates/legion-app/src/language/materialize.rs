//! Offline, app-owned language package materialization.
//!
//! This module deliberately has no network or process authority.  A local
//! archive is copied and hashed first, then preflighted with a bounded raw tar
//! reader before the maintained tar parser is allowed to create files.

use flate2::read::GzDecoder;
use legion_lsp::{LspArtifactRuntime, LspDownloadedArtifactMetadata};
use legion_protocol::{CausalityId, CorrelationId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};
use tar::{Archive, EntryType};
use uuid::Uuid;

#[cfg(test)]
type TestPhaseHook = Box<dyn FnMut(MaterializePhase, &CancellationToken)>;

#[cfg(test)]
thread_local! {
    static TEST_PHASE_HOOK: std::cell::RefCell<Option<TestPhaseHook>> = const { std::cell::RefCell::new(None) };
}

/// Maximum compressed local archive size.
pub const DEFAULT_MAX_COMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum aggregate uncompressed archive size.
pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
/// Maximum regular-file size.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum number of archive records, including metadata records.
pub const DEFAULT_MAX_ENTRIES: u64 = 100_000;
/// Maximum individual GNU/PAX metadata payload.
pub const MAX_METADATA_BYTES: u64 = 64 * 1024;
/// Maximum encoded archive path length.
pub const MAX_PATH_BYTES: usize = 4096;
/// Aggregate bounded path/manifest memory budget.
pub const MAX_PATH_MEMORY_BYTES: usize = 16 * 1024 * 1024;
const MATERIALIZER_VERSION: u32 = 1;

fn runtime_identity(runtime: &LspArtifactRuntime) -> String {
    match runtime {
        LspArtifactRuntime::Node { minimum_version } => format!(
            "node:{}.{}.{}",
            minimum_version.major, minimum_version.minor, minimum_version.patch
        ),
    }
}

/// Verified package identity and bounded extraction policy.
#[derive(Debug, Clone)]
pub struct ArtifactDescriptor {
    /// Stable catalog artifact identifier.
    pub artifact_id: String,
    /// Expected package manifest name.
    pub package_name: String,
    /// Expected package manifest version.
    pub version: String,
    /// Supported archive encoding.
    pub archive_format: String,
    /// Expected SHA-256 of the source archive.
    pub expected_sha256: String,
    /// Relative package directory in the archive.
    pub package_root: PathBuf,
    /// Relative runtime entrypoint below the package directory.
    pub entrypoint: PathBuf,
    /// Runtime descriptor retained in the cache manifest.
    pub runtime: LspArtifactRuntime,
}

impl ArtifactDescriptor {
    /// Derive a descriptor from the registry's pinned metadata.
    pub fn from_metadata(
        artifact_id: impl Into<String>,
        checksum: impl Into<String>,
        metadata: &LspDownloadedArtifactMetadata,
    ) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            package_name: metadata.package_name.clone(),
            version: metadata.version.clone(),
            archive_format: metadata.archive_format.clone(),
            expected_sha256: checksum.into(),
            package_root: metadata.package_root.clone(),
            entrypoint: metadata.entrypoint.clone(),
            runtime: metadata.runtime.clone(),
        }
    }
}

/// Source authority accepted by the local importer.
#[derive(Debug, Clone)]
pub enum ArtifactSource {
    /// Archive already present on the local filesystem.
    LocalArchive {
        /// Local archive path.
        path: PathBuf,
    },
}

/// Trusted operation context and bounded local-import request.
#[derive(Debug, Clone)]
pub struct MaterializeRequest {
    /// Pinned archive descriptor.
    pub descriptor: ArtifactDescriptor,
    /// Local source authority.
    pub source: ArtifactSource,
    /// Nonzero operation identifier.
    pub operation_id: u128,
    /// Nonzero correlation identifier.
    pub correlation_id: CorrelationId,
    /// Non-nil causality identifier.
    pub causality_id: CausalityId,
    /// App-approved trust decision; false denies before filesystem effects.
    pub trusted: bool,
    /// App-owned cache root.
    pub cache_root: PathBuf,
    /// Maximum operation duration, capped at ten minutes.
    pub deadline: Duration,
}

impl MaterializeRequest {
    /// Construct a denied-by-default local archive request.
    pub fn local(
        descriptor: ArtifactDescriptor,
        path: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
        operation_id: u128,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> Self {
        Self {
            descriptor,
            source: ArtifactSource::LocalArchive { path: path.into() },
            operation_id,
            correlation_id,
            causality_id,
            trusted: false,
            cache_root: cache_root.into(),
            deadline: Duration::from_secs(600),
        }
    }
}

/// Coalesced progress states emitted by the importer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializeProgress {
    /// Operation identifiers associated with all following events.
    Context {
        /// Current operation identifier.
        operation_id: u128,
        /// Current correlation identifier.
        correlation_id: CorrelationId,
        /// Current causality identifier.
        causality_id: CausalityId,
    },
    /// Worker accepted the request.
    Queued,
    /// Bytes copied and hashed from the local source.
    Copying {
        /// Total source bytes copied so far.
        bytes: u64,
    },
    /// Source digest matched the descriptor.
    Verifying,
    /// Raw archive bounds are being checked.
    Preflighting,
    /// Verified entries are being extracted.
    Extracting {
        /// Total archive records extracted so far.
        entries: u64,
    },
    /// The verified tree is being published atomically.
    Publishing,
    /// An already validated cache was reused.
    Reused,
}

/// Bounded, typed local-import failure categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializeError {
    /// Trust or operation identifiers were invalid.
    Denied(&'static str),
    /// Request descriptor or deadline was malformed.
    InvalidRequest(&'static str),
    /// Bounded filesystem I/O failed.
    Io(String),
    /// Source digest differed from the pinned digest.
    HashMismatch {
        /// Descriptor digest.
        expected: String,
        /// Computed source digest.
        actual: String,
    },
    /// An archive or manifest bound was exceeded.
    LimitExceeded(&'static str),
    /// Raw or maintained tar parsing failed.
    InvalidArchive(String),
    /// A path violated the archive containment policy.
    UnsafePath(String),
    /// Package metadata did not match the descriptor.
    PackageMismatch(&'static str),
    /// Cancellation was observed before publication.
    Cancelled,
    /// The bounded deadline expired.
    TimedOut,
    /// A different cache identity occupied the target.
    CacheCollision,
    /// Existing cache contents failed integrity validation.
    CacheTampered,
}

impl std::fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MaterializeError {}
impl From<io::Error> for MaterializeError {
    fn from(e: io::Error) -> Self {
        Self::Io(bounded_text(e.to_string()))
    }
}

fn bounded_text(text: impl Into<String>) -> String {
    text.into().chars().take(256).collect()
}

/// Verified artifact result with current operation provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedArtifact {
    /// Stable catalog artifact identifier.
    pub artifact_id: String,
    /// Verified package name.
    pub package_name: String,
    /// Verified package version.
    pub version: String,
    /// SHA-256 verified before parsing.
    pub sha256: String,
    /// Published content-addressed cache directory.
    pub cache_root: PathBuf,
    /// Package directory relative to the cache root.
    pub package_root: PathBuf,
    /// Entrypoint relative to the package directory.
    pub entrypoint: PathBuf,
    /// Archive records materialized.
    pub entries: u64,
    /// Aggregate regular-file bytes.
    pub uncompressed_bytes: u64,
    /// Current operation identifier.
    pub operation_id: u128,
    /// Current correlation identifier.
    pub correlation_id: CorrelationId,
    /// Current causality identifier.
    pub causality_id: CausalityId,
    /// Importer-verified manifest retained as opaque launch evidence.
    #[allow(dead_code)]
    verified_manifest: Option<Box<CacheManifest>>,
}

/// Progress or terminal result delivered by the worker.
#[derive(Debug, Clone)]
pub enum MaterializeEvent {
    /// Coalesced bounded progress update.
    Progress(MaterializeProgress),
    /// Verified artifact is ready.
    Ready(Box<MaterializedArtifact>),
    /// Operation terminated before publication.
    Failed(MaterializeError),
}

/// Cooperative cancellation handle shared with the worker.
#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Create a cancellation token.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    /// Signal cooperative cancellation to the worker.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reuse a startup worker's cancellation flag for bounded revalidation.
    pub fn from_atomic(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Receiver for bounded progress and exactly one terminal event.
pub struct MaterializeHandle {
    rx: mpsc::Receiver<MaterializeEvent>,
    cancel: CancellationToken,
}
impl MaterializeHandle {
    /// Request cancellation; the worker cleans only its private staging tree.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    /// Receive the next progress or terminal event, blocking the caller.
    pub fn recv(&self) -> Result<MaterializeEvent, mpsc::RecvError> {
        self.rx.recv()
    }
    /// Poll the next event without blocking.
    pub fn try_recv(&self) -> Result<MaterializeEvent, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}
impl Drop for MaterializeHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn spawn_worker(
    request: MaterializeRequest,
) -> (
    mpsc::Receiver<MaterializeEvent>,
    CancellationToken,
    thread::JoinHandle<()>,
) {
    let (tx, rx) = mpsc::sync_channel(8);
    let cancel = CancellationToken::new();
    let worker_cancel = cancel.clone();
    let join = thread::spawn(move || {
        let _ = tx.try_send(MaterializeEvent::Progress(MaterializeProgress::Context {
            operation_id: request.operation_id,
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
        }));
        let _ = tx.try_send(MaterializeEvent::Progress(MaterializeProgress::Queued));
        let mut progress_sent = 0usize;
        let outcome = materialize_inner(&request, &worker_cancel, |p| {
            // Coalesce metadata progress: dropping an intermediate event is
            // explicitly allowed; terminal delivery remains guaranteed.
            if progress_sent < 5 && tx.try_send(MaterializeEvent::Progress(p)).is_ok() {
                progress_sent += 1;
            }
        });
        let event = match outcome {
            Ok(a) => MaterializeEvent::Ready(Box::new(a)),
            Err(e) => MaterializeEvent::Failed(e),
        };
        let _ = tx.send(event);
    });
    (rx, cancel, join)
}

/// App-owned local archive materializer.
pub struct LanguageArtifactMaterializer;
impl LanguageArtifactMaterializer {
    /// Start a nonblocking worker with bounded progress and guaranteed terminal delivery.
    pub fn start(request: MaterializeRequest) -> MaterializeHandle {
        let (rx, cancel, _join) = spawn_worker(request);
        MaterializeHandle { rx, cancel }
    }
    /// Materialize synchronously for callers already running off the UI thread.
    pub fn materialize(
        request: &MaterializeRequest,
    ) -> Result<MaterializedArtifact, MaterializeError> {
        let token = CancellationToken::new();
        materialize_inner(request, &token, |_| {})
    }

    /// Materialize synchronously while observing the caller's cancellation.
    pub fn materialize_with_cancellation(
        request: &MaterializeRequest,
        cancellation: &CancellationToken,
    ) -> Result<MaterializedArtifact, MaterializeError> {
        materialize_inner(request, cancellation, |_| {})
    }

    /// Revalidate a published cache tree against its pinned descriptor and
    /// manifest before a later process launch.
    pub fn revalidate_materialized_artifact(
        artifact: &MaterializedArtifact,
        descriptor: &ArtifactDescriptor,
        cancellation: &CancellationToken,
    ) -> Result<(), MaterializeError> {
        let guard = Guard {
            token: cancellation,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let expected = artifact
            .verified_manifest
            .as_deref()
            .ok_or(MaterializeError::CacheTampered)?;
        let checked = validate_cache(&artifact.cache_root, descriptor, &guard, expected)?
            .ok_or(MaterializeError::CacheTampered)?;
        if checked.artifact_id != artifact.artifact_id
            || checked.package_name != artifact.package_name
            || checked.version != artifact.version
            || checked.sha256 != artifact.sha256
            || checked.package_root != artifact.package_root
            || checked.entrypoint != artifact.entrypoint
        {
            return Err(MaterializeError::CacheTampered);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheManifest {
    schema: u32,
    materializer_version: u32,
    artifact_id: String,
    package_name: String,
    version: String,
    archive_format: String,
    sha256: String,
    package_root: String,
    entrypoint: String,
    entries: u64,
    uncompressed_bytes: u64,
    runtime: String,
    files: Vec<ManifestFile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ManifestFile {
    path: String,
    sha256: String,
    bytes: u64,
    directory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializePhase {
    Copy,
    Preflight,
    TrailingDrain,
    Extract,
    Manifest,
    Publish,
}

struct Guard<'a> {
    token: &'a CancellationToken,
    deadline: Instant,
}
impl Guard<'_> {
    fn check(&self) -> Result<(), MaterializeError> {
        if self.token.is_cancelled() {
            return Err(MaterializeError::Cancelled);
        }
        if Instant::now() > self.deadline {
            return Err(MaterializeError::TimedOut);
        }
        Ok(())
    }
    fn phase(&self, phase: MaterializePhase) -> Result<(), MaterializeError> {
        #[cfg(not(test))]
        let _ = phase;
        #[cfg(test)]
        TEST_PHASE_HOOK.with(|hook| {
            if let Some(callback) = hook.borrow_mut().as_mut() {
                callback(phase, self.token);
            }
        });
        self.check()
    }
}

fn ensure_cache_namespace(root: &Path) -> Result<(), MaterializeError> {
    if let Ok(meta) = fs::symlink_metadata(root)
        && (meta.file_type().is_symlink() || !meta.is_dir())
    {
        return Err(MaterializeError::CacheTampered);
    }
    // Walk the declared path so a planted symlink still fails closed. Host
    // prefixes such as macOS `/var` → `/private/var` keep the same final
    // component name; a redirected ancestor that changes identity is tamper.
    let mut current = PathBuf::new();
    for component in root.components() {
        current.push(component.as_os_str());
        // On Windows a verbatim path begins with a Prefix component such as
        // \\?\C:. Querying that incomplete prefix asks Win32 to stat a
        // device-like path and returns ERROR_INVALID_FUNCTION. The complete
        // root component is the first filesystem anchor; continue checking
        // it and every descendant for symlink entries.
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                if current.as_path() == root {
                    return Err(MaterializeError::CacheTampered);
                }
                let canonical = fs::canonicalize(&current)
                    .map_err(|error| MaterializeError::Io(bounded_text(error.to_string())))?;
                if canonical.file_name() != current.file_name() {
                    return Err(MaterializeError::CacheTampered);
                }
                current = canonical;
            }
            Ok(meta) if !meta.is_dir() => return Err(MaterializeError::CacheTampered),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(MaterializeError::Io(bounded_text(error.to_string()))),
        }
    }
    let namespace = root.join("sha256");
    if let Ok(meta) = fs::symlink_metadata(&namespace)
        && (meta.file_type().is_symlink() || !meta.is_dir())
    {
        return Err(MaterializeError::CacheTampered);
    }
    Ok(())
}

fn materialize_inner(
    request: &MaterializeRequest,
    token: &CancellationToken,
    mut progress: impl FnMut(MaterializeProgress),
) -> Result<MaterializedArtifact, MaterializeError> {
    if !request.trusted
        || request.operation_id == 0
        || request.correlation_id.0 == 0
        || request.causality_id.0.is_nil()
    {
        return Err(MaterializeError::Denied(
            "untrusted or invalid operation context",
        ));
    }
    if request.deadline.is_zero() || request.deadline > Duration::from_secs(600) {
        return Err(MaterializeError::InvalidRequest("deadline"));
    }
    if request.descriptor.archive_format != "tar.gz"
        || request.descriptor.expected_sha256.len() != 64
    {
        return Err(MaterializeError::InvalidRequest("descriptor"));
    }
    validate_rel(&request.descriptor.package_root, false)
        .map_err(|_| MaterializeError::InvalidRequest("package_root"))?;
    validate_rel(&request.descriptor.entrypoint, false)
        .map_err(|_| MaterializeError::InvalidRequest("entrypoint"))?;
    if request.descriptor.package_name.is_empty() || request.descriptor.version.is_empty() {
        return Err(MaterializeError::InvalidRequest("package identity"));
    }
    if !request.cache_root.is_absolute() {
        return Err(MaterializeError::InvalidRequest("cache_root"));
    }
    let guard = Guard {
        token,
        deadline: Instant::now()
            .checked_add(request.deadline)
            .unwrap_or_else(Instant::now),
    };
    let source_path = match &request.source {
        ArtifactSource::LocalArchive { path } => path,
    };
    guard.check()?;
    ensure_cache_namespace(&request.cache_root)?;
    fs::create_dir_all(&request.cache_root)?;
    let staging = request.cache_root.join(format!(
        ".staging-{}-{}",
        request.operation_id,
        Uuid::new_v4()
    ));
    fs::create_dir_all(&staging)?;
    let staged_archive = staging.join("archive.tgz");
    let result = (|| {
        let actual = copy_hash(source_path, &staged_archive, &guard, &mut progress)?;
        let actual_hex = hex::encode(actual);
        if !actual_hex.eq_ignore_ascii_case(&request.descriptor.expected_sha256) {
            return Err(MaterializeError::HashMismatch {
                expected: request.descriptor.expected_sha256.clone(),
                actual: actual_hex,
            });
        }
        progress(MaterializeProgress::Verifying);
        let final_root = request
            .cache_root
            .join("sha256")
            .join(request.descriptor.expected_sha256.to_ascii_lowercase());
        guard.check()?;
        progress(MaterializeProgress::Preflighting);
        preflight(&staged_archive, &guard)?;
        let extracted = staging.join("tree");
        fs::create_dir_all(&extracted)?;
        let stats = extract(
            &staged_archive,
            &extracted,
            &request.descriptor,
            &guard,
            &mut progress,
        )?;
        let artifact = validate_and_manifest(&extracted, &request.descriptor, stats)?;
        let expected_manifest = manifest_for(&extracted, &artifact, &request.descriptor, &guard)?;
        let mut artifact = artifact;
        artifact.verified_manifest = Some(Box::new(expected_manifest.clone()));
        if let Some(existing) =
            validate_cache(&final_root, &request.descriptor, &guard, &expected_manifest)?
        {
            progress(MaterializeProgress::Reused);
            return Ok(artifact_with_context(existing, request));
        }
        progress(MaterializeProgress::Publishing);
        guard.phase(MaterializePhase::Publish)?;
        if final_root.exists() {
            if validate_cache(&final_root, &request.descriptor, &guard, &expected_manifest)?
                .is_some()
            {
                return Ok(artifact_with_context(
                    artifact_with_root(artifact, final_root),
                    request,
                ));
            }
            return Err(MaterializeError::CacheCollision);
        }
        if let Some(parent) = final_root.parent() {
            guard.check()?;
            fs::create_dir_all(parent)?;
        }
        let mut published =
            artifact_with_context(artifact_with_root(artifact, final_root.clone()), request);
        let manifest = expected_manifest;
        let bytes = serde_json::to_vec(&manifest)
            .map_err(|e| MaterializeError::Io(bounded_text(e.to_string())))?;
        let side = extracted.join(".legion-manifest.json");
        let tmp = extracted.join(".legion-manifest.tmp");
        guard.check()?;
        fs::write(&tmp, bytes)?;
        guard.check()?;
        fs::rename(tmp, side)?;
        guard.check()?;
        fs::rename(&extracted, &final_root)?;
        published.cache_root = final_root;
        Ok(published)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

fn copy_hash(
    src: &Path,
    dst: &Path,
    guard: &Guard<'_>,
    progress: &mut impl FnMut(MaterializeProgress),
) -> Result<[u8; 32], MaterializeError> {
    let mut input = File::open(src)?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        guard.phase(MaterializePhase::Copy)?;
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or(MaterializeError::LimitExceeded("compressed bytes"))?;
        if total > DEFAULT_MAX_COMPRESSED_BYTES {
            return Err(MaterializeError::LimitExceeded("compressed bytes"));
        }
        output.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        progress(MaterializeProgress::Copying { bytes: total });
    }
    output.flush()?;
    Ok(hasher.finalize().into())
}

fn read_num(bytes: &[u8]) -> Result<u64, MaterializeError> {
    if bytes.first().is_some_and(|b| b & 0x80 != 0) {
        let mut n = (bytes[0] & 0x7f) as u64;
        for &b in &bytes[1..] {
            n = n
                .checked_mul(256)
                .and_then(|n| n.checked_add(b as u64))
                .ok_or(MaterializeError::LimitExceeded("tar size"))?;
        }
        return Ok(n);
    }
    let text = bytes
        .iter()
        .copied()
        .filter(|b| *b != 0 && *b != b' ')
        .collect::<Vec<_>>();
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(
        std::str::from_utf8(&text)
            .map_err(|_| MaterializeError::InvalidArchive("tar number".into()))?,
        8,
    )
    .map_err(|_| MaterializeError::InvalidArchive("tar number".into()))
}
fn checksum(header: &[u8; 512]) -> Result<(), MaterializeError> {
    let expected = read_num(&header[148..156])?;
    let mut sum = 0u64;
    for (i, b) in header.iter().enumerate() {
        sum += if (148..156).contains(&i) {
            32
        } else {
            *b as u64
        };
    }
    if sum != expected {
        return Err(MaterializeError::InvalidArchive("tar checksum".into()));
    }
    Ok(())
}
fn padded(n: u64) -> Result<u64, MaterializeError> {
    n.checked_add(511)
        .map(|x| x / 512 * 512)
        .ok_or(MaterializeError::LimitExceeded("tar size"))
}

fn preflight(path: &Path, guard: &Guard<'_>) -> Result<(), MaterializeError> {
    let file = File::open(path)?;
    let mut gz = GzDecoder::new(file);
    let mut header = [0u8; 512];
    let mut scratch = [0u8; 64 * 1024];
    let mut names = BTreeMap::<String, bool>::new();
    let mut path_memory = 0usize;
    let mut pending_path: Option<Vec<u8>> = None;
    let mut pending_pax: Option<(Vec<u8>, Option<u64>)> = None;
    let mut entries = 0u64;
    let mut bytes = 0u64;
    let mut zero_blocks = 0;
    loop {
        guard.phase(MaterializePhase::Preflight)?;
        gz.read_exact(&mut header).map_err(|e| {
            if e.kind() == ErrorKind::UnexpectedEof {
                MaterializeError::InvalidArchive("truncated tar".into())
            } else {
                e.into()
            }
        })?;
        if header.iter().all(|b| *b == 0) {
            zero_blocks += 1;
            if zero_blocks == 2 {
                break;
            }
            continue;
        }
        zero_blocks = 0;
        checksum(&header)?;
        let size = read_num(&header[124..136])?;
        let typ = header[156];
        if typ == b'K' {
            return Err(MaterializeError::InvalidArchive(
                "GNU long link metadata is forbidden".into(),
            ));
        }
        let payload = typ == b'L' || typ == b'x' || typ == b'g';
        if payload {
            entries = entries
                .checked_add(1)
                .ok_or(MaterializeError::LimitExceeded("entries"))?;
            if entries > DEFAULT_MAX_ENTRIES {
                return Err(MaterializeError::LimitExceeded("entries"));
            }
            if size > MAX_METADATA_BYTES {
                return Err(MaterializeError::LimitExceeded("tar metadata"));
            }
            let mut data = Vec::with_capacity(size as usize);
            let mut remaining = size;
            while remaining > 0 {
                guard.check()?;
                let take = remaining.min(scratch.len() as u64) as usize;
                gz.read_exact(&mut scratch[..take])?;
                data.extend_from_slice(&scratch[..take]);
                remaining -= take as u64;
            }
            let mut pad = padded(size)?.saturating_sub(size);
            while pad > 0 {
                guard.check()?;
                let take = pad.min(scratch.len() as u64) as usize;
                gz.read_exact(&mut scratch[..take])?;
                pad -= take as u64;
            }
            bytes = bytes
                .checked_add(size)
                .ok_or(MaterializeError::LimitExceeded("archive bytes"))?;
            if bytes > DEFAULT_MAX_UNCOMPRESSED_BYTES {
                return Err(MaterializeError::LimitExceeded("uncompressed bytes"));
            }
            if typ == b'L' {
                if pending_path.is_some() {
                    return Err(MaterializeError::InvalidArchive(
                        "consecutive GNU long-name records without target".into(),
                    ));
                }
                pending_path = Some(data);
            } else if typ == b'g' {
                return Err(MaterializeError::InvalidArchive(
                    "global pax metadata is unsupported by the bounded importer".into(),
                ));
            } else {
                if pending_pax.is_some() {
                    return Err(MaterializeError::InvalidArchive(
                        "consecutive local PAX records without target".into(),
                    ));
                }
                let (_path, pax_size) = parse_pax(&data)?;
                pending_pax = Some((data, pax_size));
            }
            continue;
        }
        let mut name = header_name(&header)?;
        let mut effective_size = size;
        let mut has_long_path = false;
        if let Some(long) = pending_path.take() {
            name = long.into_iter().take_while(|b| *b != 0).collect();
            has_long_path = true;
        }
        if let Some((pax, pax_size)) = pending_pax.take() {
            if !has_long_path && let Some(p) = parse_pax(&pax)?.0 {
                name = p;
            }
            if let Some(pax_size) = pax_size {
                effective_size = pax_size;
            }
        }
        if effective_size > DEFAULT_MAX_FILE_BYTES && matches!(header[156], 0 | b'0') {
            return Err(MaterializeError::LimitExceeded("file bytes"));
        }
        bytes = bytes
            .checked_add(effective_size)
            .ok_or(MaterializeError::LimitExceeded("archive bytes"))?;
        if bytes > DEFAULT_MAX_UNCOMPRESSED_BYTES {
            return Err(MaterializeError::LimitExceeded("uncompressed bytes"));
        }
        let type_ok = matches!(typ, 0 | b'0' | b'5');
        if !type_ok {
            return Err(MaterializeError::InvalidArchive(
                "unsupported tar entry type".into(),
            ));
        }
        let text = std::str::from_utf8(&name)
            .map_err(|_| MaterializeError::UnsafePath("non-utf8 path".into()))?;
        validate_rel(Path::new(text), typ == b'5')
            .map_err(|_| MaterializeError::UnsafePath(bounded_text(text)))?;
        path_memory = path_memory.saturating_add(text.len() + 64);
        if path_memory > MAX_PATH_MEMORY_BYTES {
            return Err(MaterializeError::LimitExceeded("path memory"));
        }
        insert_canonical_path(&mut names, text, typ == b'5')?;
        entries += 1;
        if entries > DEFAULT_MAX_ENTRIES {
            return Err(MaterializeError::LimitExceeded("entries"));
        }
        let mut remaining = padded(effective_size)?;
        while remaining > 0 {
            guard.check()?;
            let take = remaining.min(scratch.len() as u64) as usize;
            gz.read_exact(&mut scratch[..take])?;
            remaining -= take as u64;
        }
    }
    if pending_path.is_some() || pending_pax.is_some() {
        return Err(MaterializeError::InvalidArchive(
            "metadata without target".into(),
        ));
    }
    let mut tail = [0u8; 64 * 1024];
    let mut trailing = 0u64;
    loop {
        guard.phase(MaterializePhase::TrailingDrain)?;
        let n = gz.read(&mut tail)?;
        if n == 0 {
            break;
        }
        trailing = trailing.saturating_add(n as u64);
        if trailing > MAX_METADATA_BYTES * 16 || tail[..n].iter().any(|b| *b != 0) {
            return Err(MaterializeError::InvalidArchive("trailing tar data".into()));
        }
    }
    let _ = entries;
    Ok(())
}

fn insert_canonical_path(
    names: &mut BTreeMap<String, bool>,
    text: &str,
    directory: bool,
) -> Result<(), MaterializeError> {
    let canonical = text.to_ascii_lowercase();
    if names.contains_key(&canonical) {
        return Err(MaterializeError::InvalidArchive(
            "duplicate/conflicting path".into(),
        ));
    }
    if !directory
        && let Some((existing, _)) = names.range(canonical.clone()..).next()
        && existing.starts_with(&(canonical.clone() + "/"))
    {
        return Err(MaterializeError::InvalidArchive(
            "directory ancestor conflicts with file".into(),
        ));
    }
    let mut prefix = String::new();
    let parts: Vec<_> = canonical.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            prefix.push('/');
        }
        prefix.push_str(part);
        if index + 1 < parts.len()
            && let Some(is_dir) = names.get(&prefix)
            && !*is_dir
        {
            return Err(MaterializeError::InvalidArchive(
                "file used as directory ancestor".into(),
            ));
        }
    }
    names.insert(canonical, directory);
    Ok(())
}

fn header_name(h: &[u8; 512]) -> Result<Vec<u8>, MaterializeError> {
    let mut out = Vec::new();
    out.extend_from_slice(&h[..100]);
    let prefix = &h[345..500];
    while out.last() == Some(&0) || out.last() == Some(&b' ') {
        out.pop();
    }
    let mut p = prefix.to_vec();
    while p.last() == Some(&0) || p.last() == Some(&b' ') {
        p.pop();
    }
    if !p.is_empty() {
        let mut full = p;
        full.push(b'/');
        full.extend_from_slice(&out);
        out = full;
    }
    Ok(out)
}
fn parse_pax(data: &[u8]) -> Result<(Option<Vec<u8>>, Option<u64>), MaterializeError> {
    let mut i = 0;
    let mut path = None;
    let mut size = None;
    while i < data.len() {
        let start = i;
        while i < data.len() && data[i] != b' ' {
            if !data[i].is_ascii_digit() {
                return Err(MaterializeError::InvalidArchive("malformed pax".into()));
            }
            i += 1;
        }
        if i == data.len() {
            return Err(MaterializeError::InvalidArchive("malformed pax".into()));
        }
        let len: usize = std::str::from_utf8(&data[start..i])
            .unwrap()
            .parse()
            .map_err(|_| MaterializeError::InvalidArchive("malformed pax".into()))?;
        if len < 3 || start.checked_add(len).is_none() || start + len > data.len() {
            return Err(MaterializeError::InvalidArchive("malformed pax".into()));
        }
        let rec = &data[i + 1..start + len];
        if !rec.ends_with(b"\n") {
            return Err(MaterializeError::InvalidArchive("malformed pax".into()));
        }
        if let Some(eq) = rec.iter().position(|b| *b == b'=') {
            let key = &rec[..eq];
            let val = &rec[eq + 1..rec.len() - 1];
            if key.starts_with(b"GNU.sparse.")
                || key.starts_with(b"SCHILY.xattr.")
                || key.starts_with(b"SCHILY.acl.")
                || key.starts_with(b"LIBARCHIVE.xattr.")
                || key.starts_with(b"LIBARCHIVE.acl.")
                || key.starts_with(b"security.")
                || key == b"linkpath"
            {
                return Err(MaterializeError::InvalidArchive(
                    "forbidden pax metadata".into(),
                ));
            }
            if key == b"path" {
                path = Some(val.to_vec());
            }
            if key == b"size" {
                size = Some(
                    std::str::from_utf8(val)
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| {
                            MaterializeError::InvalidArchive("malformed pax size".into())
                        })?,
                );
            }
        }
        i = start + len;
    }
    Ok((path, size))
}

#[derive(Default)]
struct Stats {
    entries: u64,
    bytes: u64,
}
fn extract(
    path: &Path,
    root: &Path,
    _descriptor: &ArtifactDescriptor,
    guard: &Guard<'_>,
    progress: &mut impl FnMut(MaterializeProgress),
) -> Result<Stats, MaterializeError> {
    let file = File::open(path)?;
    let mut archive = Archive::new(GzDecoder::new(file));
    let mut seen = HashSet::new();
    let mut stats = Stats::default();
    for item in archive
        .entries()
        .map_err(|e| MaterializeError::InvalidArchive(bounded_text(e.to_string())))?
    {
        guard.phase(MaterializePhase::Extract)?;
        let mut entry =
            item.map_err(|e| MaterializeError::InvalidArchive(bounded_text(e.to_string())))?;
        let et = entry.header().entry_type();
        if matches!(
            et,
            EntryType::XHeader | EntryType::XGlobalHeader | EntryType::GNULongName
        ) {
            let mut scratch = [0u8; 64 * 1024];
            loop {
                guard.check()?;
                let n = entry.read(&mut scratch)?;
                if n == 0 {
                    break;
                }
            }
            continue;
        }
        if !(et == EntryType::Regular || et == EntryType::Directory) {
            return Err(MaterializeError::InvalidArchive(
                "unsupported tar entry type".into(),
            ));
        }
        let raw = entry.path_bytes();
        let name = std::str::from_utf8(&raw)
            .map_err(|_| MaterializeError::UnsafePath("non-utf8 path".into()))?;
        validate_rel(Path::new(name), et == EntryType::Directory)
            .map_err(|_| MaterializeError::UnsafePath(bounded_text(name)))?;
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(MaterializeError::InvalidArchive(
                "duplicate/conflicting path".into(),
            ));
        }
        let dest = root.join(name);
        if !dest.starts_with(root) {
            return Err(MaterializeError::UnsafePath(bounded_text(name)));
        }
        if et == EntryType::Directory {
            fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let size = entry.size();
            if size > DEFAULT_MAX_FILE_BYTES {
                return Err(MaterializeError::LimitExceeded("file bytes"));
            }
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest)?;
            let mut limited = entry.by_ref().take(DEFAULT_MAX_FILE_BYTES + 1);
            let mut copied = 0u64;
            let mut buf = [0u8; 64 * 1024];
            loop {
                guard.check()?;
                let n = limited.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                copied = copied.saturating_add(n as u64);
                out.write_all(&buf[..n])?;
            }
            if copied > DEFAULT_MAX_FILE_BYTES {
                return Err(MaterializeError::LimitExceeded("file bytes"));
            }
            if copied != size {
                return Err(MaterializeError::InvalidArchive(
                    "truncated tar file".into(),
                ));
            }
            stats.bytes = stats
                .bytes
                .checked_add(copied)
                .ok_or(MaterializeError::LimitExceeded("uncompressed bytes"))?;
        }
        stats.entries += 1;
        if stats.entries > DEFAULT_MAX_ENTRIES {
            return Err(MaterializeError::LimitExceeded("entries"));
        }
        if stats.bytes > DEFAULT_MAX_UNCOMPRESSED_BYTES {
            return Err(MaterializeError::LimitExceeded("uncompressed bytes"));
        }
        progress(MaterializeProgress::Extracting {
            entries: stats.entries,
        });
    }
    Ok(stats)
}

fn validate_rel(path: &Path, directory: bool) -> Result<(), ()> {
    let s = path.to_str().ok_or(())?;
    if s.is_empty()
        || s.len() > MAX_PATH_BYTES
        || s.contains('\\')
        || s.contains("//")
        || s.contains(':')
        || s.starts_with('/')
        || s.starts_with("//")
        || s.ends_with('.')
        || s.ends_with(' ')
    {
        return Err(());
    }
    let mut depth = 0;
    for c in path.components() {
        match c {
            Component::Normal(v) => {
                let n = v.to_str().ok_or(())?;
                if n.is_empty()
                    || n == "."
                    || n == ".."
                    || n.ends_with('.')
                    || n.ends_with(' ')
                    || is_reserved(n)
                {
                    return Err(());
                }
                depth += 1;
                if n.len() > 255 {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
    }
    if depth == 0 || depth > 64 || (!directory && s.ends_with('/')) {
        return Err(());
    }
    Ok(())
}
fn is_reserved(s: &str) -> bool {
    let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(
        base.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn validate_and_manifest(
    root: &Path,
    d: &ArtifactDescriptor,
    stats: Stats,
) -> Result<MaterializedArtifact, MaterializeError> {
    let package = root.join(&d.package_root);
    let entry = package.join(&d.entrypoint);
    if !package.is_dir() {
        return Err(MaterializeError::PackageMismatch("package root"));
    }
    if !entry.is_file() {
        return Err(MaterializeError::PackageMismatch("entrypoint"));
    }
    let package_json = package.join("package.json");
    let metadata = fs::metadata(&package_json)?;
    if metadata.len() > 1024 * 1024 {
        return Err(MaterializeError::LimitExceeded("package manifest"));
    }
    let value: serde_json::Value = serde_json::from_slice(&fs::read(package_json)?)
        .map_err(|_| MaterializeError::PackageMismatch("package.json"))?;
    if value.get("name").and_then(|v| v.as_str()) != Some(d.package_name.as_str()) {
        return Err(MaterializeError::PackageMismatch("package name"));
    }
    if value.get("version").and_then(|v| v.as_str()) != Some(d.version.as_str()) {
        return Err(MaterializeError::PackageMismatch("package version"));
    }
    Ok(MaterializedArtifact {
        artifact_id: d.artifact_id.clone(),
        package_name: d.package_name.clone(),
        version: d.version.clone(),
        sha256: d.expected_sha256.to_ascii_lowercase(),
        cache_root: root.to_path_buf(),
        package_root: d.package_root.clone(),
        entrypoint: d.entrypoint.clone(),
        entries: stats.entries,
        uncompressed_bytes: stats.bytes,
        operation_id: 0,
        correlation_id: CorrelationId(0),
        causality_id: CausalityId(uuid::Uuid::nil()),
        verified_manifest: None,
    })
}
fn artifact_with_root(mut a: MaterializedArtifact, root: PathBuf) -> MaterializedArtifact {
    a.cache_root = root;
    a
}
fn artifact_with_context(
    mut a: MaterializedArtifact,
    request: &MaterializeRequest,
) -> MaterializedArtifact {
    a.operation_id = request.operation_id;
    a.correlation_id = request.correlation_id;
    a.causality_id = request.causality_id;
    a
}
fn manifest_for(
    root: &Path,
    a: &MaterializedArtifact,
    d: &ArtifactDescriptor,
    guard: &Guard<'_>,
) -> Result<CacheManifest, MaterializeError> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut mem = 0usize;
    while let Some(dir) = stack.pop() {
        guard.phase(MaterializePhase::Manifest)?;
        for item in fs::read_dir(dir)? {
            guard.check()?;
            let e = item?;
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap();
            if rel == Path::new(".legion-manifest.json") || rel == Path::new(".legion-manifest.tmp")
            {
                continue;
            }
            let md = e.metadata()?;
            let text = rel.to_string_lossy().replace('\\', "/");
            mem = mem.saturating_add(text.len() + 64);
            if mem > MAX_PATH_MEMORY_BYTES {
                return Err(MaterializeError::LimitExceeded("path memory"));
            }
            if md.is_dir() {
                files.push(ManifestFile {
                    path: text,
                    sha256: String::new(),
                    bytes: 0,
                    directory: true,
                });
                stack.push(p);
            } else if md.is_file() {
                let mut f = File::open(&p)?;
                let mut h = Sha256::new();
                let mut n = 0;
                let mut b = [0; 65536];
                loop {
                    guard.check()?;
                    let k = f.read(&mut b)?;
                    if k == 0 {
                        break;
                    }
                    h.update(&b[..k]);
                    n += k as u64;
                }
                files.push(ManifestFile {
                    path: text,
                    sha256: hex::encode(h.finalize()),
                    bytes: n,
                    directory: false,
                });
            }
        }
    }
    Ok(CacheManifest {
        schema: 1,
        materializer_version: MATERIALIZER_VERSION,
        artifact_id: a.artifact_id.clone(),
        package_name: d.package_name.clone(),
        version: d.version.clone(),
        archive_format: d.archive_format.clone(),
        sha256: a.sha256.clone(),
        package_root: d.package_root.to_string_lossy().into(),
        entrypoint: d.entrypoint.to_string_lossy().into(),
        entries: a.entries,
        uncompressed_bytes: a.uncompressed_bytes,
        runtime: runtime_identity(&d.runtime),
        files,
    })
}
fn validate_cache(
    root: &Path,
    d: &ArtifactDescriptor,
    guard: &Guard<'_>,
    expected: &CacheManifest,
) -> Result<Option<MaterializedArtifact>, MaterializeError> {
    guard.check()?;
    if !root.exists() {
        return Ok(None);
    }
    let root_meta = fs::symlink_metadata(root).map_err(|_| MaterializeError::CacheTampered)?;
    if !root_meta.is_dir() {
        return Err(MaterializeError::CacheTampered);
    }
    let side = root.join(".legion-manifest.json");
    let side_meta = fs::symlink_metadata(&side).map_err(|_| MaterializeError::CacheTampered)?;
    if !side_meta.is_file() || side_meta.len() > MAX_PATH_MEMORY_BYTES as u64 {
        return Err(MaterializeError::CacheTampered);
    }
    let mut side_bytes = Vec::with_capacity(side_meta.len() as usize);
    File::open(&side)?
        .take(MAX_PATH_MEMORY_BYTES as u64 + 1)
        .read_to_end(&mut side_bytes)?;
    if side_bytes.len() > MAX_PATH_MEMORY_BYTES {
        return Err(MaterializeError::CacheTampered);
    }
    let m: CacheManifest =
        serde_json::from_slice(&side_bytes).map_err(|_| MaterializeError::CacheTampered)?;
    if m.schema != 1
        || m.materializer_version != MATERIALIZER_VERSION
        || m.artifact_id != d.artifact_id
        || m.package_name != d.package_name
        || m.version != d.version
        || m.archive_format != d.archive_format
        || m.package_root != d.package_root.to_string_lossy()
        || m.entrypoint != d.entrypoint.to_string_lossy()
        || m.sha256 != d.expected_sha256.to_ascii_lowercase()
        || m.runtime != runtime_identity(&d.runtime)
    {
        return Err(MaterializeError::CacheTampered);
    }
    if &m != expected {
        return Err(MaterializeError::CacheTampered);
    }
    let mut listed = HashSet::new();
    let mut path_memory = 0usize;
    for f in &m.files {
        guard.check()?;
        path_memory = path_memory.saturating_add(f.path.len() + 64);
        if path_memory > MAX_PATH_MEMORY_BYTES
            || validate_rel(Path::new(&f.path), f.directory).is_err()
            || !listed.insert(f.path.clone())
        {
            return Err(MaterializeError::CacheTampered);
        }
        let p = root.join(&f.path);
        let md = fs::symlink_metadata(&p).map_err(|_| MaterializeError::CacheTampered)?;
        if f.directory {
            if !md.is_dir() || f.bytes != 0 {
                return Err(MaterializeError::CacheTampered);
            }
        } else {
            if !md.is_file() || md.len() != f.bytes {
                return Err(MaterializeError::CacheTampered);
            }
            let mut h = Sha256::new();
            let mut file = File::open(&p)?;
            let mut b = [0; 65536];
            loop {
                guard.check()?;
                let n = file.read(&mut b)?;
                if n == 0 {
                    break;
                }
                h.update(&b[..n]);
            }
            if hex::encode(h.finalize()) != f.sha256 {
                return Err(MaterializeError::CacheTampered);
            }
        }
    }
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        guard.check()?;
        for item in fs::read_dir(&dir).map_err(|_| MaterializeError::CacheTampered)? {
            let entry = item.map_err(|_| MaterializeError::CacheTampered)?;
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .map_err(|_| MaterializeError::CacheTampered)?;
            if rel == Path::new(".legion-manifest.json") {
                continue;
            }
            let rel_text = rel.to_string_lossy().replace('\\', "/");
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| MaterializeError::CacheTampered)?;
            if !listed.contains(&rel_text) || metadata.file_type().is_symlink() {
                return Err(MaterializeError::CacheTampered);
            }
            if metadata.is_dir() {
                dirs.push(path);
            }
        }
    }
    let package = root.join(&d.package_root);
    let entry = package.join(&d.entrypoint);
    if !package.is_dir() || !entry.is_file() {
        return Err(MaterializeError::CacheTampered);
    }
    Ok(Some(MaterializedArtifact {
        artifact_id: d.artifact_id.clone(),
        package_name: d.package_name.clone(),
        version: d.version.clone(),
        sha256: d.expected_sha256.to_ascii_lowercase(),
        cache_root: root.to_path_buf(),
        package_root: d.package_root.clone(),
        entrypoint: d.entrypoint.clone(),
        entries: m.entries,
        uncompressed_bytes: m.uncompressed_bytes,
        operation_id: 0,
        correlation_id: CorrelationId(0),
        causality_id: CausalityId(uuid::Uuid::nil()),
        verified_manifest: Some(Box::new(expected.clone())),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn verbatim_workspace_cache_namespace_skips_incomplete_drive_prefix() {
        let workspace = tempfile::tempdir().expect("workspace");
        let canonical_workspace =
            std::fs::canonicalize(workspace.path()).expect("canonical workspace");
        let cache = canonical_workspace.join(".legion").join("language-tools");
        ensure_cache_namespace(&cache).expect("verbatim cache namespace");
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_that_changes_identity_is_tamper() {
        let dir = tempfile::tempdir().expect("workspace");
        let real = dir.path().join("real");
        let alias = dir.path().join("alias");
        std::fs::create_dir(&real).expect("real dir");
        std::os::unix::fs::symlink(&real, &alias).expect("plant redirected ancestor");
        let cache = alias.join("language-tools");
        std::fs::create_dir(&cache).expect("cache through alias");
        assert_eq!(
            ensure_cache_namespace(&cache),
            Err(MaterializeError::CacheTampered)
        );
    }

    #[test]
    fn relative_cache_root_is_rejected_before_filesystem_work() {
        let dir = tempfile::tempdir().expect("workspace");
        let archive = dir.path().join("artifact.tgz");
        let bytes = valid_test_archive();
        std::fs::write(&archive, &bytes).expect("archive");
        let hash = hex::encode(Sha256::digest(&bytes));
        let relative_cache = PathBuf::from(format!("relative-cache-{}", uuid::Uuid::new_v4()));
        assert!(!relative_cache.exists());
        let request = test_request(&hash, &archive, &relative_cache);
        assert_eq!(
            materialize_inner(&request, &CancellationToken::new(), |_| {}),
            Err(MaterializeError::InvalidRequest("cache_root"))
        );
        assert!(!relative_cache.exists());
    }

    #[cfg(windows)]
    #[test]
    fn drive_relative_cache_root_is_rejected_before_filesystem_work() {
        let dir = tempfile::tempdir().expect("workspace");
        let archive = dir.path().join("artifact.tgz");
        let bytes = valid_test_archive();
        std::fs::write(&archive, &bytes).expect("archive");
        let hash = hex::encode(Sha256::digest(&bytes));
        let request = test_request(&hash, &archive, Path::new(r"C:relative-cache"));
        assert_eq!(
            materialize_inner(&request, &CancellationToken::new(), |_| {}),
            Err(MaterializeError::InvalidRequest("cache_root"))
        );
    }

    #[test]
    fn phase_hook_cancels_real_pipeline_at_each_bounded_phase() {
        let phases = [
            MaterializePhase::Copy,
            MaterializePhase::Preflight,
            MaterializePhase::TrailingDrain,
            MaterializePhase::Extract,
            MaterializePhase::Manifest,
            MaterializePhase::Publish,
        ];
        for phase in phases {
            let dir = tempfile::tempdir().unwrap();
            let archive_path = dir.path().join("artifact.tgz");
            let bytes = valid_test_archive();
            std::fs::write(&archive_path, &bytes).unwrap();
            let hash = hex::encode(Sha256::digest(&bytes));
            let request = test_request(&hash, &archive_path, &dir.path().join("cache"));
            let reached = Arc::new(std::sync::Mutex::new(false));
            let reached_by_hook = reached.clone();
            TEST_PHASE_HOOK.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move |observed, token| {
                    if observed == phase {
                        *reached_by_hook.lock().unwrap() = true;
                        token.cancel();
                    }
                }));
            });
            let token = CancellationToken::new();
            let result = materialize_inner(&request, &token, |_| {});
            TEST_PHASE_HOOK.with(|hook| *hook.borrow_mut() = None);
            assert_eq!(result, Err(MaterializeError::Cancelled), "phase {phase:?}");
            assert!(*reached.lock().unwrap(), "phase {phase:?} was not reached");
            assert!(!dir.path().join("cache/sha256").exists());
            assert!(
                std::fs::read_dir(dir.path().join("cache"))
                    .map(|entries| entries
                        .flatten()
                        .all(|entry| !entry.file_name().to_string_lossy().starts_with(".staging-")))
                    .unwrap_or(true)
            );
        }
    }

    #[test]
    fn phase_hook_expires_real_pipeline_deadline_at_late_phases() {
        for phase in [MaterializePhase::Manifest, MaterializePhase::Publish] {
            let dir = tempfile::tempdir().unwrap();
            let archive_path = dir.path().join("artifact.tgz");
            let bytes = valid_test_archive();
            std::fs::write(&archive_path, &bytes).unwrap();
            let hash = hex::encode(Sha256::digest(&bytes));
            let mut request = test_request(&hash, &archive_path, &dir.path().join("cache"));
            request.deadline = Duration::from_millis(100);
            let reached = Arc::new(std::sync::Mutex::new(false));
            let reached_by_hook = reached.clone();
            TEST_PHASE_HOOK.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move |observed, _token| {
                    if observed == phase {
                        *reached_by_hook.lock().unwrap() = true;
                        std::thread::sleep(Duration::from_millis(150));
                    }
                }));
            });
            let token = CancellationToken::new();
            let result = materialize_inner(&request, &token, |_| {});
            TEST_PHASE_HOOK.with(|hook| *hook.borrow_mut() = None);
            assert_eq!(result, Err(MaterializeError::TimedOut), "phase {phase:?}");
            assert!(*reached.lock().unwrap(), "phase {phase:?} was not reached");
            assert!(!dir.path().join("cache/sha256").exists());
            assert!(
                std::fs::read_dir(dir.path().join("cache"))
                    .map(|entries| entries
                        .flatten()
                        .all(|entry| !entry.file_name().to_string_lossy().starts_with(".staging-")))
                    .unwrap_or(true)
            );
        }
    }

    fn valid_test_archive() -> Vec<u8> {
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        {
            let mut tar = tar::Builder::new(&mut gzip);
            for (name, body) in [
                (
                    "package/package.json",
                    br#"{"name":"pyright","version":"1.1.400"}"#.as_slice(),
                ),
                ("package/langserver.index.js", b"entry".as_slice()),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar.append_data(&mut header, name, body).unwrap();
            }
            tar.finish().unwrap();
        }
        gzip.finish().unwrap()
    }

    fn test_request(hash: &str, archive: &Path, cache: &Path) -> MaterializeRequest {
        let mut request = MaterializeRequest::local(
            ArtifactDescriptor {
                artifact_id: "pyright".into(),
                package_name: "pyright".into(),
                version: "1.1.400".into(),
                archive_format: "tar.gz".into(),
                expected_sha256: hash.into(),
                package_root: PathBuf::from("package"),
                entrypoint: PathBuf::from("langserver.index.js"),
                runtime: LspArtifactRuntime::Node {
                    minimum_version: legion_lsp::LspNodeVersion {
                        major: 14,
                        minor: 0,
                        patch: 0,
                    },
                },
            },
            archive,
            cache,
            7,
            CorrelationId(1),
            CausalityId(Uuid::new_v4()),
        );
        request.trusted = true;
        request
    }

    #[test]
    fn retained_manifest_rejects_coordinated_cache_tamper_and_honors_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("artifact.tgz");
        let bytes = valid_test_archive();
        std::fs::write(&archive_path, &bytes).unwrap();
        let hash = hex::encode(Sha256::digest(&bytes));
        let request = test_request(&hash, &archive_path, &dir.path().join("cache"));
        let artifact = LanguageArtifactMaterializer::materialize(&request).unwrap();
        let token = CancellationToken::new();
        LanguageArtifactMaterializer::revalidate_materialized_artifact(
            &artifact,
            &request.descriptor,
            &token,
        )
        .unwrap();
        let reused = LanguageArtifactMaterializer::materialize(&request).unwrap();
        LanguageArtifactMaterializer::revalidate_materialized_artifact(
            &reused,
            &request.descriptor,
            &token,
        )
        .unwrap();
        std::fs::write(
            reused.cache_root.join("package/langserver.index.js"),
            b"tampered",
        )
        .unwrap();
        let manifest_path = reused.cache_root.join(".legion-manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        let files = manifest["files"].as_array_mut().unwrap();
        let tampered_hash = hex::encode(Sha256::digest(b"tampered"));
        let entry = files
            .iter_mut()
            .find(|entry| entry["path"] == "package/langserver.index.js")
            .unwrap();
        entry["sha256"] = serde_json::Value::String(tampered_hash);
        entry["bytes"] = serde_json::Value::from(8u64);
        std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(matches!(
            LanguageArtifactMaterializer::revalidate_materialized_artifact(
                &reused,
                &request.descriptor,
                &token,
            ),
            Err(MaterializeError::CacheTampered)
        ));
        token.cancel();
        assert_eq!(
            LanguageArtifactMaterializer::revalidate_materialized_artifact(
                &reused,
                &request.descriptor,
                &token,
            ),
            Err(MaterializeError::Cancelled)
        );
    }

    #[test]
    fn worker_finishes_with_receiver_unread_and_one_terminal_event() {
        for bad_hash in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let archive_path = dir.path().join("artifact.tgz");
            let bytes = valid_test_archive();
            std::fs::write(&archive_path, &bytes).unwrap();
            let hash = if bad_hash {
                "00".repeat(32)
            } else {
                hex::encode(Sha256::digest(&bytes))
            };
            let request = test_request(&hash, &archive_path, &dir.path().join("cache"));
            let (rx, _cancel, join) = spawn_worker(request);
            let started = std::time::Instant::now();
            while !join.is_finished() {
                assert!(started.elapsed() < Duration::from_secs(2));
                std::thread::yield_now();
            }
            join.join().unwrap();
            let events: Vec<_> = rx.try_iter().collect();
            assert!(events.len() <= 8);
            let terminals: Vec<_> = events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        MaterializeEvent::Ready(_) | MaterializeEvent::Failed(_)
                    )
                })
                .collect();
            assert_eq!(terminals.len(), 1);
            match (bad_hash, terminals[0]) {
                (false, MaterializeEvent::Ready(_)) => {}
                (true, MaterializeEvent::Failed(actual)) => {
                    assert!(matches!(actual, MaterializeError::HashMismatch { .. }));
                    assert!(
                        matches!(actual, MaterializeError::HashMismatch { expected, .. } if *expected == "00".repeat(32))
                    );
                }
                other => panic!("unexpected worker result: {other:?}"),
            }
        }
    }
}
