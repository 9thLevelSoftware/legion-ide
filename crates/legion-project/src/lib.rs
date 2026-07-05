//! Project model: workspace, file tree, file watcher, and trust-aware VFS resolution.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use globset::{Glob, GlobSet};
use ignore::WalkBuilder;
use regex::RegexBuilder;
use tantivy::{
    Index, Term,
    collector::TopDocs,
    doc,
    query::{BooleanQuery, Occur, Query, TermQuery},
    schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value},
    tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer},
};

use legion_index::{SourceDocument, TreeSitterParser, tree_sitter_supports_path};
use legion_observability::{
    NoopEventSink, conflict_created_event, fallback_denied_event, open_file_read_failure_event,
    security_denial_event, stale_proposal_rejected_event, watcher_recovery_event,
};
use legion_platform::{
    FileSystemEntryKind, FileSystemMetadata, FileSystemService, PathNormalizationService,
    PlatformError, WatcherService,
};
use legion_protocol::{
    BufferVersion, CanonicalPath, CapabilityId, CapabilityRequestContext, CausalityId,
    CorrelationId, DebugConfigurationId, DebugLaunchConfiguration, DebugLaunchRequestKind,
    EventSequence, EventSinkPort, EventSinkRequest, FileConflictContext,
    FileConflictLifecycleState, FileConflictReason, FileConflictState, FileContentVersion,
    FileFingerprint as ProtocolFileFingerprint, FileId, FileIdentity, FileKind, FileMetadata,
    FileTreeDelta, FileTreeDeltaOp, FileTreeNode, LanguageId, LanguageOutlineSymbolProjection,
    PrincipalId, ProjectId, ProposalDenialReason, ProposalFailureReason, ProposalId,
    ProposalLifecycleState, ProposalLifecycleTransition, ProposalResponse, ProposalStaleContext,
    ProposalStaleReason, ProposalVersionPreconditions, ProtocolDiagnostic,
    ProtocolDiagnosticSeverity, ProtocolError, ProtocolResult, SemanticPrivacyScope, SnapshotId,
    TimestampMillis, WatcherEvent, WatcherEventKind, WorkspaceCloseRequest, WorkspaceClosed,
    WorkspaceConfigSnapshot, WorkspaceDiscoveryChangeKind, WorkspaceDiscoveryDecision,
    WorkspaceDiscoveryDelta, WorkspaceDiscoveryPathPolicyResult, WorkspaceDiscoveryPolicyDecision,
    WorkspaceDiscoveryRecord, WorkspaceDiscoverySkipReason, WorkspaceDiscoverySnapshot,
    WorkspaceDiscoveryTrustResult, WorkspaceGeneration, WorkspaceId, WorkspaceOpenRequest,
    WorkspaceOpened, WorkspaceRequest, WorkspaceResponse, WorkspaceRootId, WorkspaceTrustState,
};
use legion_security::{DenyByDefaultBroker, TrustState};
use thiserror::Error;
use uuid::Uuid;

/// Internal filesystem trait alias used by [`WorkspaceActor`] for path-normalization and file-system operations.
pub trait ProjectFilesystemService:
    PathNormalizationService + FileSystemService + Send + Sync
{
}

impl<T> ProjectFilesystemService for T where
    T: PathNormalizationService + FileSystemService + Send + Sync
{
}

type ProjectFilesystem = dyn ProjectFilesystemService;

const LARGE_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TREE_CHILDREN_DEPTH: usize = 2;
const WATCHER_EVENT_BUFFER: usize = 1_024;
const WATCHER_RENAME_DEBOUNCE_MILLIS: u64 = 64;
const WATCHER_RECOVERY_MAX_RESCANS: usize = 2;
const WORKSPACE_SEARCH_MAX_FILE_BYTES: u64 = 256 * 1024;

type WorkspaceScanResult = (
    Vec<FileTreeNode>,
    HashMap<String, FileFingerprint>,
    Vec<WorkspaceDiscoveryRecord>,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum SearchPatternKind {
    Literal,
    Regex,
}

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct SearchPattern {
    regex: regex::Regex,
}

#[allow(missing_docs)]
impl SearchPattern {
    pub fn literal(
        pattern: &str,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Result<Self, regex::Error> {
        Self::build(
            pattern,
            SearchPatternKind::Literal,
            case_sensitive,
            whole_word,
        )
    }

    pub fn regex(
        pattern: &str,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Result<Self, regex::Error> {
        Self::build(
            pattern,
            SearchPatternKind::Regex,
            case_sensitive,
            whole_word,
        )
    }

    fn build(
        pattern: &str,
        kind: SearchPatternKind,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Result<Self, regex::Error> {
        let mut compiled = match kind {
            SearchPatternKind::Literal => regex::escape(pattern),
            SearchPatternKind::Regex => pattern.to_string(),
        };
        if whole_word {
            compiled = format!(r"\b(?:{})\b", compiled);
        }
        let mut builder = RegexBuilder::new(&compiled);
        builder.case_insensitive(!case_sensitive);
        Ok(Self {
            regex: builder.build()?,
        })
    }

    pub fn find_ranges(&self, text: &str) -> Vec<Range<usize>> {
        self.regex
            .find_iter(text)
            .map(|m| m.start()..m.end())
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
#[allow(missing_docs)]
pub struct WorkspaceSearchFilters {
    pub include: Option<Arc<GlobSet>>,
    pub exclude: Option<Arc<GlobSet>>,
}

impl WorkspaceSearchFilters {
    fn accepts(&self, path: &Path) -> bool {
        if self.include.as_ref().is_some_and(|set| !set.is_match(path)) {
            return false;
        }
        if self.exclude.as_ref().is_some_and(|set| set.is_match(path)) {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct WorkspaceSearchQuery {
    pub workspace_id: WorkspaceId,
    pub pattern: SearchPattern,
    pub search_text: String,
    pub filters: WorkspaceSearchFilters,
    pub result_limit: usize,
    pub batch_size: usize,
    pub use_indexed_backend: bool,
}

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct WorkspaceSearchHit {
    pub file_id: FileId,
    pub canonical_path: CanonicalPath,
    /// One-based line number of the match (matches the blame convention).
    pub line_number: u32,
    pub byte_range: Range<u64>,
    pub line_text: String,
    pub snippet: String,
    pub snippet_truncated: bool,
}

#[derive(Debug, Clone, Default)]
#[allow(missing_docs)]
pub struct WorkspaceSearchBatch {
    pub hits: Vec<WorkspaceSearchHit>,
    pub omitted_hit_count: usize,
    pub omitted_file_count: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default)]
#[allow(missing_docs)]
pub struct WorkspaceSearchReport {
    pub hit_count: usize,
    pub omitted_hit_count: usize,
    pub omitted_file_count: usize,
    pub diagnostics: Vec<String>,
    pub cancelled: bool,
}

type WorkspaceSearchSnapshot = (
    PathBuf,
    WorkspaceGeneration,
    Vec<(FileId, String, Option<FileMetadata>)>,
);

fn workspace_search_snippet(line: &str) -> (String, bool) {
    const SEARCH_SNIPPET_LIMIT_BYTES: usize = 160;
    if line.len() <= SEARCH_SNIPPET_LIMIT_BYTES {
        return (line.to_string(), false);
    }

    let mut end = SEARCH_SNIPPET_LIMIT_BYTES;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}...", &line[..end]), true)
}

fn emit_workspace_search_batch<F>(
    pending_hits: &mut Vec<WorkspaceSearchHit>,
    pending_omitted_hit_count: &mut usize,
    pending_omitted_file_count: &mut usize,
    pending_diagnostics: &mut Vec<String>,
    on_batch: &mut F,
) -> bool
where
    F: FnMut(WorkspaceSearchBatch) -> bool,
{
    if pending_hits.is_empty()
        && *pending_omitted_hit_count == 0
        && *pending_omitted_file_count == 0
        && pending_diagnostics.is_empty()
    {
        return true;
    }

    let batch = WorkspaceSearchBatch {
        hits: std::mem::take(pending_hits),
        omitted_hit_count: std::mem::take(pending_omitted_hit_count),
        omitted_file_count: std::mem::take(pending_omitted_file_count),
        diagnostics: std::mem::take(pending_diagnostics),
    };
    on_batch(batch)
}

fn workspace_search_path_label(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn workspace_search_match_count(text: &str, pattern: &SearchPattern) -> Vec<Range<usize>> {
    pattern.find_ranges(text)
}

struct WorkspaceScanAccumulation {
    nodes: Vec<FileTreeNode>,
    fingerprints: HashMap<String, FileFingerprint>,
    discovery_records: Vec<WorkspaceDiscoveryRecord>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |dur| dur.as_millis() as u64)
}

/// Algorithm version tag for [`stable_hash`]. Bump this when the hashing
/// algorithm changes so that persisted/protocol-facing ids derived from it
/// (WorkspaceId, git hunk ids, content-version digests) shift in a detectable,
/// intentional way rather than silently colliding across versions.
const STABLE_HASH_VERSION: u8 = 1;

/// Deterministic, cross-version-stable 128-bit hash (FNV-1a).
///
/// Used for persisted/protocol-facing ids. Unlike `std`'s `DefaultHasher`,
/// FNV-1a is a fixed published specification, so its output does not change
/// across compiler/std versions. The [`STABLE_HASH_VERSION`] tag is mixed in
/// first so the id space is namespaced by algorithm version.
fn stable_hash(value: &str) -> u128 {
    // FNV-1a (128-bit) constants.
    const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    let mut hash = FNV_OFFSET_BASIS;
    // Mix the algorithm version tag in first.
    hash ^= STABLE_HASH_VERSION as u128;
    hash = hash.wrapping_mul(FNV_PRIME);
    for byte in value.as_bytes() {
        hash ^= *byte as u128;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn trust_to_protocol(state: TrustState) -> WorkspaceTrustState {
    match state {
        TrustState::Trusted => WorkspaceTrustState::Trusted,
        TrustState::Untrusted => WorkspaceTrustState::Untrusted,
        TrustState::Unknown => WorkspaceTrustState::Unknown,
    }
}

fn language_id_for_path(path: &CanonicalPath) -> LanguageId {
    let lower = path.0.to_ascii_lowercase();
    let language = if lower.ends_with(".rs") {
        "rust"
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        "typescript"
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") {
        "javascript"
    } else if lower.ends_with(".md") {
        "markdown"
    } else if lower.ends_with(".json") {
        "json"
    } else {
        "text"
    };
    LanguageId(language.to_string())
}

#[derive(Debug, Error)]
/// Errors emitted by the workspace VFS.
pub enum WorkspaceError {
    /// Workspace has not been opened in this actor instance.
    #[error("workspace {workspace_id:?} has not been opened")]
    WorkspaceMissing {
        /// Workspace id.
        workspace_id: WorkspaceId,
    },
    /// Candidate path is outside the workspace root boundary.
    #[error("path `{path}` is outside workspace root boundary")]
    PathOutsideRoot {
        /// Requested canonical path.
        path: String,
    },
    /// Security policy denied the operation.
    #[error("security denied operation for `{path}`: {reason}")]
    SecurityDenied {
        /// Requested path.
        path: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Platform-level error propagated as protocol-facing error.
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    /// Internal data inconsistency.
    #[error("internal error: {0}")]
    Internal(&'static str),
}

type WorkspaceResult<T> = Result<T, WorkspaceError>;

/// Errors emitted while inspecting or mutating git metadata.
#[derive(Debug, Error)]
pub enum GitInspectionError {
    /// Git executable could not be launched.
    #[error("git command `{command}` could not be launched: {source}")]
    Launch {
        /// Display-safe git subcommand label.
        command: String,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Git exited unsuccessfully.
    #[error("git command `{command}` failed: {stderr}")]
    CommandFailed {
        /// Display-safe git subcommand label.
        command: String,
        /// Redacted stderr text.
        stderr: String,
    },
    /// Git output could not be interpreted.
    #[error("git output parse error: {0}")]
    Parse(String),
    /// Input validation failed before any git command was run.
    #[error("git input validation error: {0}")]
    InvalidInput(String),
}

/// Configurable bounds for git projection collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshotOptions {
    /// File-size threshold for syntactic diff metadata before line-diff fallback is reported.
    pub max_file_bytes_for_syntactic_diff: u64,
    /// Maximum number of hunks projected per snapshot.
    pub max_hunks: usize,
    /// Maximum active-file blame lines projected.
    pub max_blame_lines: usize,
    /// Maximum commit graph rows projected.
    pub max_commits: usize,
}

/// Errors emitted while discovering debug configurations from project metadata.
#[derive(Debug, Error)]
pub enum DebugLocatorError {
    /// Cargo manifest could not be read.
    #[error("cargo manifest could not be read at `{path}`: {source}")]
    ManifestRead {
        /// Manifest path.
        path: String,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Cargo manifest did not contain enough metadata.
    #[error("cargo manifest parse error: {0}")]
    ManifestParse(String),
}

/// Options for deterministic Cargo debug configuration discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDebugLocatorOptions {
    /// Debug adapter type to place in launch configurations.
    pub adapter_type: String,
    /// Display-safe Cargo target directory label.
    pub target_dir_label: String,
}

impl Default for CargoDebugLocatorOptions {
    fn default() -> Self {
        Self {
            adapter_type: "lldb-dap".to_string(),
            target_dir_label: "target/debug".to_string(),
        }
    }
}

impl Default for GitSnapshotOptions {
    fn default() -> Self {
        Self {
            max_file_bytes_for_syntactic_diff: 512 * 1024,
            max_hunks: 128,
            max_blame_lines: 256,
            max_commits: 64,
        }
    }
}

/// Discover deterministic Cargo launch configurations for binary targets.
pub fn discover_cargo_debug_configurations(
    root: &Path,
    options: CargoDebugLocatorOptions,
) -> Result<Vec<DebugLaunchConfiguration>, DebugLocatorError> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|source| {
        DebugLocatorError::ManifestRead {
            path: manifest_path.to_string_lossy().into_owned(),
            source,
        }
    })?;
    let package_name = cargo_manifest_package_name(&manifest).ok_or_else(|| {
        DebugLocatorError::ManifestParse("Cargo.toml missing [package] name".to_string())
    })?;
    let mut bins = Vec::new();
    if root.join("src/main.rs").is_file() {
        bins.push(package_name.clone());
    }
    for bin in cargo_manifest_bin_names(&manifest) {
        if !bins.contains(&bin) {
            bins.push(bin);
        }
    }
    bins.sort();

    let workspace_id = WorkspaceId(stable_hash(&root.to_string_lossy()));
    let cwd = CanonicalPath(path_label(root));
    Ok(bins
        .into_iter()
        .map(|bin| DebugLaunchConfiguration {
            configuration_id: DebugConfigurationId(format!("cargo:{package_name}:bin:{bin}")),
            workspace_id,
            name: format!("Debug {bin}"),
            adapter_type: options.adapter_type.clone(),
            request: DebugLaunchRequestKind::Launch,
            program_label: format!("{}/{}", options.target_dir_label.trim_end_matches('/'), bin),
            cwd: cwd.clone(),
            cargo_package: Some(package_name.clone()),
            cargo_target: Some(bin.clone()),
            cargo_args: vec![
                "build".to_string(),
                "--package".to_string(),
                package_name.clone(),
                "--bin".to_string(),
                bin,
            ],
            stop_on_entry: false,
            deterministic: true,
            schema_version: 1,
        })
        .collect())
}

fn cargo_manifest_package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && let Some(name) = parse_toml_string_assignment(trimmed, "name") {
            return Some(name);
        }
    }
    None
}

fn cargo_manifest_bin_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_bin = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_bin = trimmed == "[[bin]]";
            continue;
        }
        if in_bin && let Some(name) = parse_toml_string_assignment(trimmed, "name") {
            names.push(name);
        }
    }
    names
}

fn parse_toml_string_assignment(line: &str, key: &str) -> Option<String> {
    let without_comment = line.split_once('#').map_or(line, |(value, _)| value).trim();
    let (left, right) = without_comment.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = right.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.to_string())
}

fn path_label(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Diff strategy projected for a changed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitDiffStrategy {
    /// Deterministic syntax-aware metadata is available for this file.
    Syntactic,
    /// The projection fell back to line-diff metadata.
    LineFallback,
}

/// Stage where a git hunk currently lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHunkStage {
    /// Hunk exists only in the working tree.
    Unstaged,
    /// Hunk exists in the git index.
    Staged,
}

/// One changed file in a git projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitChangedFile {
    /// Repository-relative path.
    pub path: String,
    /// Two-column porcelain status.
    pub status: String,
    /// Number of inserted lines from numstat metadata.
    pub inserted_lines: u32,
    /// Number of deleted lines from numstat metadata.
    pub deleted_lines: u32,
    /// Number of unstaged hunks projected for this file.
    pub unstaged_hunk_count: usize,
    /// Number of staged hunks projected for this file.
    pub staged_hunk_count: usize,
    /// Whether the projected hunks expose stage/unstage actions.
    pub stageable: bool,
    /// Diff strategy selected for this file.
    pub diff_strategy: GitDiffStrategy,
    /// Fallback reason when syntactic metadata is unavailable.
    pub fallback_reason: Option<String>,
    /// Whether merge conflict markers were detected in the file.
    pub conflict: bool,
}

/// One hunk projected from git diff output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitHunk {
    /// Stable hunk identifier derived from path, stage, and hunk header.
    pub hunk_id: String,
    /// Repository-relative path.
    pub path: String,
    /// Current stage of the hunk.
    pub stage: GitHunkStage,
    /// Unified diff hunk header.
    pub header: String,
    /// Old-file start line.
    pub old_start: u32,
    /// Old-file line count.
    pub old_lines: u32,
    /// New-file start line.
    pub new_start: u32,
    /// New-file line count.
    pub new_lines: u32,
    /// Added line count in this hunk.
    pub added_lines: u32,
    /// Deleted line count in this hunk.
    pub deleted_lines: u32,
    /// Optional function or scope context from the hunk header.
    pub context: Option<String>,
    /// Patch payload scoped to this single hunk for git-apply hunk staging.
    pub patch: String,
}

/// One inline blame row for the active file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitBlameLine {
    /// Repository-relative path.
    pub path: String,
    /// One-based line number in the current file.
    pub line_number: u32,
    /// Short commit hash or all-zero worktree marker.
    pub commit_short: String,
    /// Commit author label.
    pub author: String,
    /// Commit summary label.
    pub summary: String,
    /// Bounded source preview for the line.
    pub line_preview: String,
}

/// One commit in the projected git graph/history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitCommit {
    /// Full commit hash.
    pub hash: String,
    /// Short commit hash.
    pub short_hash: String,
    /// Author label.
    pub author: String,
    /// Commit date label from git.
    pub date: String,
    /// Commit summary.
    pub summary: String,
    /// Number of parents.
    pub parent_count: usize,
    /// Decorated refs reported by git.
    pub refs: Vec<String>,
}

/// Merge-conflict marker projection for a changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitConflict {
    /// Repository-relative path.
    pub path: String,
    /// Number of conflict marker lines.
    pub marker_count: usize,
    /// Deterministic local resolution actions exposed by the UI.
    pub actions: Vec<String>,
}

/// Projected git worktree classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectGitWorktreeKind {
    /// Worktree used for delegated agent isolation.
    Agent,
    /// Human-managed worktree.
    Manual,
}

/// Forge family detected from a git remote URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitForgeKind {
    /// GitHub remote/compare URL shape.
    GitHub,
    /// GitLab remote/merge-request URL shape.
    GitLab,
}

/// Forge-agnostic PR URL builder.
pub trait GitForge {
    /// Return the forge kind handled by this builder.
    fn kind(&self) -> GitForgeKind;

    /// Build a pull-request/merge-request URL for the remote and branches.
    fn pull_request_url(
        &self,
        remote_url: &str,
        base_branch: &str,
        head_branch: &str,
    ) -> Option<String>;
}

/// GitHub compare/PR URL builder.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitHubForge;

impl GitForge for GitHubForge {
    fn kind(&self) -> GitForgeKind {
        GitForgeKind::GitHub
    }

    fn pull_request_url(
        &self,
        remote_url: &str,
        base_branch: &str,
        head_branch: &str,
    ) -> Option<String> {
        let repo = git_forge_repository(remote_url, GitForgeKind::GitHub)?;
        Some(format!(
            "https://github.com/{}/compare/{}...{}",
            repo,
            percent_encode_path_segment(base_branch),
            percent_encode_path_segment(head_branch)
        ))
    }
}

/// GitLab merge-request URL builder.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitLabForge;

impl GitForge for GitLabForge {
    fn kind(&self) -> GitForgeKind {
        GitForgeKind::GitLab
    }

    fn pull_request_url(
        &self,
        remote_url: &str,
        base_branch: &str,
        head_branch: &str,
    ) -> Option<String> {
        let repo = git_forge_repository(remote_url, GitForgeKind::GitLab)?;
        Some(format!(
            "https://gitlab.com/{}/-/merge_requests/new?merge_request[source_branch]={}&merge_request[target_branch]={}",
            repo,
            percent_encode_query_value(head_branch),
            percent_encode_query_value(base_branch)
        ))
    }
}

/// Detect the forge family for a remote URL.
pub fn git_forge_kind(remote_url: &str) -> Option<GitForgeKind> {
    let host = git_remote_host(remote_url)?;
    if host.eq_ignore_ascii_case("github.com") {
        Some(GitForgeKind::GitHub)
    } else if host.eq_ignore_ascii_case("gitlab.com") {
        Some(GitForgeKind::GitLab)
    } else {
        None
    }
}

/// Build a pull-request/merge-request URL for a supported remote URL.
pub fn git_pull_request_url(
    remote_url: &str,
    base_branch: &str,
    head_branch: &str,
) -> Option<String> {
    match git_forge_kind(remote_url)? {
        GitForgeKind::GitHub => GitHubForge.pull_request_url(remote_url, base_branch, head_branch),
        GitForgeKind::GitLab => GitLabForge.pull_request_url(remote_url, base_branch, head_branch),
    }
}

fn git_forge_repository(remote_url: &str, kind: GitForgeKind) -> Option<String> {
    let (host, path) = git_remote_host_and_path(remote_url)?;
    let expected_host = match kind {
        GitForgeKind::GitHub => "github.com",
        GitForgeKind::GitLab => "gitlab.com",
    };
    if !host.eq_ignore_ascii_case(expected_host) {
        return None;
    }
    Some(path)
}

fn git_remote_host(remote_url: &str) -> Option<String> {
    git_remote_host_and_path(remote_url).map(|(host, _)| host)
}

fn git_remote_host_and_path(remote_url: &str) -> Option<(String, String)> {
    let remote_url = remote_url.trim();
    if remote_url.is_empty() {
        return None;
    }

    let remote_url = remote_url.strip_prefix("git+").unwrap_or(remote_url).trim();

    if let Some((scheme, rest)) = remote_url.split_once("://")
        && matches!(scheme, "http" | "https" | "ssh")
    {
        let rest = rest.trim_start_matches('/');
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = host.rsplit_once('@').map(|(_, host)| host).unwrap_or(host);
        return Some((host.to_string(), normalize_repo_path(path)));
    }

    if let Some((host_and_user, path)) = remote_url.split_once(':')
        && host_and_user.contains('@')
        && !host_and_user.contains('/')
    {
        let host = host_and_user
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(host_and_user)
            .to_string();
        return Some((host, normalize_repo_path(path)));
    }

    let (host, path) = remote_url.split_once('/')?;
    Some((host.to_string(), normalize_repo_path(path)))
}

fn normalize_repo_path(path: &str) -> String {
    path.trim_start_matches('/')
        .trim_start_matches(':')
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string()
}

fn percent_encode_path_segment(value: &str) -> String {
    percent_encode(value, false)
}

fn percent_encode_query_value(value: &str) -> String {
    percent_encode(value, true)
}

fn percent_encode(value: &str, encode_slash: bool) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        let keep = matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        ) || (!encode_slash && byte == b'/');
        if keep {
            encoded.push(byte as char);
        } else {
            let _ = write!(&mut encoded, "%{:02X}", byte);
        }
    }
    encoded
}

/// Projected git worktree row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitWorktree {
    /// Worktree path.
    pub path: String,
    /// Current branch label when available.
    pub branch_label: Option<String>,
    /// Current short HEAD hash when available.
    pub head_short: Option<String>,
    /// Worktree category.
    pub kind: ProjectGitWorktreeKind,
    /// Whether git considers the worktree prunable/orphaned.
    pub prunable: bool,
}

/// Full git projection collected for a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGitSnapshot {
    /// Repository root.
    pub root: CanonicalPath,
    /// Current branch label when available.
    pub branch_label: Option<String>,
    /// Current short HEAD hash when available.
    pub head_short: Option<String>,
    /// Repository origin remote URL when available.
    pub remote_url: Option<String>,
    /// Origin default branch label when available.
    pub remote_default_branch: Option<String>,
    /// Changed files.
    pub changed_files: Vec<ProjectGitChangedFile>,
    /// Staged and unstaged hunks.
    pub hunks: Vec<ProjectGitHunk>,
    /// Inline blame lines for the active file.
    pub blame_lines: Vec<ProjectGitBlameLine>,
    /// Commit graph/history rows.
    pub commits: Vec<ProjectGitCommit>,
    /// Conflict marker projections.
    pub conflicts: Vec<ProjectGitConflict>,
    /// Projected worktree rows.
    pub worktrees: Vec<ProjectGitWorktree>,
    /// Display-safe diagnostics.
    pub diagnostics: Vec<String>,
    /// Snapshot timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u32,
}

/// Git inspection backend used for the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitInspectionBackend {
    /// Existing `git` CLI implementation.
    Cli,
    /// Pure-Rust `gix` implementation.
    Gix,
}

/// Collect deterministic git metadata with an explicit backend.
pub fn collect_git_snapshot_with_backend(
    root: impl AsRef<Path>,
    active_file: Option<&Path>,
    options: GitSnapshotOptions,
    backend: GitInspectionBackend,
) -> Result<ProjectGitSnapshot, GitInspectionError> {
    let root = root.as_ref();
    let repository_root = git_stdout(root, &["rev-parse", "--show-toplevel"], None)?;
    let repository_root = PathBuf::from(repository_root.trim());
    let branch_label = git_stdout(
        &repository_root,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        None,
    )
    .ok()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty());
    let head_short = git_stdout(&repository_root, &["rev-parse", "--short", "HEAD"], None)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let remote_url = git_remote_url(&repository_root, "origin");
    let remote_default_branch =
        git_remote_default_branch(&repository_root, "origin").or_else(|| branch_label.clone());

    let status_entries = match backend {
        GitInspectionBackend::Cli => git_status_entries(&repository_root)?,
        GitInspectionBackend::Gix => git_status_entries_gix(&repository_root)
            .or_else(|_| git_status_entries(&repository_root))?,
    };
    let unstaged_numstat = git_numstat(&repository_root, false)?;
    let staged_numstat = git_numstat(&repository_root, true)?;
    let mut hunks = Vec::new();
    hunks.extend(git_diff_hunks(
        &repository_root,
        GitHunkStage::Unstaged,
        options.max_hunks,
    )?);
    if hunks.len() < options.max_hunks {
        hunks.extend(git_diff_hunks(
            &repository_root,
            GitHunkStage::Staged,
            options.max_hunks - hunks.len(),
        )?);
    }

    let conflicts = git_conflicts(&repository_root, status_entries.keys())?;
    let worktrees = git_worktrees(&repository_root)?;
    let changed_files = git_changed_files(
        &repository_root,
        status_entries,
        &unstaged_numstat,
        &staged_numstat,
        &hunks,
        &conflicts,
        options.max_file_bytes_for_syntactic_diff,
    );
    let active_relative = active_file.and_then(|path| relative_git_path(&repository_root, path));
    let blame_lines = match active_relative.as_deref() {
        Some(path) => match backend {
            GitInspectionBackend::Cli => {
                git_blame_lines(&repository_root, path, options.max_blame_lines)?
            }
            GitInspectionBackend::Gix => {
                git_blame_lines_gix(&repository_root, path, options.max_blame_lines)
                    .or_else(|_| git_blame_lines(&repository_root, path, options.max_blame_lines))?
            }
        },
        None => Vec::new(),
    };
    let commits = git_commits(&repository_root, options.max_commits)?;

    Ok(ProjectGitSnapshot {
        root: CanonicalPath(repository_root.to_string_lossy().into_owned()),
        branch_label,
        head_short,
        remote_url,
        remote_default_branch,
        changed_files,
        hunks,
        blame_lines,
        commits,
        conflicts,
        worktrees,
        diagnostics: Vec::new(),
        generated_at: TimestampMillis(now_millis()),
        schema_version: 1,
    })
}

/// Collect deterministic git status, diff, blame, history, and conflict metadata for a workspace.
pub fn collect_git_snapshot(
    root: impl AsRef<Path>,
    active_file: Option<&Path>,
    options: GitSnapshotOptions,
) -> Result<ProjectGitSnapshot, GitInspectionError> {
    collect_git_snapshot_with_backend(root, active_file, options, GitInspectionBackend::Gix)
}

fn git_worktree_kind(path: &Path) -> ProjectGitWorktreeKind {
    let path = path.to_string_lossy();
    if path.contains("target/delegated-tasks/task-") {
        ProjectGitWorktreeKind::Agent
    } else {
        ProjectGitWorktreeKind::Manual
    }
}

fn git_remote_url(root: &Path, remote: &str) -> Option<String> {
    git_stdout(root, &["remote", "get-url", remote], None)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn git_remote_default_branch(root: &Path, remote: &str) -> Option<String> {
    let head_ref = format!("refs/remotes/{remote}/HEAD");
    git_stdout(
        root,
        &["symbolic-ref", "--quiet", "--short", &head_ref],
        None,
    )
    .ok()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .map(|value| {
        value
            .strip_prefix(&format!("{remote}/"))
            .unwrap_or(&value)
            .to_string()
    })
}

fn git_worktrees(root: &Path) -> Result<Vec<ProjectGitWorktree>, GitInspectionError> {
    let output = git_stdout(root, &["worktree", "list", "--porcelain"], None)?;
    let mut worktrees = Vec::new();
    let mut current: Option<ProjectGitWorktree> = None;

    let flush = |worktrees: &mut Vec<ProjectGitWorktree>,
                 current: &mut Option<ProjectGitWorktree>| {
        if let Some(worktree) = current.take() {
            worktrees.push(worktree);
        }
    };

    for line in output.lines() {
        if line.trim().is_empty() {
            flush(&mut worktrees, &mut current);
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            flush(&mut worktrees, &mut current);
            let path = PathBuf::from(path.trim());
            current = Some(ProjectGitWorktree {
                path: path.to_string_lossy().into_owned(),
                branch_label: None,
                head_short: None,
                kind: git_worktree_kind(&path),
                prunable: false,
            });
            continue;
        }
        let Some(worktree) = current.as_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            worktree.head_short = Some(head.trim().to_string());
            continue;
        }
        if let Some(branch) = line.strip_prefix("branch ") {
            let branch = branch.trim();
            worktree.branch_label = branch
                .strip_prefix("refs/heads/")
                .map(|label| label.to_string())
                .or_else(|| {
                    if branch.contains("detached") {
                        None
                    } else {
                        Some(branch.to_string())
                    }
                });
            continue;
        }
        if line.starts_with("prunable ") {
            worktree.prunable = true;
        }
    }

    flush(&mut worktrees, &mut current);
    Ok(worktrees)
}

/// Switch to an existing branch.
pub fn switch_git_branch(root: impl AsRef<Path>, branch: &str) -> Result<(), GitInspectionError> {
    git_stdout(root.as_ref(), &["switch", branch], None).map(|_| ())
}

/// Create and switch to a new branch.
pub fn create_git_branch(root: impl AsRef<Path>, branch: &str) -> Result<(), GitInspectionError> {
    git_stdout(root.as_ref(), &["switch", "-c", branch], None).map(|_| ())
}

/// Delete a branch that has been merged.
pub fn delete_git_branch(root: impl AsRef<Path>, branch: &str) -> Result<(), GitInspectionError> {
    git_stdout(root.as_ref(), &["branch", "-d", branch], None).map(|_| ())
}

/// Stash tracked and untracked changes.
pub fn stash_git_changes(
    root: impl AsRef<Path>,
    message: Option<&str>,
) -> Result<(), GitInspectionError> {
    let mut args = vec![
        "stash".to_string(),
        "push".to_string(),
        "--include-untracked".to_string(),
    ];
    if let Some(message) = message {
        args.push("-m".to_string());
        args.push(message.to_string());
    }
    git_stdout_owned(root.as_ref(), &args, None).map(|_| ())
}

/// Remove a worktree by path.
pub fn remove_git_worktree(root: impl AsRef<Path>, path: &Path) -> Result<(), GitInspectionError> {
    let path = path.to_string_lossy().into_owned();
    let args = vec![
        "worktree".to_string(),
        "remove".to_string(),
        "--force".to_string(),
        path,
    ];
    git_stdout_owned(root.as_ref(), &args, None).map(|_| ())
}

/// Prune orphaned git worktree metadata.
pub fn prune_git_worktrees(root: impl AsRef<Path>) -> Result<(), GitInspectionError> {
    git_stdout(root.as_ref(), &["worktree", "prune"], None).map(|_| ())
}

/// Stage one projected unstaged git hunk.
pub fn stage_git_hunk(
    root: impl AsRef<Path>,
    hunk: &ProjectGitHunk,
) -> Result<(), GitInspectionError> {
    if hunk.stage != GitHunkStage::Unstaged {
        return Err(GitInspectionError::Parse(
            "only unstaged hunks can be staged".to_string(),
        ));
    }
    git_stdout(
        root.as_ref(),
        &["apply", "--cached", "--unidiff-zero", "-"],
        Some(hunk.patch.as_bytes()),
    )
    .map(|_| ())
}

/// Unstage one projected staged git hunk.
pub fn unstage_git_hunk(
    root: impl AsRef<Path>,
    hunk: &ProjectGitHunk,
) -> Result<(), GitInspectionError> {
    if hunk.stage != GitHunkStage::Staged {
        return Err(GitInspectionError::Parse(
            "only staged hunks can be unstaged".to_string(),
        ));
    }
    git_stdout(
        root.as_ref(),
        &["apply", "--reverse", "--cached", "--unidiff-zero", "-"],
        Some(hunk.patch.as_bytes()),
    )
    .map(|_| ())
}

/// Validate a user-provided commit message before invoking git.
///
/// The editor should surface this validation before running `git commit` so that
/// empty subjects, whitespace-only messages, and NUL-containing payloads are
/// rejected without mutating the repository.
pub fn validate_git_commit_message(message: &str) -> Result<(), GitInspectionError> {
    if message.contains('\0') {
        return Err(GitInspectionError::Parse(
            "commit message cannot contain NUL bytes".to_string(),
        ));
    }
    let Some(subject) = message.lines().find(|line| !line.trim().is_empty()) else {
        return Err(GitInspectionError::Parse(
            "commit message cannot be empty".to_string(),
        ));
    };
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(GitInspectionError::Parse(
            "commit subject cannot be empty".to_string(),
        ));
    }
    if subject.len() > 72 {
        return Err(GitInspectionError::Parse(
            "commit subject should be 72 characters or fewer".to_string(),
        ));
    }
    Ok(())
}

/// Read a single git config value by key (e.g. `"user.name"`).
///
/// Returns `Ok(None)` when the key is not set; errors from git invocation are
/// silently treated as "not configured" since missing config is the normal case
/// for validation purposes.
pub fn get_git_config_value(
    root: impl AsRef<Path>,
    key: &str,
) -> Result<Option<String>, GitInspectionError> {
    match git_stdout(root.as_ref(), &["config", "--get", key], None) {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        // `git config --get` exits non-zero when the key is absent; treat as
        // "not configured" rather than a hard error.
        Err(_) => Ok(None),
    }
}

/// Result of combined commit message and author validation.
///
/// Hard errors block the commit; warn-level messages are advisory only (the
/// conventional-commits prefix lint lives here).
#[derive(Debug, Clone, Default)]
pub struct CommitValidationResult {
    /// Hard errors that MUST be resolved before the commit is allowed.
    pub errors: Vec<String>,
    /// Advisory warnings — surfaced in the UI but do not block the commit.
    pub warnings: Vec<String>,
}

impl CommitValidationResult {
    /// `true` when there are no hard errors (warnings are still permitted).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Conventional-commits type prefixes recognised by the warn-level lint.
const CC_PREFIXES: &[&str] = &[
    "feat", "fix", "refactor", "test", "docs", "build", "chore", "perf", "style", "ci", "revert",
];

/// Validate a commit message together with the author identity from git config.
///
/// Hard errors are returned for:
/// - an empty or blank commit message,
/// - a commit message containing NUL bytes,
/// - missing `user.name` in the local git config,
/// - missing `user.email` in the local git config.
///
/// A single advisory warning is returned when the commit subject does not
/// start with a recognised conventional-commits type prefix.  This warning
/// does **not** block the commit.
pub fn validate_commit_with_author(
    root: impl AsRef<Path>,
    message: &str,
) -> CommitValidationResult {
    let mut result = CommitValidationResult::default();
    let root = root.as_ref();

    // Hard error: empty / blank message.
    let subject = message
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(str::trim)
        .unwrap_or("");
    if subject.is_empty() {
        result
            .errors
            .push("commit message cannot be empty".to_string());
        return result;
    }

    // Hard error: NUL bytes.
    if message.contains('\0') {
        result
            .errors
            .push("commit message cannot contain NUL bytes".to_string());
    }

    // Hard error: missing author name.
    let name = get_git_config_value(root, "user.name")
        .ok()
        .flatten()
        .unwrap_or_default();
    if name.is_empty() {
        result.errors.push(
            "git user.name is not configured; run: git config user.name \"Your Name\"".to_string(),
        );
    }

    // Hard error: missing author email.
    let email = get_git_config_value(root, "user.email")
        .ok()
        .flatten()
        .unwrap_or_default();
    if email.is_empty() {
        result.errors.push(
            "git user.email is not configured; run: git config user.email \"you@example.com\""
                .to_string(),
        );
    }

    // Advisory warning: non-conventional-commits subject prefix.
    let is_conventional = CC_PREFIXES.iter().any(|prefix| {
        subject.starts_with(&format!("{prefix}:"))
            || subject.starts_with(&format!("{prefix}("))
            || subject.starts_with(&format!("{prefix}!"))
    });
    if !is_conventional {
        result.warnings.push(
            "subject does not start with a conventional-commits type prefix \
             (feat/fix/refactor/test/docs/build/chore); \
             this is advisory only and does not block the commit"
                .to_string(),
        );
    }

    result
}

/// Walk up `path` until a component exists on disk, canonicalize it, then
/// re-append the non-existing suffix.  Resolves Windows 8.3 short names
/// (`RUNNER~1` → `runneradmin`) and macOS /var symlinks, even when the leaf
/// has not been created yet.  Cannot be imported from `legion-agent` across
/// the dependency boundary, so it is replicated here.
fn resolve_existing_prefix(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    let mut existing = path;
    let mut suffix: Vec<OsString> = Vec::new();
    loop {
        if existing.symlink_metadata().is_ok() {
            break;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                suffix.push(name.to_os_string());
                existing = parent;
            }
            _ => return Some(path.to_path_buf()),
        }
    }
    let mut resolved = std::fs::canonicalize(existing).ok()?;
    for part in suffix.into_iter().rev() {
        resolved.push(part);
    }
    Some(resolved)
}

/// Strip the Windows UNC prefix `\\?\` from a `PathBuf` (no-op elsewhere).
fn strip_unc_pathbuf(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped.to_string())
    } else {
        p
    }
}

/// Create a new git worktree for `branch` at `worktree_path`.
///
/// Uses `git worktree add <worktree_path> <branch>`.  The branch must already
/// exist; creating the branch alongside the worktree is the caller's
/// responsibility.
///
/// # Validation
///
/// Rejects paths that contain `..` components, absolute paths that fall outside
/// the workspace parent directory, and paths that already exist on disk.
pub fn create_git_worktree(
    root: impl AsRef<Path>,
    branch: &str,
    worktree_path: impl AsRef<Path>,
) -> Result<(), GitInspectionError> {
    let worktree_ref = worktree_path.as_ref();

    // Reject `..` traversal components.
    for component in worktree_ref.components() {
        if component == std::path::Component::ParentDir {
            return Err(GitInspectionError::InvalidInput(
                "worktree path must not contain '..' traversal components".to_string(),
            ));
        }
    }

    // For absolute paths, validate they fall within the workspace parent directory.
    if worktree_ref.is_absolute() {
        let root_ref = root.as_ref();
        let allowed_parent = root_ref.parent().unwrap_or(root_ref);

        // Resolve both sides through their deepest existing ancestor so that
        // Windows 8.3 short names (RUNNER~1 → runneradmin) and macOS /var
        // symlinks are expanded before the containment comparison.  The target
        // worktree may not exist yet, so resolve_existing_prefix canonicalizes
        // the deepest ancestor that does exist and re-appends the rest.
        let norm_parent = resolve_existing_prefix(allowed_parent)
            .map(strip_unc_pathbuf)
            .unwrap_or_else(|| allowed_parent.to_path_buf());
        let norm_worktree = resolve_existing_prefix(worktree_ref)
            .map(strip_unc_pathbuf)
            .unwrap_or_else(|| worktree_ref.to_path_buf());

        if !norm_worktree.starts_with(&norm_parent) {
            return Err(GitInspectionError::InvalidInput(
                "worktree path must reside within the workspace parent directory".to_string(),
            ));
        }
    }

    // Reject paths that already exist on disk to prevent accidental overwrites.
    let resolved = if worktree_ref.is_absolute() {
        worktree_ref.to_path_buf()
    } else {
        root.as_ref()
            .parent()
            .unwrap_or(root.as_ref())
            .join(worktree_ref)
    };
    if resolved.exists() {
        return Err(GitInspectionError::InvalidInput(
            "worktree path already exists on disk".to_string(),
        ));
    }

    let path_str = worktree_ref.to_string_lossy().into_owned();
    git_stdout(root.as_ref(), &["worktree", "add", &path_str, branch], None)?;
    Ok(())
}

/// Commit the current index with the supplied message.
///
/// Git inherits the caller's authentication environment unchanged, so SSH agent
/// sockets (`SSH_AUTH_SOCK`) and credential helpers remain available for any
/// remote hooks or commit-related integrations that rely on them.
pub fn commit_git_changes(
    root: impl AsRef<Path>,
    message: &str,
) -> Result<String, GitInspectionError> {
    validate_git_commit_message(message)?;
    git_stdout(
        root.as_ref(),
        &["commit", "--file", "-", "--cleanup=verbatim"],
        Some(message.as_bytes()),
    )
}

/// Push the current branch to the named remote.
///
/// The git subprocess inherits SSH agent and credential-helper configuration
/// from the caller, so the desktop shell can rely on the user's existing auth
/// setup without reimplementing credential prompts.
pub fn push_git_remote(
    root: impl AsRef<Path>,
    remote: &str,
    branch: &str,
) -> Result<String, GitInspectionError> {
    let args = vec!["push".to_string(), remote.to_string(), branch.to_string()];
    git_stdout_owned(root.as_ref(), &args, None)
}

/// Fetch from the named remote without mutating the working tree.
pub fn fetch_git_remote(
    root: impl AsRef<Path>,
    remote: &str,
) -> Result<String, GitInspectionError> {
    let args = vec!["fetch".to_string(), remote.to_string()];
    git_stdout_owned(root.as_ref(), &args, None)
}

/// Pull the named branch from the named remote.
pub fn pull_git_remote(
    root: impl AsRef<Path>,
    remote: &str,
    branch: &str,
) -> Result<String, GitInspectionError> {
    let args = vec!["pull".to_string(), remote.to_string(), branch.to_string()];
    git_stdout_owned(root.as_ref(), &args, None)
}

/// Which side of a conflict to keep when resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitConflictChoice {
    /// Keep the current (ours) side.
    AcceptCurrent,
    /// Keep the incoming (theirs) side.
    AcceptIncoming,
}

/// Discover the Git repository root for the given workspace root.
pub fn git_repository_root(root: impl AsRef<Path>) -> Result<PathBuf, GitInspectionError> {
    let root = root.as_ref();
    let out = git_stdout(root, &["rev-parse", "--show-toplevel"], None)?;
    Ok(PathBuf::from(out.trim()))
}

/// Resolve one conflicted file by keeping the chosen side for every conflict block.
pub fn resolve_git_conflict(
    root: impl AsRef<Path>,
    path: impl AsRef<Path>,
    choice: GitConflictChoice,
) -> Result<(), GitInspectionError> {
    let root = root.as_ref();
    let repository_root =
        PathBuf::from(git_stdout(root, &["rev-parse", "--show-toplevel"], None)?.trim());
    let path = path.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repository_root.join(path)
    };
    let canonical_target =
        canonicalize_path_or_existing_parent(&absolute).unwrap_or_else(|| absolute.clone());
    let canonical_repo = canonicalize_path_or_existing_parent(&repository_root)
        .unwrap_or_else(|| repository_root.clone());
    if !canonical_target.starts_with(&canonical_repo) {
        return Err(GitInspectionError::Parse(format!(
            "path `{}` is outside repository root boundary",
            absolute.display()
        )));
    }
    let status_entries = git_status_entries(root)?;
    let relative_path = match canonical_target.strip_prefix(&canonical_repo) {
        Ok(p) => p.to_string_lossy().replace('\\', "/"),
        Err(_) => absolute.to_string_lossy().replace('\\', "/"),
    };
    let relative_path = relative_path.trim_start_matches('/');
    let status_code = status_entries.get(relative_path);
    let is_unmerged = match status_code {
        Some(code) => {
            code.bytes().any(|b| b == b'U')
                || matches!(
                    code.as_str(),
                    "AA" | "DD" | "AU" | "UA" | "DU" | "UD" | "UU"
                )
        }
        None => false,
    };
    if !is_unmerged {
        return Err(GitInspectionError::Parse(format!(
            "path `{}` is not in an unmerged conflict state (status: {})",
            relative_path,
            status_code.map_or("??", |v| v.as_str())
        )));
    }
    let text = std::fs::read_to_string(&absolute).map_err(|source| GitInspectionError::Launch {
        command: "read conflict file".to_string(),
        source,
    })?;
    let ours_blob = git_stdout(root, &["show", &format!(":2:{relative_path}")], None).ok();
    let theirs_blob = git_stdout(root, &["show", &format!(":3:{relative_path}")], None).ok();
    let stage_blobs = ours_blob.as_deref().zip(theirs_blob.as_deref());
    let resolved = parse_and_resolve_conflict(&text, choice, stage_blobs)?;
    std::fs::write(&absolute, resolved).map_err(|source| GitInspectionError::Launch {
        command: "write resolved file".to_string(),
        source,
    })?;
    git_stdout(
        root,
        &["add", "--", absolute.to_string_lossy().as_ref()],
        None,
    )?;
    Ok(())
}

fn parse_and_resolve_conflict(
    text: &str,
    choice: GitConflictChoice,
    stage_blobs: Option<(&str, &str)>,
) -> Result<String, GitInspectionError> {
    let mut result = String::new();
    let segments: Vec<&str> = text.split_inclusive('\n').collect();
    let mut conflict_block_found = false;
    let mut index = 0;

    while let Some(segment) = segments.get(index).copied() {
        let trimmed = segment.trim_end_matches(['\r', '\n']);
        let Some(marker_len) = conflict_marker_length(trimmed, '<') else {
            result.push_str(segment);
            index += 1;
            continue;
        };

        conflict_block_found = true;
        let block_start = index + 1;
        let mut end_indices = Vec::new();
        let mut scan = block_start;
        while let Some(inner) = segments.get(scan).copied() {
            let inner_trimmed = inner.trim_end_matches(['\r', '\n']);
            if scan > block_start && conflict_marker_length(inner_trimmed, '<').is_some() {
                if end_indices.is_empty() {
                    return Err(GitInspectionError::Parse(
                        "ambiguous conflict markers: nested opening marker".to_string(),
                    ));
                }
                break;
            }
            if is_conflict_end(inner_trimmed, marker_len) {
                end_indices.push(scan);
            }
            scan += 1;
        }
        let [end_index] = end_indices.as_slice() else {
            return Err(GitInspectionError::Parse(
                "malformed conflict markers: unbalanced block".to_string(),
            ));
        };
        let end_index = *end_index;
        if let Some((ours_blob, theirs_blob)) = stage_blobs {
            let raw_block = segments[index..=end_index].concat();
            if ours_blob.contains(&raw_block) && theirs_blob.contains(&raw_block) {
                return Err(GitInspectionError::Parse(
                    "ambiguous conflict markers: literal marker block present in both stages"
                        .to_string(),
                ));
            }
        }

        let separators: Vec<usize> = (block_start..end_index)
            .filter(|candidate| {
                segments
                    .get(*candidate)
                    .map(|line| {
                        is_conflict_separator(line.trim_end_matches(['\r', '\n']), marker_len)
                    })
                    .unwrap_or(false)
            })
            .collect();
        let Some(mut separator_index) = separators.first().copied() else {
            return Err(GitInspectionError::Parse(
                "malformed conflict markers: unbalanced block".to_string(),
            ));
        };

        let mut current_end = separator_index;
        if let Some(base_index) = (block_start..end_index).find(|candidate| {
            segments
                .get(*candidate)
                .map(|line| is_conflict_base(line.trim_end_matches(['\r', '\n']), marker_len))
                .unwrap_or(false)
                && separators.iter().any(|separator| *separator > *candidate)
        }) {
            if matches!(choice, GitConflictChoice::AcceptCurrent) {
                return Err(GitInspectionError::Parse(
                    "ambiguous conflict markers: base marker on current side".to_string(),
                ));
            }
            current_end = base_index;
            let separators_after_base: Vec<usize> = separators
                .iter()
                .copied()
                .filter(|candidate| *candidate > base_index)
                .collect();
            let [candidate] = separators_after_base.as_slice() else {
                return Err(GitInspectionError::Parse(
                    "ambiguous conflict markers: multiple separator lines".to_string(),
                ));
            };
            separator_index = *candidate;
        } else if separators.len() != 1 {
            return Err(GitInspectionError::Parse(
                "ambiguous conflict markers: multiple separator lines".to_string(),
            ));
        }

        match choice {
            GitConflictChoice::AcceptCurrent => {
                for l in &segments[block_start..current_end] {
                    result.push_str(l);
                }
            }
            GitConflictChoice::AcceptIncoming => {
                for l in &segments[(separator_index + 1)..end_index] {
                    result.push_str(l);
                }
            }
        }
        index = end_index + 1;
    }

    if !conflict_block_found {
        return Err(GitInspectionError::Parse(
            "no conflict markers found in file".to_string(),
        ));
    }
    Ok(result)
}

fn conflict_marker_length(line: &str, marker_char: char) -> Option<usize> {
    let prefix_len = line.chars().take_while(|c| *c == marker_char).count();
    if prefix_len >= 7 && line.as_bytes().get(prefix_len).copied() == Some(b' ') {
        Some(prefix_len)
    } else {
        None
    }
}

fn is_conflict_separator(line: &str, expected_len: usize) -> bool {
    line.len() == expected_len && line.chars().all(|c| c == '=')
}

fn is_conflict_base(line: &str, expected_len: usize) -> bool {
    let prefix_len = line.chars().take_while(|c| *c == '|').count();
    prefix_len == expected_len && line.as_bytes().get(prefix_len).copied() == Some(b' ')
}

fn is_conflict_end(line: &str, expected_len: usize) -> bool {
    let prefix_len = line.chars().take_while(|c| *c == '>').count();
    prefix_len == expected_len && line.as_bytes().get(prefix_len).copied() == Some(b' ')
}

fn git_stdout(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<String, GitInspectionError> {
    let command_label = args.join(" ");
    let mut command = Command::new("git");
    command.current_dir(root).args(args);
    let output = if let Some(input) = input {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| GitInspectionError::Launch {
                command: command_label.clone(),
                source,
            })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            GitInspectionError::Parse("git stdin pipe was not available".to_string())
        })?;
        stdin
            .write_all(input)
            .map_err(|source| GitInspectionError::Launch {
                command: command_label.clone(),
                source,
            })?;
        drop(stdin);
        child
            .wait_with_output()
            .map_err(|source| GitInspectionError::Launch {
                command: command_label.clone(),
                source,
            })?
    } else {
        command
            .output()
            .map_err(|source| GitInspectionError::Launch {
                command: command_label.clone(),
                source,
            })?
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(GitInspectionError::CommandFailed {
            command: command_label,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn git_stdout_owned(
    root: &Path,
    args: &[String],
    input: Option<&[u8]>,
) -> Result<String, GitInspectionError> {
    let borrowed_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_stdout(root, &borrowed_args, input)
}

fn git_status_entries(root: &Path) -> Result<HashMap<String, String>, GitInspectionError> {
    let output = git_stdout(root, &["status", "--porcelain=v1", "-z"], None)?;
    Ok(parse_git_porcelain_status(&output))
}

/// Parse the NUL-separated `git status --porcelain=v1 -z` payload into a
/// path -> two-character status-code map.
///
/// For rename (`R`) and copy (`C`) records the porcelain `-z` format splits the
/// path into two NUL-delimited fields; the trailing field is consumed and used
/// as the map key (preserving the long-standing CLI parser behavior).
fn parse_git_porcelain_status(output: &str) -> HashMap<String, String> {
    let entries = output
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;
    let mut status = HashMap::new();
    while index < entries.len() {
        let entry = entries[index];
        if entry.len() >= 4 {
            let code = entry[0..2].to_string();
            let mut path = entry[3..].to_string();
            if matches!(code.as_bytes().first(), Some(b'R' | b'C')) && index + 1 < entries.len() {
                index += 1;
                path = entries[index].to_string();
            }
            status.insert(path, code);
        }
        index += 1;
    }
    status
}

fn git_status_entries_gix(root: &Path) -> Result<HashMap<String, String>, GitInspectionError> {
    // The previous implementation scraped paths and status codes out of the
    // `gix` status item `Debug` representation, which is an unstable format with
    // no cross-version contract (it silently breaks on rename/copy/delete and on
    // any gix upgrade). Until a typed `gix` status mapping exists, defer to the
    // authoritative CLI porcelain parser (mirroring `git_blame_lines_gix`).
    git_status_entries(root)
}

fn git_blame_lines_gix(
    root: &Path,
    path: &str,
    limit: usize,
) -> Result<Vec<ProjectGitBlameLine>, GitInspectionError> {
    git_blame_lines(root, path, limit)
}

fn git_numstat(
    root: &Path,
    staged: bool,
) -> Result<HashMap<String, (u32, u32)>, GitInspectionError> {
    let output = if staged {
        git_stdout(root, &["diff", "--cached", "--numstat", "--"], None)?
    } else {
        git_stdout(root, &["diff", "--numstat", "--"], None)?
    };
    let mut stats = HashMap::new();
    for line in output.lines() {
        let mut parts = line.split('\t');
        let inserted = parts.next().and_then(parse_numstat_count).unwrap_or(0);
        let deleted = parts.next().and_then(parse_numstat_count).unwrap_or(0);
        if let Some(path) = parts.next() {
            stats.insert(path.to_string(), (inserted, deleted));
        }
    }
    Ok(stats)
}

fn parse_numstat_count(value: &str) -> Option<u32> {
    if value == "-" {
        Some(0)
    } else {
        value.parse().ok()
    }
}

fn git_diff_hunks(
    root: &Path,
    stage: GitHunkStage,
    limit: usize,
) -> Result<Vec<ProjectGitHunk>, GitInspectionError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let output = match stage {
        GitHunkStage::Unstaged => {
            git_stdout(root, &["diff", "--unified=0", "--no-ext-diff", "--"], None)?
        }
        GitHunkStage::Staged => git_stdout(
            root,
            &["diff", "--cached", "--unified=0", "--no-ext-diff", "--"],
            None,
        )?,
    };
    parse_diff_hunks(&output, stage, limit)
}

fn parse_diff_hunks(
    patch: &str,
    stage: GitHunkStage,
    limit: usize,
) -> Result<Vec<ProjectGitHunk>, GitInspectionError> {
    let mut hunks = Vec::new();
    let mut file_header = Vec::<String>::new();
    let mut current_path = String::new();
    let mut hunk_lines = Vec::<String>::new();

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            flush_git_hunk(
                &mut hunks,
                stage,
                &current_path,
                &file_header,
                &mut hunk_lines,
                limit,
            )?;
            file_header.clear();
            file_header.push(line.to_string());
            current_path = parse_diff_git_path(line).unwrap_or_default();
        } else if line.starts_with("@@ ") {
            flush_git_hunk(
                &mut hunks,
                stage,
                &current_path,
                &file_header,
                &mut hunk_lines,
                limit,
            )?;
            hunk_lines.push(line.to_string());
        } else if hunk_lines.is_empty() {
            if let Some(path) = parse_diff_plus_path(line) {
                current_path = path;
            }
            if !file_header.is_empty() {
                file_header.push(line.to_string());
            }
        } else {
            hunk_lines.push(line.to_string());
        }
        if hunks.len() >= limit {
            return Ok(hunks);
        }
    }

    flush_git_hunk(
        &mut hunks,
        stage,
        &current_path,
        &file_header,
        &mut hunk_lines,
        limit,
    )?;
    Ok(hunks)
}

fn flush_git_hunk(
    hunks: &mut Vec<ProjectGitHunk>,
    stage: GitHunkStage,
    path: &str,
    file_header: &[String],
    hunk_lines: &mut Vec<String>,
    limit: usize,
) -> Result<(), GitInspectionError> {
    if hunk_lines.is_empty() || hunks.len() >= limit {
        hunk_lines.clear();
        return Ok(());
    }
    let header = hunk_lines[0].clone();
    let (old_start, old_lines, new_start, new_lines, context) = parse_hunk_header(&header)?;
    let added_lines = hunk_lines
        .iter()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count() as u32;
    let deleted_lines = hunk_lines
        .iter()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count() as u32;
    let mut patch = String::new();
    for line in file_header {
        patch.push_str(line);
        patch.push('\n');
    }
    for line in hunk_lines.iter() {
        patch.push_str(line);
        patch.push('\n');
    }
    let hunk_id = format!(
        "git-hunk:{:032x}",
        stable_hash(&format!("{stage:?}:{path}:{header}:{}", hunks.len()))
    );
    hunks.push(ProjectGitHunk {
        hunk_id,
        path: path.to_string(),
        stage,
        header,
        old_start,
        old_lines,
        new_start,
        new_lines,
        added_lines,
        deleted_lines,
        context,
        patch,
    });
    hunk_lines.clear();
    Ok(())
}

fn parse_diff_git_path(line: &str) -> Option<String> {
    line.split_whitespace()
        .nth(3)
        .map(strip_git_side_prefix)
        .filter(|path| path != "/dev/null")
}

fn parse_diff_plus_path(line: &str) -> Option<String> {
    line.strip_prefix("+++ ")
        .map(strip_git_side_prefix)
        .filter(|path| path != "/dev/null")
}

fn strip_git_side_prefix(path: &str) -> String {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

fn parse_hunk_header(
    header: &str,
) -> Result<(u32, u32, u32, u32, Option<String>), GitInspectionError> {
    let mut sections = header.split("@@");
    sections.next();
    let ranges = sections
        .next()
        .ok_or_else(|| GitInspectionError::Parse(format!("invalid hunk header `{header}`")))?
        .trim();
    let context = sections
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let mut tokens = ranges.split_whitespace();
    let old = tokens
        .next()
        .ok_or_else(|| GitInspectionError::Parse(format!("missing old range `{header}`")))?;
    let new = tokens
        .next()
        .ok_or_else(|| GitInspectionError::Parse(format!("missing new range `{header}`")))?;
    let (old_start, old_lines) = parse_hunk_range(old, '-')?;
    let (new_start, new_lines) = parse_hunk_range(new, '+')?;
    Ok((old_start, old_lines, new_start, new_lines, context))
}

fn parse_hunk_range(token: &str, prefix: char) -> Result<(u32, u32), GitInspectionError> {
    let token = token
        .strip_prefix(prefix)
        .ok_or_else(|| GitInspectionError::Parse(format!("invalid hunk range `{token}`")))?;
    let mut parts = token.split(',');
    let start = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| GitInspectionError::Parse(format!("invalid hunk start `{token}`")))?;
    let lines = parts
        .next()
        .map_or(Some(1), |value| value.parse::<u32>().ok())
        .ok_or_else(|| GitInspectionError::Parse(format!("invalid hunk length `{token}`")))?;
    Ok((start, lines))
}

fn git_changed_files(
    root: &Path,
    status_entries: HashMap<String, String>,
    unstaged_numstat: &HashMap<String, (u32, u32)>,
    staged_numstat: &HashMap<String, (u32, u32)>,
    hunks: &[ProjectGitHunk],
    conflicts: &[ProjectGitConflict],
    max_file_bytes_for_syntactic_diff: u64,
) -> Vec<ProjectGitChangedFile> {
    let mut paths = status_entries.keys().cloned().collect::<HashSet<_>>();
    paths.extend(unstaged_numstat.keys().cloned());
    paths.extend(staged_numstat.keys().cloned());
    paths.extend(hunks.iter().map(|hunk| hunk.path.clone()));
    paths.extend(conflicts.iter().map(|conflict| conflict.path.clone()));
    let conflict_paths = conflicts
        .iter()
        .map(|conflict| conflict.path.as_str())
        .collect::<HashSet<_>>();
    let mut files = paths
        .into_iter()
        .map(|path| {
            let (unstaged_inserted, unstaged_deleted) =
                unstaged_numstat.get(&path).copied().unwrap_or_default();
            let (staged_inserted, staged_deleted) =
                staged_numstat.get(&path).copied().unwrap_or_default();
            let unstaged_hunk_count = hunks
                .iter()
                .filter(|hunk| hunk.path == path && hunk.stage == GitHunkStage::Unstaged)
                .count();
            let staged_hunk_count = hunks
                .iter()
                .filter(|hunk| hunk.path == path && hunk.stage == GitHunkStage::Staged)
                .count();
            let (diff_strategy, fallback_reason) =
                git_diff_strategy(root, &path, max_file_bytes_for_syntactic_diff);
            ProjectGitChangedFile {
                status: status_entries
                    .get(&path)
                    .cloned()
                    .unwrap_or_else(|| "??".to_string()),
                inserted_lines: unstaged_inserted.saturating_add(staged_inserted),
                deleted_lines: unstaged_deleted.saturating_add(staged_deleted),
                unstaged_hunk_count,
                staged_hunk_count,
                stageable: unstaged_hunk_count + staged_hunk_count > 0,
                diff_strategy,
                fallback_reason,
                conflict: conflict_paths.contains(path.as_str()),
                path,
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn git_diff_strategy(
    root: &Path,
    path: &str,
    max_file_bytes_for_syntactic_diff: u64,
) -> (GitDiffStrategy, Option<String>) {
    let file_path = root.join(path);
    let byte_len = fs_metadata_len(&file_path);
    if byte_len > max_file_bytes_for_syntactic_diff {
        return (
            GitDiffStrategy::LineFallback,
            Some(format!(
                "file_size_exceeds_syntactic_threshold:{byte_len}>{max_file_bytes_for_syntactic_diff}"
            )),
        );
    }
    if syntax_diff_supported(path) {
        (GitDiffStrategy::Syntactic, None)
    } else {
        (
            GitDiffStrategy::LineFallback,
            Some("unsupported_syntax_for_syntactic_diff".to_string()),
        )
    }
}

fn fs_metadata_len(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn syntax_diff_supported(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some(
            "rs" | "js"
                | "jsx"
                | "ts"
                | "tsx"
                | "py"
                | "go"
                | "java"
                | "c"
                | "cc"
                | "cpp"
                | "h"
                | "hpp"
                | "cs"
                | "kt"
                | "swift"
                | "toml"
                | "json"
                | "yaml"
                | "yml"
        )
    )
}

fn git_blame_lines(
    root: &Path,
    path: &str,
    limit: usize,
) -> Result<Vec<ProjectGitBlameLine>, GitInspectionError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let output = match git_stdout(root, &["blame", "--line-porcelain", "--", path], None) {
        Ok(output) => output,
        Err(GitInspectionError::CommandFailed { .. }) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut rows = Vec::new();
    let mut commit_short = String::new();
    let mut line_number = 0;
    let mut author = String::new();
    let mut summary = String::new();
    for line in output.lines() {
        if rows.len() >= limit {
            break;
        }
        if let Some(preview) = line.strip_prefix('\t') {
            rows.push(ProjectGitBlameLine {
                path: path.to_string(),
                line_number,
                commit_short: commit_short.clone(),
                author: author.clone(),
                summary: summary.clone(),
                line_preview: preview.chars().take(160).collect(),
            });
        } else if is_blame_header(line) {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            commit_short = parts[0].chars().take(12).collect();
            line_number = parts
                .get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            author.clear();
            summary.clear();
        } else if let Some(value) = line.strip_prefix("author ") {
            author = value.to_string();
        } else if let Some(value) = line.strip_prefix("summary ") {
            summary = value.to_string();
        }
    }
    Ok(rows)
}

fn is_blame_header(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(hash) = parts.next() else {
        return false;
    };
    hash.len() >= 8
        && hash.chars().all(|ch| ch.is_ascii_hexdigit())
        && parts.next().is_some()
        && parts.next().is_some()
}

fn git_commits(root: &Path, limit: usize) -> Result<Vec<ProjectGitCommit>, GitInspectionError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let limit_arg = format!("-n{limit}");
    let output = match git_stdout(
        root,
        &[
            "log",
            "--date=short",
            "--decorate=short",
            "--pretty=format:%H%x1f%h%x1f%an%x1f%ad%x1f%s%x1f%P%x1f%D",
            &limit_arg,
        ],
        None,
    ) {
        Ok(output) => output,
        Err(GitInspectionError::CommandFailed { .. }) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut commits = output
        .lines()
        .filter_map(|line| {
            let parts = line.split('\x1f').collect::<Vec<_>>();
            if parts.len() < 7 {
                return None;
            }
            Some(ProjectGitCommit {
                hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
                summary: parts[4].to_string(),
                parent_count: parts[5].split_whitespace().count(),
                refs: parts[6]
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    commits.truncate(limit);
    Ok(commits)
}

fn git_conflicts<'a>(
    root: &Path,
    paths: impl Iterator<Item = &'a String>,
) -> Result<Vec<ProjectGitConflict>, GitInspectionError> {
    let mut conflicts = Vec::new();
    for path in paths {
        let file_path = root.join(path);
        let Ok(text) = std::fs::read_to_string(&file_path) else {
            continue;
        };
        let marker_count = text
            .lines()
            .filter(|line| {
                line.starts_with("<<<<<<<")
                    || line.starts_with("=======")
                    || line.starts_with(">>>>>>>")
            })
            .count();
        if marker_count > 0 {
            conflicts.push(ProjectGitConflict {
                path: path.clone(),
                marker_count,
                actions: vec![
                    "open_conflict_resolution".to_string(),
                    "accept_current".to_string(),
                    "accept_incoming".to_string(),
                ],
            });
        }
    }
    conflicts.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(conflicts)
}

fn relative_git_path(root: &Path, path: &Path) -> Option<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };

    let mut roots = Vec::new();
    push_git_path_candidate(&mut roots, root.to_path_buf());
    if let Some(canonical_root) = canonicalize_path_or_existing_parent(root) {
        push_git_path_candidate(&mut roots, canonical_root);
    }

    let mut absolutes = Vec::new();
    push_git_path_candidate(&mut absolutes, absolute.clone());
    if let Some(canonical_absolute) = canonicalize_path_or_existing_parent(&absolute) {
        push_git_path_candidate(&mut absolutes, canonical_absolute);
    }

    for candidate in &absolutes {
        for root_candidate in &roots {
            if let Ok(relative) = candidate.strip_prefix(root_candidate) {
                return Some(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    None
}

fn push_git_path_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    let path = strip_windows_verbatim_prefix(path);
    if !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn canonicalize_path_or_existing_parent(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(canonical);
    }

    let mut missing_suffix = Vec::new();
    let mut cursor = path;
    loop {
        missing_suffix.push(cursor.file_name()?.to_os_string());
        cursor = cursor.parent()?;
        if let Ok(mut canonical_parent) = std::fs::canonicalize(cursor) {
            for component in missing_suffix.iter().rev() {
                canonical_parent.push(component);
            }
            return Some(canonical_parent);
        }
    }
}

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(stripped) = text.strip_prefix("\\\\?\\UNC\\") {
            return PathBuf::from(format!("\\\\{stripped}"));
        }
        if let Some(stripped) = text.strip_prefix("\\\\?\\") {
            return PathBuf::from(stripped);
        }
    }

    path
}

/// Metadata returned with a successful workspace text open.
#[derive(Debug, Clone)]
pub struct OpenedFileText {
    /// File identity captured at open time.
    pub identity: FileIdentity,
    /// UTF-8 text loaded from disk, or an explicit safe-new-file empty payload.
    pub text: String,
    /// Protocol fingerprint captured for save preconditions.
    pub fingerprint: ProtocolFileFingerprint,
    /// File content version captured at open time.
    pub file_content_version: FileContentVersion,
    /// Workspace generation captured at open time.
    pub workspace_generation: WorkspaceGeneration,
    /// Modified timestamp captured at open time if available.
    pub modified_at: Option<TimestampMillis>,
    /// File length captured at open time if available.
    pub file_length: Option<u64>,
    /// Whether this open represented explicit create intent for a new file.
    pub is_new_file: bool,
}

/// Proposal-context save request accepted by the workspace write pipeline.
#[derive(Debug, Clone)]
pub struct WorkspaceSaveRequest {
    /// Workspace being mutated.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing this write.
    pub proposal_id: ProposalId,
    /// Principal requesting the save.
    pub principal: PrincipalId,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// Expected file identity.
    pub file_id: FileId,
    /// Target path.
    pub path: CanonicalPath,
    /// Expected disk fingerprint.
    pub expected_fingerprint: ProtocolFileFingerprint,
    /// Expected file content version.
    pub expected_file_content_version: FileContentVersion,
    /// Expected workspace generation.
    pub expected_workspace_generation: WorkspaceGeneration,
    /// Buffer version being saved.
    pub buffer_version: BufferVersion,
    /// Snapshot being saved.
    pub snapshot_id: SnapshotId,
    /// Payload byte length.
    pub payload_byte_len: u64,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking save/proposal/workspace events.
    pub causality_id: CausalityId,
    /// UTF-8 text payload to write.
    pub text: String,
}

/// Proposal-context create-file request accepted by workspace VFS authority.
#[derive(Debug, Clone)]
pub struct WorkspaceCreateFileRequest {
    /// Workspace being mutated.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing this mutation.
    pub proposal_id: ProposalId,
    /// Principal requesting the mutation.
    pub principal: PrincipalId,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// Destination path.
    pub path: CanonicalPath,
    /// Expected workspace generation.
    pub expected_workspace_generation: WorkspaceGeneration,
    /// Initial UTF-8 file content.
    pub initial_content: String,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking proposal/workspace events.
    pub causality_id: CausalityId,
}

/// Proposal-context delete-file request accepted by workspace VFS authority.
#[derive(Debug, Clone)]
pub struct WorkspaceDeleteFileRequest {
    /// Workspace being mutated.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing this mutation.
    pub proposal_id: ProposalId,
    /// Principal requesting the mutation.
    pub principal: PrincipalId,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// File identity being deleted.
    pub file: FileIdentity,
    /// Expected disk fingerprint.
    pub expected_fingerprint: ProtocolFileFingerprint,
    /// Expected file content version.
    pub expected_file_content_version: FileContentVersion,
    /// Expected workspace generation.
    pub expected_workspace_generation: WorkspaceGeneration,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking proposal/workspace events.
    pub causality_id: CausalityId,
}

/// Proposal-context rename-file request accepted by workspace VFS authority.
#[derive(Debug, Clone)]
pub struct WorkspaceRenameFileRequest {
    /// Workspace being mutated.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing this mutation.
    pub proposal_id: ProposalId,
    /// Principal requesting the mutation.
    pub principal: PrincipalId,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// File identity being renamed.
    pub file: FileIdentity,
    /// Destination path.
    pub destination: CanonicalPath,
    /// Expected disk fingerprint.
    pub expected_fingerprint: ProtocolFileFingerprint,
    /// Expected file content version.
    pub expected_file_content_version: FileContentVersion,
    /// Expected workspace generation.
    pub expected_workspace_generation: WorkspaceGeneration,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking proposal/workspace events.
    pub causality_id: CausalityId,
}

/// Workspace-authorized rollback checkpoint target for accepted file mutations.
#[derive(Debug, Clone)]
pub enum WorkspaceMutationRollbackTarget {
    /// A create-file proposal will create this path; rollback removes it if audit fails.
    CreatedFile {
        /// Path expected to be absent before mutation.
        path: CanonicalPath,
    },
    /// A delete-file proposal will remove this file; rollback restores its text snapshot.
    DeletedFile {
        /// File identity expected before mutation.
        file: FileIdentity,
    },
    /// A rename-file proposal will move this file; rollback moves destination back to source.
    RenamedFile {
        /// Source file identity expected before mutation.
        file: FileIdentity,
        /// Destination path expected to be absent before mutation.
        destination: CanonicalPath,
    },
    /// A save-file proposal will replace this file; rollback restores its text snapshot.
    SavedFile {
        /// File identity expected before mutation.
        file: FileIdentity,
    },
}

/// Request to capture a workspace-owned rollback checkpoint before a file mutation.
#[derive(Debug, Clone)]
pub struct WorkspaceMutationRollbackCheckpointRequest {
    /// Workspace being protected.
    pub workspace_id: WorkspaceId,
    /// Proposal authorizing the mutation that may need rollback.
    pub proposal_id: ProposalId,
    /// Principal requesting the mutation.
    pub principal: PrincipalId,
    /// Required capability for the mutation.
    pub required_capability: CapabilityId,
    /// Mutation target that needs rollback material.
    pub target: WorkspaceMutationRollbackTarget,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking proposal/workspace events.
    pub causality_id: CausalityId,
}

/// Workspace-owned rollback material for accepted file mutations.
#[derive(Debug, Clone)]
pub enum WorkspaceMutationRollbackCheckpoint {
    /// Roll back a create by removing the created path.
    CreatedFile {
        /// Canonical path to remove.
        path: CanonicalPath,
    },
    /// Roll back a delete by restoring the captured UTF-8 text.
    DeletedFile {
        /// File identity to restore.
        file: FileIdentity,
        /// Pre-mutation UTF-8 text snapshot.
        text: String,
    },
    /// Roll back a rename by moving the destination back to the source identity path.
    RenamedFile {
        /// Original source file identity.
        file: FileIdentity,
        /// Canonical destination path after mutation.
        destination: CanonicalPath,
    },
    /// Roll back a save by restoring the captured UTF-8 text.
    SavedFile {
        /// File identity to restore.
        file: FileIdentity,
        /// Pre-mutation UTF-8 text snapshot.
        text: String,
    },
}

/// Result of capturing workspace rollback material.
#[allow(clippy::result_large_err)]
pub type WorkspaceMutationRollbackCheckpointResult =
    Result<WorkspaceMutationRollbackCheckpoint, ProposalResponse>;

/// Request to compensate an audit-failed workspace file mutation.
#[derive(Debug, Clone)]
pub struct WorkspaceMutationRollbackRequest {
    /// Workspace being compensated.
    pub workspace_id: WorkspaceId,
    /// Proposal whose mutation is being rolled back.
    pub proposal_id: ProposalId,
    /// Principal requesting the original mutation.
    pub principal: PrincipalId,
    /// Required capability for the original mutation.
    pub required_capability: CapabilityId,
    /// Workspace-owned checkpoint captured before mutation.
    pub checkpoint: WorkspaceMutationRollbackCheckpoint,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id linking proposal/workspace events.
    pub causality_id: CausalityId,
}

/// Successful rollback compensation metadata.
#[derive(Debug, Clone)]
pub struct WorkspaceMutationRollbackApplied {
    /// Updated workspace generation after compensation.
    pub workspace_generation: WorkspaceGeneration,
}

/// Workspace rollback compensation result preserving typed proposal responses.
#[allow(clippy::result_large_err)]
pub type WorkspaceMutationRollbackResult =
    Result<WorkspaceMutationRollbackApplied, ProposalResponse>;

/// Successful closed-file workspace mutation metadata.
#[derive(Debug, Clone)]
pub struct WorkspaceFileMutationApplied {
    /// Updated or affected file identity.
    pub identity: FileIdentity,
    /// Disk fingerprint after mutation, when the file still exists.
    pub fingerprint: Option<ProtocolFileFingerprint>,
    /// Updated or affected file content version.
    pub file_content_version: FileContentVersion,
    /// Updated workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Proposal response for applied lifecycle.
    pub response: ProposalResponse,
}

/// Closed-file workspace mutation result preserving typed proposal responses.
#[allow(clippy::result_large_err)]
pub type WorkspaceFileMutationResult = Result<WorkspaceFileMutationApplied, ProposalResponse>;

/// Non-atomic fallback policy for workspace saves.
///
/// Track 3 intentionally exposes only the fail-closed policy. Any future fallback variant must add
/// explicit security approval, immediate fingerprint re-verification, visible fallback response,
/// and event/audit hook placeholders before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonAtomicSaveFallbackPolicy {
    /// Fail closed when atomic replacement fails.
    Disabled,
}

/// Successful workspace save metadata.
#[derive(Debug, Clone)]
pub struct WorkspaceSaveApplied {
    /// Updated file identity.
    pub identity: FileIdentity,
    /// New disk fingerprint.
    pub fingerprint: ProtocolFileFingerprint,
    /// Updated file content version.
    pub file_content_version: FileContentVersion,
    /// Updated workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Updated modified timestamp if available.
    pub modified_at: Option<TimestampMillis>,
    /// Updated file length if available.
    pub file_length: Option<u64>,
    /// Whether a non-atomic fallback path was used.
    pub used_non_atomic_fallback: bool,
    /// Visible fallback status for audit/UI surfaces.
    pub fallback_status: Option<String>,
    /// Proposal response for applied lifecycle.
    pub response: ProposalResponse,
}

/// Workspace save result preserving typed stale/conflict/denial/failure responses.
#[allow(clippy::result_large_err)]
pub type WorkspaceSaveResult = Result<WorkspaceSaveApplied, ProposalResponse>;

#[derive(Debug, Clone)]
struct FileFingerprint {
    size: Option<u64>,
    modified: Option<TimestampMillis>,
    hash: Option<String>,
    read_only: bool,
}

impl FileFingerprint {
    fn from_path(path: &Path, fs: &ProjectFilesystem) -> Result<Self, WorkspaceError> {
        let metadata = fs.read_metadata(path).map_err(WorkspaceError::Platform)?;
        Self::from_metadata(path, fs, &metadata)
    }

    fn from_metadata(
        path: &Path,
        fs: &ProjectFilesystem,
        metadata: &FileSystemMetadata,
    ) -> Result<Self, WorkspaceError> {
        let size = metadata.length;
        let modified = metadata.modified_at.map(TimestampMillis);
        let hash = if metadata.is_file() && size <= LARGE_FILE_BYTES {
            let fingerprint = fs
                .read_fingerprint(path)
                .map_err(WorkspaceError::Platform)?;
            if fingerprint.length != Some(size) || fingerprint.modified_at != metadata.modified_at {
                return Err(WorkspaceError::Platform(
                    PlatformError::MetadataInconsistent {
                        operation: "workspace fingerprint read".to_string(),
                        path: path.to_path_buf(),
                        details: format!(
                            "metadata={metadata:?}, fingerprint_length={:?}, fingerprint_modified={:?}",
                            fingerprint.length, fingerprint.modified_at
                        ),
                    },
                ));
            }
            Some(fingerprint.stable_hash.ok_or_else(|| {
                WorkspaceError::Platform(PlatformError::MetadataInconsistent {
                    operation: "workspace fingerprint read".to_string(),
                    path: path.to_path_buf(),
                    details: "regular file fingerprint did not include stable hash".to_string(),
                })
            })?)
        } else {
            None
        };

        Ok(Self {
            size: Some(size),
            modified,
            hash,
            read_only: metadata.read_only,
        })
    }

    fn from_dir() -> Self {
        Self {
            size: None,
            modified: None,
            hash: None,
            read_only: false,
        }
    }

    fn for_new_file(path: &Path) -> Self {
        let mut value = path.to_string_lossy().into_owned();
        value.push_str("|new-file|0");
        Self {
            size: Some(0),
            modified: None,
            hash: Some(format!("new:{:016x}", stable_hash(&value))),
            read_only: false,
        }
    }

    fn to_protocol(&self) -> ProtocolFileFingerprint {
        let hash = self.hash.clone().unwrap_or_else(|| "nohash".to_string());
        let modified = self
            .modified
            .map(|value| value.0.to_string())
            .unwrap_or_else(|| "nomtime".to_string());
        let size = self
            .size
            .map(|value| value.to_string())
            .unwrap_or_else(|| "nosize".to_string());
        ProtocolFileFingerprint {
            algorithm: "legion-fingerprint-v1".to_string(),
            value: format!("size={size};modified={modified};hash={hash}"),
        }
    }
}

impl PartialEq for FileFingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size && self.modified == other.modified && self.hash == other.hash
    }
}

#[derive(Debug)]
struct WorkspaceState {
    workspace_id: WorkspaceId,
    workspace_root_id: WorkspaceRootId,
    principal_id: PrincipalId,
    root_path: PathBuf,
    trust: TrustState,
    generation: WorkspaceGeneration,
    config: WorkspaceConfigSnapshot,
    config_snapshot_id: SnapshotId,
    next_file_id: u128,
    file_id_by_path: HashMap<String, FileId>,
    file_metadata: HashMap<FileId, FileMetadata>,
    file_path_by_id: HashMap<FileId, String>,
    tree: Vec<FileTreeNode>,
    discovery_records: Vec<WorkspaceDiscoveryRecord>,
    last_scan: HashMap<String, FileFingerprint>,
    active_sessions: HashSet<FileId>,
    search_index: Option<WorkspaceSearchIndexState>,
    watcher_sequence: u64,
    watcher_queue: VecDeque<WatcherEvent>,
    last_watcher_poll: u64,
    last_watcher_signature: HashSet<String>,
    in_recovery: bool,
}

struct WorkspaceStateInit {
    workspace_id: WorkspaceId,
    workspace_root_id: WorkspaceRootId,
    principal_id: PrincipalId,
    root_path: PathBuf,
    trust: TrustState,
    snapshot_id: SnapshotId,
    tree: Vec<FileTreeNode>,
    scan: HashMap<String, FileFingerprint>,
}

impl WorkspaceState {
    fn new(init: WorkspaceStateInit) -> Self {
        let WorkspaceStateInit {
            workspace_id,
            workspace_root_id,
            principal_id,
            root_path,
            trust,
            snapshot_id,
            tree,
            scan,
        } = init;
        let canonical_root = CanonicalPath(root_path.to_string_lossy().into_owned());

        Self {
            workspace_id,
            workspace_root_id,
            principal_id,
            root_path,
            trust,
            generation: WorkspaceGeneration(1),
            config: WorkspaceConfigSnapshot {
                workspace_id,
                root_path: canonical_root,
                merged: HashMap::new(),
                trust_state: trust_to_protocol(trust),
                captured_at: TimestampMillis(now_millis()),
                schema_version: "1.0".to_string(),
            },
            config_snapshot_id: snapshot_id,
            next_file_id: 1,
            file_id_by_path: HashMap::new(),
            file_metadata: HashMap::new(),
            file_path_by_id: HashMap::new(),
            tree,
            discovery_records: Vec::new(),
            last_scan: scan,
            active_sessions: HashSet::new(),
            search_index: None,
            watcher_sequence: 0,
            watcher_queue: VecDeque::new(),
            last_watcher_poll: 0,
            last_watcher_signature: HashSet::new(),
            in_recovery: false,
        }
    }

    fn next_file_id(&mut self) -> FileId {
        let id = FileId(self.next_file_id);
        self.next_file_id = self.next_file_id.saturating_add(1);
        id
    }

    fn enqueue_watcher_event(&mut self, event: WatcherEvent) {
        if self.watcher_queue.len() >= WATCHER_EVENT_BUFFER {
            let _ = self.watcher_queue.pop_front();
        }
        let signature = format!("{}::{:?}", event.path.0, event.kind);
        if self.last_watcher_signature.contains(&signature) {
            return;
        }
        self.last_watcher_signature.insert(signature);
        self.watcher_queue.push_back(event);
    }
}

#[derive(Debug, Clone)]
struct WorkspaceSearchIndexState {
    generation: WorkspaceGeneration,
    index: Arc<WorkspaceSearchIndex>,
    _indexed_file_count: usize,
}

#[derive(Debug)]
struct WorkspaceSearchIndex {
    index: Index,
    path_field: Field,
    content_field: Field,
}

impl WorkspaceSearchIndex {
    fn query_terms(search_text: &str) -> Vec<String> {
        let normalized = search_text.trim().to_lowercase();
        if normalized.len() < 3 {
            return Vec::new();
        }

        let chars: Vec<char> = normalized.chars().collect();
        let mut terms = HashSet::new();
        for window in chars.windows(3) {
            let term = window.iter().collect::<String>();
            terms.insert(term);
        }
        terms.into_iter().collect()
    }

    fn query_is_indexable(search_text: &str) -> bool {
        !Self::query_terms(search_text).is_empty()
    }
}

#[derive(Debug, Default)]
struct DiscoveryConfig {
    skip_hidden: bool,
    skip_generated: bool,
    skip_binary: bool,
    skip_large: bool,
}

/// Actor-like workspace service with typed project state and shallow tree ownership.
pub struct WorkspaceActor {
    fs: Arc<ProjectFilesystem>,
    watcher: Arc<dyn WatcherService + Send + Sync>,
    security: Mutex<DenyByDefaultBroker>,
    state: Mutex<Option<WorkspaceState>>,
    discovery: DiscoveryConfig,
    event_sink: Box<dyn EventSinkPort + Send + Sync>,
}

impl WorkspaceActor {
    /// Creates a new workspace actor.
    pub fn new(
        fs: Arc<ProjectFilesystem>,
        watcher: Arc<dyn WatcherService + Send + Sync>,
        security: DenyByDefaultBroker,
    ) -> Self {
        Self::with_event_sink(fs, watcher, security, Box::new(NoopEventSink))
    }

    /// Creates a new workspace actor with an injected event sink.
    pub fn with_event_sink(
        fs: Arc<ProjectFilesystem>,
        watcher: Arc<dyn WatcherService + Send + Sync>,
        security: DenyByDefaultBroker,
        event_sink: Box<dyn EventSinkPort + Send + Sync>,
    ) -> Self {
        Self {
            fs,
            watcher,
            security: Mutex::new(security),
            state: Mutex::new(None),
            discovery: DiscoveryConfig {
                skip_hidden: true,
                skip_generated: true,
                skip_binary: true,
                skip_large: true,
            },
            event_sink,
        }
    }

    fn now_sequence(state: &mut WorkspaceState) -> EventSequence {
        state.watcher_sequence = state.watcher_sequence.saturating_add(1);
        EventSequence(state.watcher_sequence)
    }

    fn causality() -> CausalityId {
        CausalityId(Uuid::now_v7())
    }

    fn emit(
        &self,
        envelope: Result<legion_protocol::EventEnvelope, legion_observability::ObservabilityError>,
    ) {
        // Observability envelope builders fail closed on invalid core ids; drop
        // the event in that case rather than propagating an audit-build error
        // into the workspace operation that is already returning its own result.
        if let Ok(envelope) = envelope {
            let _ = self.event_sink.emit(EventSinkRequest { envelope });
        }
    }

    fn canonicalize_root_path(&self, state: &WorkspaceState) -> WorkspaceResult<PathBuf> {
        self.fs
            .canonicalize_path(&state.root_path)
            .or_else(|_| self.fs.normalize_path(&state.root_path))
            .map_err(WorkspaceError::Platform)
    }

    fn canonicalize_with_parent_fallback(&self, path: &Path) -> WorkspaceResult<PathBuf> {
        match self.fs.canonicalize_path(path) {
            Ok(path) => Ok(path),
            Err(PlatformError::NotFound { .. }) => {
                let mut suffix = Vec::new();
                let mut cursor = path.to_path_buf();

                while let Err(PlatformError::NotFound { .. }) = self.fs.canonicalize_path(&cursor) {
                    let Some(name) = cursor.file_name() else {
                        break;
                    };
                    suffix.push(name.to_os_string());

                    let Some(parent) = cursor.parent() else {
                        break;
                    };
                    cursor = parent.to_path_buf();
                }

                let mut rebuilt = self
                    .fs
                    .canonicalize_path(&cursor)
                    .or_else(|_| self.fs.normalize_path(&cursor))
                    .map_err(WorkspaceError::Platform)?;

                for part in suffix.iter().rev() {
                    rebuilt.push(part);
                }

                self.fs
                    .normalize_path(&rebuilt)
                    .map_err(WorkspaceError::Platform)
            }
            Err(err) => Err(WorkspaceError::Platform(err)),
        }
    }

    fn workspace_search_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<WorkspaceSearchSnapshot> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        let files = state
            .file_path_by_id
            .iter()
            .map(|(file_id, path)| {
                (
                    *file_id,
                    path.clone(),
                    state.file_metadata.get(file_id).cloned(),
                )
            })
            .collect::<Vec<_>>();

        Ok((state.root_path.clone(), state.generation, files))
    }

    fn rebuild_workspace_search_index(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<WorkspaceSearchIndexState> {
        let (_root_path, generation, files) = self.workspace_search_snapshot(workspace_id)?;

        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", TextOptions::default().set_stored());
        let content_field = schema_builder.add_text_field(
            "content",
            TextOptions::default().set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("trigram")
                    .set_index_option(IndexRecordOption::Basic),
            ),
        );
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);
        let tokenizer = NgramTokenizer::all_ngrams(3, 3)
            .map_err(|_| WorkspaceError::Internal("tantivy tokenizer"))?;
        index.tokenizers().register(
            "trigram",
            TextAnalyzer::builder(tokenizer).filter(LowerCaser).build(),
        );
        let mut writer = index
            .writer(50_000_000)
            .map_err(|_| WorkspaceError::Internal("tantivy writer"))?;

        let mut _indexed_file_count = 0usize;
        for (_file_id, path, metadata) in files {
            if metadata
                .as_ref()
                .and_then(|metadata| metadata.size_bytes)
                .is_some_and(|size_bytes| size_bytes > WORKSPACE_SEARCH_MAX_FILE_BYTES)
            {
                continue;
            }
            let Ok(text) = self.read_file_text(workspace_id, &path) else {
                continue;
            };
            let _ = writer.add_document(doc!(path_field => path, content_field => text));
            _indexed_file_count = _indexed_file_count.saturating_add(1);
        }
        writer
            .commit()
            .map_err(|_| WorkspaceError::Internal("tantivy commit"))?;

        Ok(WorkspaceSearchIndexState {
            generation,
            index: Arc::new(WorkspaceSearchIndex {
                index,
                path_field,
                content_field,
            }),
            _indexed_file_count,
        })
    }

    fn ensure_workspace_search_index(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<Option<WorkspaceSearchIndexState>> {
        let generation = {
            let state_guard = self
                .state
                .lock()
                .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
            let state = state_guard
                .as_ref()
                .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
            if state.workspace_id != workspace_id {
                return Err(WorkspaceError::WorkspaceMissing { workspace_id });
            }
            if let Some(index) = &state.search_index
                && index.generation == state.generation
            {
                return Ok(Some(index.clone()));
            }
            state.generation
        };

        let rebuilt = match self.rebuild_workspace_search_index(workspace_id) {
            Ok(index) => index,
            Err(_) => return Ok(None),
        };

        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }
        if state.generation == generation {
            state.search_index = Some(rebuilt.clone());
            return Ok(Some(rebuilt));
        }
        Ok(state.search_index.clone())
    }

    fn indexed_workspace_search_candidate_paths(
        &self,
        index_state: &WorkspaceSearchIndexState,
        search_text: &str,
    ) -> WorkspaceResult<Vec<String>> {
        let terms = WorkspaceSearchIndex::query_terms(search_text);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let reader = index_state
            .index
            .index
            .reader()
            .map_err(|_| WorkspaceError::Internal("tantivy reader"))?;
        let searcher = reader.searcher();
        let query: Vec<(Occur, Box<dyn Query>)> = terms
            .into_iter()
            .map(|term| {
                let term_query = TermQuery::new(
                    Term::from_field_text(index_state.index.content_field, &term),
                    IndexRecordOption::Basic,
                );
                (Occur::Must, Box::new(term_query) as Box<dyn Query>)
            })
            .collect();
        let boolean = BooleanQuery::new(query);
        let top_docs = TopDocs::with_limit(searcher.num_docs() as usize).order_by_score();
        let docs = searcher
            .search(&boolean, &top_docs)
            .map_err(|_| WorkspaceError::Internal("tantivy search"))?;
        let mut paths = Vec::new();
        for (_, doc_address) in docs {
            let doc = searcher
                .doc::<tantivy::schema::TantivyDocument>(doc_address)
                .map_err(|_| WorkspaceError::Internal("tantivy doc"))?;
            if let Some(path) = doc
                .get_first(index_state.index.path_field)
                .and_then(|value| value.as_str())
            {
                paths.push(path.to_string());
            }
        }
        Ok(paths)
    }

    fn search_workspace_stream_indexed<F>(
        &self,
        query: WorkspaceSearchQuery,
        mut on_batch: F,
    ) -> WorkspaceResult<WorkspaceSearchReport>
    where
        F: FnMut(WorkspaceSearchBatch) -> bool,
    {
        let index_state = match self.ensure_workspace_search_index(query.workspace_id)? {
            Some(index_state) => index_state,
            None => return Ok(WorkspaceSearchReport::default()),
        };

        if !WorkspaceSearchIndex::query_is_indexable(&query.search_text) {
            return Ok(WorkspaceSearchReport::default());
        }

        let (root_path, _, _) = self.workspace_search_snapshot(query.workspace_id)?;
        let candidate_paths =
            self.indexed_workspace_search_candidate_paths(&index_state, &query.search_text)?;
        let result_limit = query.result_limit.max(1);
        let batch_size = query.batch_size.max(1);
        let mut report = WorkspaceSearchReport::default();
        let mut pending_hits = Vec::new();
        let mut pending_omitted_hit_count: usize = 0;
        let mut pending_omitted_file_count: usize = 0;
        let mut pending_diagnostics = Vec::new();

        for path in candidate_paths {
            let relative_path = Path::new(&path)
                .strip_prefix(&root_path)
                .unwrap_or(Path::new(&path));
            if !query.filters.accepts(relative_path) {
                continue;
            }

            let file_identity = match self.resolve_file(query.workspace_id, &path) {
                Ok(identity) => identity,
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!("workspace search skipped `{path}`: {err}");
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };

            let text = match self.read_file_text(query.workspace_id, &path) {
                Ok(text) => text,
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!("workspace search skipped `{path}`: {err}");
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };

            let mut line_start = 0u64;
            for (line_number, line) in text.split_inclusive('\n').enumerate() {
                let line_matches = workspace_search_match_count(line, &query.pattern);
                if line_matches.is_empty() {
                    line_start = line_start.saturating_add(line.len() as u64);
                    continue;
                }

                for match_range in line_matches {
                    if report.hit_count < result_limit {
                        report.hit_count = report.hit_count.saturating_add(1);
                        let byte_start = line_start + match_range.start as u64;
                        let byte_end = line_start + match_range.end as u64;
                        let (snippet, snippet_truncated) = workspace_search_snippet(line);
                        pending_hits.push(WorkspaceSearchHit {
                            file_id: file_identity.file_id,
                            canonical_path: file_identity.canonical_path.clone(),
                            line_number: (line_number as u32).saturating_add(1),
                            byte_range: byte_start..byte_end,
                            line_text: line.to_string(),
                            snippet,
                            snippet_truncated,
                        });
                        if pending_hits.len() >= batch_size
                            && !emit_workspace_search_batch(
                                &mut pending_hits,
                                &mut pending_omitted_hit_count,
                                &mut pending_omitted_file_count,
                                &mut pending_diagnostics,
                                &mut on_batch,
                            )
                        {
                            report.cancelled = true;
                            return Ok(report);
                        }
                    } else {
                        report.omitted_hit_count = report.omitted_hit_count.saturating_add(1);
                        pending_omitted_hit_count = pending_omitted_hit_count.saturating_add(1);
                    }
                }
                line_start = line_start.saturating_add(line.len() as u64);
            }
        }

        if !emit_workspace_search_batch(
            &mut pending_hits,
            &mut pending_omitted_hit_count,
            &mut pending_omitted_file_count,
            &mut pending_diagnostics,
            &mut on_batch,
        ) {
            report.cancelled = true;
            return Ok(report);
        }

        Ok(report)
    }

    fn path_components_for_compare(path: &Path) -> Vec<String> {
        let mut normalized = path.to_string_lossy().replace('\\', "/");

        if normalized.starts_with("//?/UNC/") {
            normalized = format!("//{}", &normalized[8..]);
        } else if normalized.starts_with("//?/") || normalized.starts_with("//./") {
            normalized = normalized[4..].to_string();
        }

        #[cfg(windows)]
        {
            normalized = normalized.to_ascii_lowercase();
        }

        let mut components = Vec::new();
        for part in normalized.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                components.pop();
                continue;
            }
            components.push(part.to_string());
        }
        components
    }

    fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
        let root_parts = Self::path_components_for_compare(root);
        let candidate_parts = Self::path_components_for_compare(candidate);

        if root_parts.len() > candidate_parts.len() {
            return false;
        }

        root_parts
            .iter()
            .zip(candidate_parts.iter())
            .all(|(left, right)| left == right)
    }

    fn check_path_within_root(&self, state: &WorkspaceState, path: &Path) -> WorkspaceResult<()> {
        let root = self.canonicalize_root_path(state)?;
        let candidate = self.canonicalize_with_parent_fallback(path)?;

        if Self::path_is_within_root(&root, &candidate) {
            Ok(())
        } else {
            Err(WorkspaceError::PathOutsideRoot {
                path: candidate.to_string_lossy().into_owned(),
            })
        }
    }

    fn canonicalize_candidate(
        &self,
        state: &WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<PathBuf> {
        let path = Path::new(path);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            state.root_path.join(path)
        };
        // Resolve symlinks like macOS /var → /private/var. For paths that do
        // not exist yet (new-file/create flows), canonicalize the nearest
        // existing parent and rebuild the candidate path under that canonical
        // parent so security policy sees the same root spelling it pinned on
        // workspace open.
        let normalized = self.canonicalize_with_parent_fallback(&absolute)?;
        self.check_path_within_root(state, &normalized)?;
        Ok(normalized)
    }

    fn skip_reason_for_entry(
        &self,
        entry_name: &str,
        metadata: Option<&FileSystemMetadata>,
    ) -> Option<WorkspaceDiscoverySkipReason> {
        if self.discovery.skip_hidden && entry_name.starts_with('.') {
            return Some(if entry_name == ".gitignore" {
                WorkspaceDiscoverySkipReason::Ignored
            } else {
                WorkspaceDiscoverySkipReason::Hidden
            });
        }

        let generated = [".git", "target", ".idea", ".vscode", "out", "dist", "build"];
        if self.discovery.skip_generated && generated.contains(&entry_name) {
            return Some(WorkspaceDiscoverySkipReason::Generated);
        }

        let vendored = ["node_modules", "vendor", "third_party"];
        if self.discovery.skip_generated && vendored.contains(&entry_name) {
            return Some(WorkspaceDiscoverySkipReason::Vendored);
        }

        let binaries = [
            ".exe", ".dll", ".so", ".png", ".jpg", ".jpeg", ".gif", ".pdf", ".zip", ".class",
            ".jar", ".ico", ".bin", ".mp4", ".mp3",
        ];
        if self.discovery.skip_binary
            && let Some(ext) = Path::new(entry_name)
                .extension()
                .and_then(|value| value.to_str())
        {
            let suffix = format!(".{ext}").to_ascii_lowercase();
            if binaries.iter().any(|value| *value == suffix) {
                return Some(WorkspaceDiscoverySkipReason::Binary);
            }
        }

        if self.discovery.skip_large
            && let Some(meta) = metadata
            && meta.is_file()
            && meta.length > LARGE_FILE_BYTES
        {
            return Some(WorkspaceDiscoverySkipReason::Oversized);
        }

        None
    }

    fn trust_result(state: &WorkspaceState) -> WorkspaceDiscoveryTrustResult {
        match state.trust {
            TrustState::Trusted => WorkspaceDiscoveryTrustResult::Trusted,
            TrustState::Untrusted => WorkspaceDiscoveryTrustResult::Untrusted,
            TrustState::Unknown => WorkspaceDiscoveryTrustResult::Unknown,
        }
    }

    fn language_hint_for_path(path: &Path) -> Option<LanguageId> {
        let language = match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => "rust",
            Some("toml") => "toml",
            Some("md") => "markdown",
            Some("json") => "json",
            Some("ts") | Some("tsx") => "typescript",
            Some("js") | Some("jsx") => "javascript",
            Some("py") => "python",
            Some("go") => "go",
            Some("java") => "java",
            Some("c") | Some("h") => "c",
            Some("cpp") | Some("cc") | Some("hpp") => "cpp",
            _ => return None,
        };
        Some(LanguageId(language.to_string()))
    }

    fn display_path(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn content_hash_fingerprint(hash: Option<&String>) -> Option<ProtocolFileFingerprint> {
        hash.map(|value| ProtocolFileFingerprint {
            algorithm: "workspace-content-hash".to_string(),
            value: value.clone(),
        })
    }

    fn discovery_policy(
        state: &WorkspaceState,
        reason: Option<WorkspaceDiscoverySkipReason>,
    ) -> WorkspaceDiscoveryPolicyDecision {
        let trust = Self::trust_result(state);
        let trust_denied = trust != WorkspaceDiscoveryTrustResult::Trusted;
        let skip_reason = if trust_denied {
            Some(WorkspaceDiscoverySkipReason::PolicyDenied)
        } else {
            reason
        };
        let decision = match skip_reason {
            None => WorkspaceDiscoveryDecision::ContentAllowed,
            Some(
                WorkspaceDiscoverySkipReason::Deleted | WorkspaceDiscoverySkipReason::External,
            ) => WorkspaceDiscoveryDecision::Excluded,
            Some(_) => WorkspaceDiscoveryDecision::MetadataOnly,
        };
        let path_policy = match skip_reason {
            Some(WorkspaceDiscoverySkipReason::External) => {
                WorkspaceDiscoveryPathPolicyResult::External
            }
            Some(WorkspaceDiscoverySkipReason::PolicyDenied) => {
                WorkspaceDiscoveryPathPolicyResult::WorkspaceDenied
            }
            _ => WorkspaceDiscoveryPathPolicyResult::WorkspaceAllowed,
        };
        WorkspaceDiscoveryPolicyDecision {
            decision,
            skip_reason,
            path_policy,
            trust,
            generated: matches!(skip_reason, Some(WorkspaceDiscoverySkipReason::Generated)),
            binary: matches!(skip_reason, Some(WorkspaceDiscoverySkipReason::Binary)),
            vendored: matches!(skip_reason, Some(WorkspaceDiscoverySkipReason::Vendored)),
            oversized: matches!(skip_reason, Some(WorkspaceDiscoverySkipReason::Oversized)),
            metadata_only: decision != WorkspaceDiscoveryDecision::ContentAllowed,
        }
    }

    fn discovery_record(
        &self,
        state: &WorkspaceState,
        path: Option<&Path>,
        identity: Option<FileIdentity>,
        metadata: Option<FileMetadata>,
        reason: Option<WorkspaceDiscoverySkipReason>,
        change_kind: Option<WorkspaceDiscoveryChangeKind>,
    ) -> WorkspaceDiscoveryRecord {
        let policy = Self::discovery_policy(state, reason);
        let path_dto = path.map(|path| CanonicalPath(path.to_string_lossy().into_owned()));
        let display_path = path.map(|path| Self::display_path(&state.root_path, path));
        let language_hint = path.and_then(Self::language_hint_for_path);
        let content_fingerprint = metadata
            .as_ref()
            .and_then(|metadata| metadata.fingerprint.clone());
        let content_hash = if policy.decision == WorkspaceDiscoveryDecision::ContentAllowed {
            metadata
                .as_ref()
                .and_then(|metadata| Self::content_hash_fingerprint(metadata.hash.as_ref()))
                .or_else(|| {
                    identity.as_ref().and_then(|identity| {
                        Self::content_hash_fingerprint(identity.content_hash.as_ref())
                    })
                })
        } else {
            None
        };

        WorkspaceDiscoveryRecord {
            schema_version: 1,
            workspace_id: Some(state.workspace_id),
            workspace_root_id: Some(state.workspace_root_id),
            workspace_generation: state.generation,
            identity,
            path: path_dto,
            display_path,
            metadata,
            policy,
            language_hint,
            privacy_scope: if matches!(state.trust, TrustState::Trusted) {
                SemanticPrivacyScope::Workspace
            } else {
                SemanticPrivacyScope::MetadataOnly
            },
            content_fingerprint,
            content_hash,
            change_kind,
        }
    }

    fn kind_for_platform_metadata(&self, metadata: &FileSystemMetadata) -> FileKind {
        match metadata.kind {
            FileSystemEntryKind::Directory => FileKind::Directory,
            FileSystemEntryKind::Symlink => FileKind::Symlink,
            FileSystemEntryKind::File => FileKind::File,
            FileSystemEntryKind::Other => FileKind::Other("other".to_string()),
        }
    }

    fn file_identity_from_platform_metadata(
        &self,
        state: &mut WorkspaceState,
        canonical_path: &Path,
        fingerprint: &FileFingerprint,
        metadata: &FileSystemMetadata,
    ) -> FileIdentity {
        let key = canonical_path.to_string_lossy().into_owned();
        let file_id = if let Some(id) = state.file_id_by_path.get(&key) {
            *id
        } else {
            let id = state.next_file_id();
            state.file_id_by_path.insert(key.clone(), id);
            state.file_path_by_id.insert(id, key.clone());
            id
        };

        let content_version = match (
            fingerprint.size,
            fingerprint.modified,
            fingerprint.hash.as_ref(),
        ) {
            (Some(size), Some(ts), Some(hash)) => {
                let digest = (size ^ ts.0).wrapping_add(stable_hash(hash) as u64);
                FileContentVersion(digest)
            }
            (Some(size), Some(ts), None) => FileContentVersion(size.saturating_add(ts.0)),
            (Some(size), None, _) => FileContentVersion(size),
            _ => FileContentVersion(0),
        };

        let protocol_fingerprint = fingerprint.to_protocol();
        let canonical_path = CanonicalPath(canonical_path.to_string_lossy().into_owned());
        let file_metadata = FileMetadata {
            canonical_path: canonical_path.clone(),
            file_id: Some(file_id),
            workspace_id: Some(state.workspace_id),
            kind: self.kind_for_platform_metadata(metadata),
            size_bytes: fingerprint.size,
            modified_at: fingerprint.modified,
            read_only: fingerprint.read_only,
            permissions: None,
            hash: fingerprint.hash.clone(),
            fingerprint: Some(protocol_fingerprint),
            content_version: Some(content_version),
            workspace_generation: Some(state.generation),
            schema_version: 1,
        };

        state.file_metadata.insert(file_id, file_metadata);

        FileIdentity {
            file_id,
            workspace_id: state.workspace_id,
            canonical_path,
            content_version,
            content_hash: fingerprint.hash.clone(),
        }
    }

    fn file_identity_for_new_path(
        &self,
        state: &mut WorkspaceState,
        canonical_path: &Path,
        fingerprint: &FileFingerprint,
    ) -> FileIdentity {
        let key = canonical_path.to_string_lossy().into_owned();
        let file_id = if let Some(id) = state.file_id_by_path.get(&key) {
            *id
        } else {
            let id = state.next_file_id();
            state.file_id_by_path.insert(key.clone(), id);
            state.file_path_by_id.insert(id, key.clone());
            id
        };
        let protocol_fingerprint = fingerprint.to_protocol();
        state.file_metadata.insert(
            file_id,
            FileMetadata {
                canonical_path: CanonicalPath(key.clone()),
                file_id: Some(file_id),
                workspace_id: Some(state.workspace_id),
                kind: FileKind::File,
                size_bytes: fingerprint.size,
                modified_at: fingerprint.modified,
                read_only: fingerprint.read_only,
                permissions: Some("new-file-precondition".to_string()),
                hash: fingerprint.hash.clone(),
                fingerprint: Some(protocol_fingerprint),
                content_version: Some(FileContentVersion(0)),
                workspace_generation: Some(state.generation),
                schema_version: 1,
            },
        );
        FileIdentity {
            file_id,
            workspace_id: state.workspace_id,
            canonical_path: CanonicalPath(key),
            content_version: FileContentVersion(0),
            content_hash: fingerprint.hash.clone(),
        }
    }

    fn metadata_for_identity(
        &self,
        state: &WorkspaceState,
        file_id: FileId,
    ) -> Option<FileMetadata> {
        state.file_metadata.get(&file_id).cloned()
    }

    fn upsert_tree_node(
        state: &mut WorkspaceState,
        identity: FileIdentity,
        metadata: FileMetadata,
    ) {
        let key = identity.canonical_path.0.clone();
        if let Some(node) = state
            .tree
            .iter_mut()
            .find(|node| node.identity.file_id == identity.file_id)
        {
            node.identity = identity;
            node.name = key
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("unknown")
                .to_string();
            node.metadata = Some(metadata);
        } else {
            state.tree.push(FileTreeNode {
                identity,
                name: key
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
                children: Vec::new(),
                metadata: Some(metadata),
            });
        }
    }

    fn remove_file_from_state(state: &mut WorkspaceState, file_id: FileId, path: &CanonicalPath) {
        state.file_metadata.remove(&file_id);
        state.file_path_by_id.remove(&file_id);
        state.file_id_by_path.remove(&path.0);
        state.last_scan.remove(&path.0);
        state.active_sessions.remove(&file_id);
        state.tree.retain(|node| node.identity.file_id != file_id);
    }

    fn open_existing_file_text_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
        correlation_id: Option<CorrelationId>,
        causality_id: Option<CausalityId>,
    ) -> WorkspaceResult<OpenedFileText> {
        let workspace_id = state.workspace_id;
        let canonical = self.canonicalize_candidate(state, path)?;
        let target_path = canonical.to_string_lossy().into_owned();
        if let Err(err) = self.decision_for_workspace(state, "fs.read", Some(&target_path)) {
            if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                let sequence = Self::now_sequence(state);
                self.emit(security_denial_event(
                    workspace_id,
                    None,
                    Some(state.principal_id.clone()),
                    &CapabilityId("fs.read".to_string()),
                    correlation_id,
                    causality_id,
                    sequence,
                    Some(&target_path),
                    err.to_string(),
                ));
            }
            return Err(err);
        }

        let metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(err) => {
                if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                    let sequence = Self::now_sequence(state);
                    self.emit(open_file_read_failure_event(
                        workspace_id,
                        correlation_id,
                        causality_id,
                        sequence,
                        &target_path,
                        err.to_string(),
                    ));
                }
                return Err(WorkspaceError::Platform(err));
            }
        };
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)?
        } else {
            FileFingerprint::from_dir()
        };
        let identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        let metadata =
            self.metadata_for_identity(state, identity.file_id)
                .ok_or(WorkspaceError::Internal(
                    "file metadata missing after identity capture",
                ))?;
        let text = match self.fs.read_text_file(&canonical) {
            Ok(text) => text,
            Err(err) => {
                if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                    let sequence = Self::now_sequence(state);
                    self.emit(open_file_read_failure_event(
                        workspace_id,
                        correlation_id,
                        causality_id,
                        sequence,
                        canonical.to_string_lossy(),
                        err.to_string(),
                    ));
                }
                return Err(WorkspaceError::Platform(err));
            }
        };
        if text.contains('\0') {
            if let (Some(correlation_id), Some(causality_id)) = (correlation_id, causality_id) {
                let sequence = Self::now_sequence(state);
                self.emit(open_file_read_failure_event(
                    workspace_id,
                    correlation_id,
                    causality_id,
                    sequence,
                    canonical.to_string_lossy(),
                    "binary content rejected for text buffer",
                ));
            }
            return Err(WorkspaceError::Platform(PlatformError::Encoding {
                operation: "read".to_string(),
                path: canonical,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "binary content rejected for text buffer",
                ),
            }));
        }

        state.active_sessions.insert(identity.file_id);
        Ok(OpenedFileText {
            identity: identity.clone(),
            text,
            fingerprint: fingerprint.to_protocol(),
            file_content_version: identity.content_version,
            workspace_generation: state.generation,
            modified_at: metadata.modified_at,
            file_length: metadata.size_bytes,
            is_new_file: false,
        })
    }

    fn open_new_file_text_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<OpenedFileText> {
        let canonical = self.canonicalize_candidate(state, path)?;
        self.decision_for_workspace(state, "fs.write", Some(&canonical.to_string_lossy()))?;
        match self.fs.read_metadata(&canonical) {
            Ok(_) => return self.open_existing_file_text_internal(state, path, None, None),
            Err(PlatformError::NotFound { .. }) => {}
            Err(err) => return Err(WorkspaceError::Platform(err)),
        }

        let fingerprint = FileFingerprint::for_new_file(&canonical);
        let identity = self.file_identity_for_new_path(state, &canonical, &fingerprint);
        state.active_sessions.insert(identity.file_id);
        Ok(OpenedFileText {
            identity,
            text: String::new(),
            fingerprint: fingerprint.to_protocol(),
            file_content_version: FileContentVersion(0),
            workspace_generation: state.generation,
            modified_at: None,
            file_length: Some(0),
            is_new_file: true,
        })
    }

    fn scan_shallow(&self, state: &mut WorkspaceState) -> WorkspaceResult<WorkspaceScanResult> {
        let mut scan = WorkspaceScanAccumulation {
            nodes: Vec::new(),
            fingerprints: HashMap::new(),
            discovery_records: Vec::new(),
        };
        let root_path = state.root_path.clone();

        self.collect_tree_nodes(&root_path, &PathBuf::new(), 0, state, &mut scan)?;

        Ok((scan.nodes, scan.fingerprints, scan.discovery_records))
    }

    fn collect_tree_nodes(
        &self,
        root: &Path,
        relative: &Path,
        depth: usize,
        state: &mut WorkspaceState,
        scan: &mut WorkspaceScanAccumulation,
    ) -> WorkspaceResult<()> {
        if depth > MAX_TREE_CHILDREN_DEPTH {
            return Ok(());
        }

        let target = if relative.as_os_str().is_empty() {
            root.to_path_buf()
        } else {
            root.join(relative)
        };

        let entries = self
            .fs
            .list_directory(&target)
            .map_err(WorkspaceError::Platform)?;

        for child in entries {
            let entry_name: String = child
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();

            let meta = self.fs.read_metadata(&child).ok();
            let meta_ref = meta.as_ref();

            let canonical = self
                .fs
                .normalize_path(&child)
                .map_err(WorkspaceError::Platform)?;
            self.check_path_within_root(state, &canonical)?;

            if let Some(skip_reason) = self.skip_reason_for_entry(&entry_name, meta.as_ref()) {
                let skipped_metadata = meta.as_ref().map(|meta| FileMetadata {
                    canonical_path: CanonicalPath(canonical.to_string_lossy().into_owned()),
                    file_id: None,
                    workspace_id: Some(state.workspace_id),
                    kind: self.kind_for_platform_metadata(meta),
                    size_bytes: Some(meta.length),
                    modified_at: meta.modified_at.map(TimestampMillis),
                    read_only: meta.read_only,
                    permissions: Some("workspace-discovery-skipped".to_string()),
                    hash: None,
                    fingerprint: None,
                    content_version: None,
                    workspace_generation: Some(state.generation),
                    schema_version: 1,
                });
                scan.discovery_records.push(self.discovery_record(
                    state,
                    Some(&canonical),
                    None,
                    skipped_metadata,
                    Some(skip_reason),
                    Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
                ));
                continue;
            }

            let metadata = match meta_ref {
                Some(meta) => {
                    if meta.is_file() {
                        FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), meta)?
                    } else {
                        FileFingerprint::from_dir()
                    }
                }
                None => FileFingerprint {
                    size: None,
                    modified: None,
                    hash: None,
                    read_only: true,
                },
            };
            let identity = if let Some(meta) = meta_ref {
                self.file_identity_from_platform_metadata(state, &canonical, &metadata, meta)
            } else {
                let key = canonical.to_string_lossy().into_owned();
                let file_id = state.next_file_id();
                state.file_id_by_path.insert(key.clone(), file_id);
                state.file_path_by_id.insert(file_id, key.clone());
                state.file_metadata.insert(
                    file_id,
                    FileMetadata {
                        canonical_path: CanonicalPath(key.clone()),
                        file_id: Some(file_id),
                        workspace_id: Some(state.workspace_id),
                        kind: FileKind::Other("unreadable".to_string()),
                        size_bytes: metadata.size,
                        modified_at: metadata.modified,
                        read_only: metadata.read_only,
                        permissions: Some("unreadable".to_string()),
                        hash: metadata.hash.clone(),
                        fingerprint: Some(metadata.to_protocol()),
                        content_version: Some(FileContentVersion(0)),
                        workspace_generation: Some(state.generation),
                        schema_version: 1,
                    },
                );
                FileIdentity {
                    file_id,
                    workspace_id: state.workspace_id,
                    canonical_path: CanonicalPath(key),
                    content_version: FileContentVersion(0),
                    content_hash: None,
                }
            };

            let mut child_ids = Vec::new();
            let is_dir = meta.as_ref().map(|meta| meta.is_dir()).unwrap_or(false);
            if is_dir && depth < MAX_TREE_CHILDREN_DEPTH {
                let child_start = scan.nodes.len();
                self.collect_tree_nodes(root, &relative.join(&entry_name), depth + 1, state, scan)?;
                for child_node in &scan.nodes[child_start..] {
                    child_ids.push(child_node.identity.file_id);
                }
            }

            let metadata = state
                .file_metadata
                .get(&identity.file_id)
                .cloned()
                .unwrap_or_else(|| FileMetadata {
                    canonical_path: identity.canonical_path.clone(),
                    file_id: Some(identity.file_id),
                    workspace_id: Some(identity.workspace_id),
                    kind: FileKind::Other("unknown".to_string()),
                    size_bytes: None,
                    modified_at: None,
                    read_only: false,
                    permissions: None,
                    hash: None,
                    fingerprint: None,
                    content_version: Some(identity.content_version),
                    workspace_generation: Some(state.generation),
                    schema_version: 1,
                });

            scan.fingerprints.insert(
                identity.canonical_path.0.clone(),
                metadata
                    .size_bytes
                    .zip(metadata.modified_at)
                    .map(|(size, modified)| FileFingerprint {
                        size: Some(size),
                        modified: Some(modified),
                        hash: metadata.hash.clone(),
                        read_only: metadata.read_only,
                    })
                    .unwrap_or_else(|| {
                        let mut f = FileFingerprint::from_dir();
                        f.read_only = metadata.read_only;
                        f
                    }),
            );

            scan.discovery_records.push(self.discovery_record(
                state,
                Some(&canonical),
                Some(identity.clone()),
                Some(metadata.clone()),
                None,
                Some(WorkspaceDiscoveryChangeKind::Added),
            ));

            scan.nodes.push(FileTreeNode {
                identity,
                name: entry_name,
                children: child_ids,
                metadata: Some(metadata),
            });
        }

        Ok(())
    }

    fn decision_for_workspace(
        &self,
        state: &WorkspaceState,
        capability: &str,
        path: Option<&str>,
    ) -> WorkspaceResult<()> {
        self.decision_for_workspace_with_context(
            state,
            capability,
            path,
            CapabilityRequestContext::default(),
        )
    }

    fn decision_for_workspace_with_context(
        &self,
        state: &WorkspaceState,
        capability: &str,
        path: Option<&str>,
        context: CapabilityRequestContext,
    ) -> WorkspaceResult<()> {
        let mut security = self
            .security
            .lock()
            .map_err(|_| WorkspaceError::Internal("security lock poisoned"))?;

        let decision = security.decide_with_request_context(
            state.trust,
            state.principal_id.clone(),
            legion_protocol::CapabilityId(capability.to_string()),
            path,
            context,
        );
        match decision {
            legion_security::SecurityDecision::Allow => Ok(()),
            legion_security::SecurityDecision::Deny(reason) => {
                Err(WorkspaceError::SecurityDenied {
                    path: path.unwrap_or("").to_string(),
                    reason,
                })
            }
        }
    }

    fn diagnostic(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<CanonicalPath>,
    ) -> ProtocolDiagnostic {
        ProtocolDiagnostic {
            code: code.into(),
            message: message.into(),
            severity: ProtocolDiagnosticSeverity::Error,
            path,
            range: None,
        }
    }

    fn save_transition(
        request: &WorkspaceSaveRequest,
        state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id: request.proposal_id,
            lifecycle_state: state,
            timestamp: TimestampMillis::now(),
            principal: request.principal.clone(),
            capability: request.required_capability.clone(),
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            diagnostics,
        }
    }

    fn mutation_transition(
        proposal_id: ProposalId,
        principal: &PrincipalId,
        capability: &CapabilityId,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        ProposalLifecycleTransition {
            proposal_id,
            lifecycle_state: state,
            timestamp: TimestampMillis::now(),
            principal: principal.clone(),
            capability: capability.clone(),
            correlation_id,
            causality_id,
            diagnostics,
        }
    }

    fn failed_mutation_response(
        proposal_id: ProposalId,
        principal: &PrincipalId,
        capability: &CapabilityId,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        path: CanonicalPath,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.failed", message, Some(path));
        ProposalResponse::Failed {
            transition: Self::mutation_transition(
                proposal_id,
                principal,
                capability,
                correlation_id,
                causality_id,
                ProposalLifecycleState::Failed,
                vec![diagnostic],
            ),
            reason: ProposalFailureReason::ApplyFailed,
        }
    }

    fn denied_mutation_response(
        proposal_id: ProposalId,
        principal: &PrincipalId,
        capability: &CapabilityId,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        path: CanonicalPath,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.denied", message, Some(path));
        ProposalResponse::Denied {
            transition: Self::mutation_transition(
                proposal_id,
                principal,
                capability,
                correlation_id,
                causality_id,
                ProposalLifecycleState::Denied,
                vec![diagnostic],
            ),
            reason: ProposalDenialReason::PolicyDenied,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stale_mutation_response(
        proposal_id: ProposalId,
        principal: &PrincipalId,
        capability: &CapabilityId,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        path: CanonicalPath,
        reason: ProposalStaleReason,
        expected: ProposalVersionPreconditions,
        actual: Option<legion_protocol::VersionContext>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.stale", message, Some(path));
        ProposalResponse::Stale {
            transition: Self::mutation_transition(
                proposal_id,
                principal,
                capability,
                correlation_id,
                causality_id,
                ProposalLifecycleState::Stale,
                vec![diagnostic],
            ),
            stale: ProposalStaleContext {
                reason,
                expected,
                actual,
            },
        }
    }

    fn denied_save_response(
        request: &WorkspaceSaveRequest,
        reason: ProposalDenialReason,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.denied", message, Some(request.path.clone()));
        ProposalResponse::Denied {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Denied,
                vec![diagnostic],
            ),
            reason,
        }
    }

    fn failed_save_response(
        request: &WorkspaceSaveRequest,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.failed", message, Some(request.path.clone()));
        ProposalResponse::Failed {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Failed,
                vec![diagnostic],
            ),
            reason: ProposalFailureReason::ApplyFailed,
        }
    }

    fn stale_save_response(
        &self,
        request: &WorkspaceSaveRequest,
        reason: ProposalStaleReason,
        actual: Option<legion_protocol::VersionContext>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.stale", message, Some(request.path.clone()));
        ProposalResponse::Stale {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Stale,
                vec![diagnostic],
            ),
            stale: ProposalStaleContext {
                reason,
                expected: ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: Some(request.buffer_version),
                    snapshot_id: Some(request.snapshot_id),
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                actual,
            },
        }
    }

    fn conflict_save_response(
        &self,
        state: &mut WorkspaceState,
        request: &WorkspaceSaveRequest,
        identity: FileIdentity,
        actual_fingerprint: Option<ProtocolFileFingerprint>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = Self::diagnostic("proposal.conflict", message, Some(request.path.clone()));
        let conflict = FileConflictState {
            state: FileConflictLifecycleState::ConflictDirty,
            context: FileConflictContext {
                workspace_id: request.workspace_id,
                file_identity: identity,
                buffer_version: request.buffer_version,
                file_content_version: request.expected_file_content_version,
                snapshot_id: request.snapshot_id,
                disk_fingerprint: actual_fingerprint,
                expected_fingerprint: Some(request.expected_fingerprint.clone()),
                reason: FileConflictReason::DiskFingerprintChanged,
                diagnostics: vec![diagnostic.clone()],
            },
            diagnostics: vec![diagnostic],
            schema_version: 1,
        };
        let sequence = Self::now_sequence(state);
        self.emit(conflict_created_event(
            &conflict,
            request.correlation_id,
            request.causality_id,
            sequence,
        ));
        ProposalResponse::Conflict {
            transition: Self::save_transition(
                request,
                ProposalLifecycleState::Conflict,
                conflict.diagnostics.clone(),
            ),
            conflict,
        }
    }

    fn resolve_identity_internal(
        &self,
        state: &mut WorkspaceState,
        path: &str,
    ) -> WorkspaceResult<FileIdentity> {
        let canonical = self.canonicalize_candidate(state, path)?;
        self.decision_for_workspace(state, "fs.read", Some(&canonical.to_string_lossy()))?;

        let metadata = self
            .fs
            .read_metadata(&canonical)
            .map_err(WorkspaceError::Platform)?;
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)?
        } else {
            FileFingerprint::from_dir()
        };

        let identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        state.active_sessions.insert(identity.file_id);
        Ok(identity)
    }

    fn apply_tree_delta_internal(
        &self,
        state: &mut WorkspaceState,
        delta: FileTreeDelta,
    ) -> WorkspaceResult<()> {
        let identity = delta.identity.clone();
        let canonical_name = identity
            .canonical_path
            .0
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
        match delta.op {
            FileTreeDeltaOp::Add => {
                state.tree.push(FileTreeNode {
                    identity,
                    name: canonical_name,
                    children: Vec::new(),
                    metadata: None,
                });
            }
            FileTreeDeltaOp::Remove => {
                state
                    .tree
                    .retain(|node| node.identity.file_id != delta.identity.file_id);
            }
            FileTreeDeltaOp::Rename | FileTreeDeltaOp::Update => {
                if let Some(node) = state
                    .tree
                    .iter_mut()
                    .find(|node| node.identity.file_id == identity.file_id)
                {
                    node.identity = identity;
                    if let Some(target) = delta.target_path {
                        node.name = target.0.rsplit('/').next().unwrap_or("unknown").to_string();
                    }
                }
            }
        }

        Ok(())
    }

    fn rebuild_tree_from_scan(&self, state: &mut WorkspaceState) -> WorkspaceResult<()> {
        let (nodes, fingerprints, discovery_records) = self.scan_shallow(state)?;
        state.tree = nodes;
        state.last_scan = fingerprints;
        state.discovery_records = discovery_records;
        Ok(())
    }

    fn rebuild_tree_from_scan_bounded(&self, state: &mut WorkspaceState) -> WorkspaceResult<bool> {
        for _ in 0..WATCHER_RECOVERY_MAX_RESCANS {
            if self.rebuild_tree_from_scan(state).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn collect_watcher_events(
        &self,
        state: &mut WorkspaceState,
    ) -> WorkspaceResult<Vec<WatcherEvent>> {
        let now = now_millis();
        if now.saturating_sub(state.last_watcher_poll) < WATCHER_RENAME_DEBOUNCE_MILLIS {
            return Ok(Vec::new());
        }
        state.last_watcher_poll = now;

        let root = state.root_path.clone();
        let workspace_id = state.workspace_id;

        if state.in_recovery {
            let recovered = self.rebuild_tree_from_scan_bounded(state)?;
            if recovered {
                state.in_recovery = false;
                let sequence = Self::now_sequence(state);
                self.emit(watcher_recovery_event(
                    workspace_id,
                    CorrelationId(sequence.0),
                    Self::causality(),
                    sequence,
                    true,
                ));
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Modified,
                    path: CanonicalPath(root.to_string_lossy().into_owned()),
                    old_path: None,
                    sequence,
                };
                state.enqueue_watcher_event(event.clone());
                return Ok(vec![event]);
            }
        }

        let snapshot = self.watcher.snapshot(workspace_id, &root);
        let new_entries: Vec<PathBuf> = match snapshot {
            Ok(events) => events
                .into_iter()
                .map(|event| PathBuf::from(event.path.0))
                .collect(),
            Err(PlatformError::WatcherOverflow { .. }) => {
                state.in_recovery = true;
                let sequence = Self::now_sequence(state);
                self.emit(watcher_recovery_event(
                    workspace_id,
                    CorrelationId(sequence.0),
                    Self::causality(),
                    sequence,
                    false,
                ));
                let overflow = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Overflow,
                    path: CanonicalPath(root.to_string_lossy().into_owned()),
                    old_path: None,
                    sequence,
                };
                state.enqueue_watcher_event(overflow.clone());
                return Ok(vec![overflow]);
            }
            Err(err) => return Err(WorkspaceError::Platform(err)),
        };

        let current = new_entries
            .into_iter()
            .filter_map(|path| {
                let normalized = self.fs.normalize_path(&path).ok()?;
                FileFingerprint::from_path(&normalized, self.fs.as_ref())
                    .ok()
                    .map(|fingerprint| (normalized.to_string_lossy().into_owned(), fingerprint))
            })
            .collect::<HashMap<String, FileFingerprint>>();

        let mut produced = Vec::new();
        let previous: HashSet<String> = state.last_scan.keys().cloned().collect();
        let current_paths: HashSet<String> = current.keys().cloned().collect();

        let removed: Vec<String> = previous.difference(&current_paths).cloned().collect();
        let added: Vec<String> = current_paths.difference(&previous).cloned().collect();

        let mut modified = Vec::new();
        for path in current_paths.intersection(&previous) {
            if state.last_scan.get(path) != current.get(path) {
                modified.push(path.clone());
            }
        }

        if removed.len() == 1 && added.len() == 1 {
            let old_path = removed[0].clone();
            let new_path = added[0].clone();
            let event = WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Renamed,
                path: CanonicalPath(new_path.clone()),
                old_path: Some(CanonicalPath(old_path)),
                sequence: Self::now_sequence(state),
            };
            state.enqueue_watcher_event(event.clone());
            produced.push(event);
        } else {
            for removed_path in removed {
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Deleted,
                    path: CanonicalPath(removed_path),
                    old_path: None,
                    sequence: Self::now_sequence(state),
                };
                state.enqueue_watcher_event(event.clone());
                produced.push(event);
            }
            for added_path in added {
                let event = WatcherEvent {
                    workspace_id,
                    kind: WatcherEventKind::Created,
                    path: CanonicalPath(added_path),
                    old_path: None,
                    sequence: Self::now_sequence(state),
                };
                state.enqueue_watcher_event(event.clone());
                produced.push(event);
            }
        }

        for modified_path in modified {
            let event = WatcherEvent {
                workspace_id,
                kind: WatcherEventKind::Modified,
                path: CanonicalPath(modified_path),
                old_path: None,
                sequence: Self::now_sequence(state),
            };
            state.enqueue_watcher_event(event.clone());
            produced.push(event);
        }

        state.last_scan = current;
        Ok(produced)
    }

    fn pop_watcher_events(&self, state: &mut WorkspaceState) -> Vec<WatcherEvent> {
        let drained: Vec<_> = state.watcher_queue.drain(..).collect();
        state.last_watcher_signature.clear();
        drained
    }

    /// Open or re-open a workspace and populate shallow tree state.
    pub fn open_workspace(
        &self,
        request: WorkspaceOpenRequest,
    ) -> WorkspaceResult<WorkspaceOpened> {
        let requested_root = Path::new(&request.root_path.0);
        let root = self
            .fs
            .canonicalize_path(requested_root)
            .or_else(|_| self.fs.normalize_path(requested_root))
            .map_err(WorkspaceError::Platform)?;

        // Pin the security broker's filesystem authority to the canonical
        // workspace root. The broker is constructed with the default relative
        // `"./"` placeholder roots, which (correctly) never authorize an
        // absolute in-workspace path once `fs.read`/`fs.write` are enforced.
        // Without this, every absolute in-workspace read/write would be denied.
        {
            let mut security = self
                .security
                .lock()
                .map_err(|_| WorkspaceError::Internal("security lock poisoned"))?;
            security.pin_workspace_path_roots(root.to_string_lossy().into_owned());
        }

        let principal_id = request.principal_id.clone();
        let workspace_id = WorkspaceId(stable_hash(&root.to_string_lossy()));
        let root_id = WorkspaceRootId(stable_hash(
            &(root.to_string_lossy().into_owned() + "-root"),
        ));
        let trust = request.trust.unwrap_or(WorkspaceTrustState::Unknown).into();

        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;

        if let Some(existing) = state_guard.as_ref()
            && existing.workspace_id == workspace_id
        {
            return Ok(WorkspaceOpened {
                workspace_id,
                root_id: existing.workspace_root_id,
                generation: existing.generation,
                snapshot_id: existing.config_snapshot_id,
                correlation_id: request.correlation_id,
            });
        }

        let mut state = WorkspaceState::new(WorkspaceStateInit {
            workspace_id,
            workspace_root_id: root_id,
            principal_id,
            root_path: root.clone(),
            trust,
            snapshot_id: SnapshotId(stable_hash(
                &(root.to_string_lossy().into_owned() + "snapshot"),
            )),
            tree: Vec::new(),
            scan: HashMap::new(),
        });
        self.rebuild_tree_from_scan(&mut state)?;
        let snapshot_id = state.config_snapshot_id;

        let generated = WorkspaceOpened {
            workspace_id,
            root_id: state.workspace_root_id,
            generation: state.generation,
            snapshot_id,
            correlation_id: request.correlation_id,
        };

        *state_guard = Some(state);
        Ok(generated)
    }

    /// Resolve a file path inside the workspace and allocate a stable `FileIdentity`.
    pub fn resolve_file(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<FileIdentity> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;

        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        self.resolve_identity_internal(state, path.as_ref())
    }

    /// Read file text via the workspace's filesystem service.
    pub fn read_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<String> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        let path = self.canonicalize_candidate(state, path.as_ref())?;
        self.decision_for_workspace(state, "fs.read", Some(&path.to_string_lossy()))?;
        self.fs
            .read_text_file(&path)
            .map_err(WorkspaceError::Platform)
    }

    /// Read file text through the harness-facing alias.
    pub fn read_workspace_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<String> {
        self.read_file_text(workspace_id, path)
    }

    /// Search workspace files with bounded batches and gitignore-aware traversal.
    pub fn search_workspace_stream<F>(
        &self,
        query: WorkspaceSearchQuery,
        mut on_batch: F,
    ) -> WorkspaceResult<WorkspaceSearchReport>
    where
        F: FnMut(WorkspaceSearchBatch) -> bool,
    {
        let _ = self.poll_watcher_events(query.workspace_id)?;
        if query.use_indexed_backend
            && WorkspaceSearchIndex::query_is_indexable(&query.search_text)
            && self
                .ensure_workspace_search_index(query.workspace_id)?
                .is_some()
        {
            return self.search_workspace_stream_indexed(query, on_batch);
        }

        let (root_path, workspace_id) = {
            let state_guard = self
                .state
                .lock()
                .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
            let state = state_guard
                .as_ref()
                .ok_or(WorkspaceError::WorkspaceMissing {
                    workspace_id: query.workspace_id,
                })?;
            if state.workspace_id != query.workspace_id {
                return Err(WorkspaceError::WorkspaceMissing {
                    workspace_id: query.workspace_id,
                });
            }
            (state.root_path.clone(), state.workspace_id)
        };

        let result_limit = query.result_limit.max(1);
        let batch_size = query.batch_size.max(1);
        let mut report = WorkspaceSearchReport::default();
        let mut pending_hits = Vec::new();
        let mut pending_omitted_hit_count: usize = 0;
        let mut pending_omitted_file_count: usize = 0;
        let mut pending_diagnostics = Vec::new();

        let walker = WalkBuilder::new(&root_path)
            .standard_filters(true)
            .git_ignore(true)
            .ignore(true)
            .git_exclude(true)
            .parents(true)
            .hidden(true)
            .build();
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!("workspace search walk error: {err}");
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };

            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let relative_path = path.strip_prefix(&root_path).unwrap_or(path);
            if !query.filters.accepts(relative_path) {
                continue;
            }

            let relative_label = workspace_search_path_label(relative_path);
            let size_bytes = match entry.metadata() {
                Ok(metadata) => metadata.len(),
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!(
                        "Skipped {relative_label} because file size metadata is unavailable: {err}"
                    );
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };
            if size_bytes > WORKSPACE_SEARCH_MAX_FILE_BYTES {
                report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                let diagnostic = format!(
                    "Skipped {relative_label} because {size_bytes} bytes exceeds the workspace search bound"
                );
                report.diagnostics.push(diagnostic.clone());
                pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                pending_diagnostics.push(diagnostic);
                if !emit_workspace_search_batch(
                    &mut pending_hits,
                    &mut pending_omitted_hit_count,
                    &mut pending_omitted_file_count,
                    &mut pending_diagnostics,
                    &mut on_batch,
                ) {
                    report.cancelled = true;
                    return Ok(report);
                }
                continue;
            }

            let file_identity = match self.resolve_file(workspace_id, &relative_label) {
                Ok(identity) => identity,
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!("workspace search skipped `{relative_label}`: {err}");
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };

            let text = match self.read_file_text(workspace_id, &relative_label) {
                Ok(text) => text,
                Err(err) => {
                    report.omitted_file_count = report.omitted_file_count.saturating_add(1);
                    let diagnostic = format!("workspace search skipped `{relative_label}`: {err}");
                    report.diagnostics.push(diagnostic.clone());
                    pending_omitted_file_count = pending_omitted_file_count.saturating_add(1);
                    pending_diagnostics.push(diagnostic);
                    if !emit_workspace_search_batch(
                        &mut pending_hits,
                        &mut pending_omitted_hit_count,
                        &mut pending_omitted_file_count,
                        &mut pending_diagnostics,
                        &mut on_batch,
                    ) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    continue;
                }
            };

            let mut line_start = 0u64;
            for (line_number, line) in text.split_inclusive('\n').enumerate() {
                let line_matches = workspace_search_match_count(line, &query.pattern);
                if line_matches.is_empty() {
                    line_start = line_start.saturating_add(line.len() as u64);
                    continue;
                }

                for match_range in line_matches {
                    if report.hit_count < result_limit {
                        report.hit_count = report.hit_count.saturating_add(1);
                        let byte_start = line_start + match_range.start as u64;
                        let byte_end = line_start + match_range.end as u64;
                        let (snippet, snippet_truncated) = workspace_search_snippet(line);
                        pending_hits.push(WorkspaceSearchHit {
                            file_id: file_identity.file_id,
                            canonical_path: file_identity.canonical_path.clone(),
                            line_number: (line_number as u32).saturating_add(1),
                            byte_range: byte_start..byte_end,
                            line_text: line.to_string(),
                            snippet,
                            snippet_truncated,
                        });
                        if pending_hits.len() >= batch_size
                            && !emit_workspace_search_batch(
                                &mut pending_hits,
                                &mut pending_omitted_hit_count,
                                &mut pending_omitted_file_count,
                                &mut pending_diagnostics,
                                &mut on_batch,
                            )
                        {
                            report.cancelled = true;
                            return Ok(report);
                        }
                    } else {
                        report.omitted_hit_count = report.omitted_hit_count.saturating_add(1);
                        pending_omitted_hit_count = pending_omitted_hit_count.saturating_add(1);
                    }
                }
                line_start = line_start.saturating_add(line.len() as u64);
            }

            if !emit_workspace_search_batch(
                &mut pending_hits,
                &mut pending_omitted_hit_count,
                &mut pending_omitted_file_count,
                &mut pending_diagnostics,
                &mut on_batch,
            ) {
                report.cancelled = true;
                return Ok(report);
            }
        }

        if !emit_workspace_search_batch(
            &mut pending_hits,
            &mut pending_omitted_hit_count,
            &mut pending_omitted_file_count,
            &mut pending_diagnostics,
            &mut on_batch,
        ) {
            report.cancelled = true;
            return Ok(report);
        }

        Ok(report)
    }

    /// Harness-facing glob over workspace file paths.
    pub fn glob_workspace_files(
        &self,
        workspace_id: WorkspaceId,
        pattern: impl AsRef<str>,
    ) -> WorkspaceResult<Vec<FileIdentity>> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;

        let pattern = pattern.as_ref().trim().to_string();
        if pattern.is_empty() {
            return Ok(Vec::new());
        }

        self.decision_for_workspace(state, "fs.read", Some(&pattern))?;
        let glob =
            Glob::new(&pattern).map_err(|_| WorkspaceError::Internal("invalid glob pattern"))?;
        let matcher = glob.compile_matcher();
        let walker = WalkBuilder::new(&state.root_path)
            .standard_filters(true)
            .git_ignore(true)
            .ignore(true)
            .git_exclude(true)
            .hidden(true)
            .build();

        let mut matches = Vec::new();
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let relative_path = path.strip_prefix(&state.root_path).unwrap_or(path);
            if !matcher.is_match(relative_path) {
                continue;
            }

            let absolute_path = path.to_string_lossy().into_owned();
            let Some(file_id) = state.file_id_by_path.get(&absolute_path).copied() else {
                continue;
            };
            let metadata = state.file_metadata.get(&file_id).cloned();
            matches.push(FileIdentity {
                file_id,
                workspace_id,
                canonical_path: CanonicalPath(absolute_path),
                content_version: metadata
                    .as_ref()
                    .and_then(|metadata| metadata.content_version)
                    .unwrap_or(FileContentVersion(0)),
                content_hash: metadata.and_then(|metadata| metadata.hash),
            });
        }

        Ok(matches)
    }

    /// Harness-facing outline projection for a workspace file.
    pub fn outline_workspace_file(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<Vec<LanguageOutlineSymbolProjection>> {
        let state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;

        let path = self.canonicalize_candidate(state, path.as_ref())?;
        let path_label = path.to_string_lossy().into_owned();
        self.decision_for_workspace(state, "fs.read", Some(&path_label))?;
        if !tree_sitter_supports_path(&path_label) {
            return Ok(Vec::new());
        }

        let Some(file_id) = state.file_id_by_path.get(&path_label).copied() else {
            return Err(WorkspaceError::Internal("workspace file metadata missing"));
        };
        let metadata = state.file_metadata.get(&file_id).cloned();
        let text = self
            .fs
            .read_text_file(&path)
            .map_err(WorkspaceError::Platform)?;
        let content_version = metadata
            .as_ref()
            .and_then(|metadata| metadata.content_version)
            .unwrap_or(FileContentVersion(0));
        let document = SourceDocument::with_versions(
            workspace_id,
            file_id,
            CanonicalPath(path_label.clone()),
            language_id_for_path(&CanonicalPath(path_label)),
            content_version,
            state.generation,
            None,
            SemanticPrivacyScope::File,
            text,
        );
        TreeSitterParser
            .structural_outline(&document)
            .map_err(|_| WorkspaceError::Internal("outline projection failed"))
    }

    /// Harness-facing alias for workspace search.
    pub fn grep_workspace_stream<F>(
        &self,
        query: WorkspaceSearchQuery,
        on_batch: F,
    ) -> WorkspaceResult<WorkspaceSearchReport>
    where
        F: FnMut(WorkspaceSearchBatch) -> bool,
    {
        self.search_workspace_stream(query, on_batch)
    }

    /// Open an existing text file and return mandatory save-precondition metadata.
    pub fn open_existing_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<OpenedFileText> {
        self.open_existing_file_text_with_causality(workspace_id, path, None, None)
    }

    /// Open an existing text file and emit read-failure metadata when causality is supplied.
    pub fn open_existing_file_text_with_causality(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
        correlation_id: Option<CorrelationId>,
        causality_id: Option<CausalityId>,
    ) -> WorkspaceResult<OpenedFileText> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }
        self.open_existing_file_text_internal(state, path.as_ref(), correlation_id, causality_id)
    }

    /// Open a safe new-file buffer only when the caller explicitly requested create intent.
    pub fn open_new_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: impl AsRef<str>,
    ) -> WorkspaceResult<OpenedFileText> {
        let mut state_guard = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state_guard
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }
        self.open_new_file_text_internal(state, path.as_ref())
    }

    /// Create a closed file through workspace VFS authority and proposal lifecycle context.
    #[allow(clippy::result_large_err)]
    pub fn create_file_with_proposal(
        &self,
        request: WorkspaceCreateFileRequest,
    ) -> WorkspaceFileMutationResult {
        let mut state_guard = self.state.lock().map_err(|_| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                "workspace state lock poisoned",
            )
        })?;
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                "workspace id does not match opened workspace",
            ));
        }
        let canonical = match self.canonicalize_candidate(state, &request.path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    request.path.clone(),
                    err.to_string(),
                ));
            }
        };
        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&canonical.to_string_lossy()),
            CapabilityRequestContext {
                write_byte_count: Some(request.initial_content.len() as u64),
                ..CapabilityRequestContext::default()
            },
        ) {
            return Err(Self::denied_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                err.to_string(),
            ));
        }
        if state.generation != request.expected_workspace_generation {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                ProposalStaleReason::WorkspaceGenerationMismatch,
                ProposalVersionPreconditions {
                    file_version: None,
                    buffer_version: None,
                    snapshot_id: None,
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: None,
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: None,
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                None,
                "workspace generation changed before create",
            ));
        }
        if self.fs.read_metadata(&canonical).is_ok() {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                "create destination already exists",
            ));
        }
        if let Err(err) = self
            .fs
            .create_text_file_new(&canonical, &request.initial_content)
        {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                err.to_string(),
            ));
        }
        let metadata = self.fs.read_metadata(&canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                err.to_string(),
            )
        })?;
        let fingerprint = FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)
            .map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.path.clone(),
                err.to_string(),
            )
        })?;
        let identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        let metadata = self
            .metadata_for_identity(state, identity.file_id)
            .expect("metadata inserted");
        let key = identity.canonical_path.0.clone();
        state.last_scan.insert(key, fingerprint.clone());
        Self::upsert_tree_node(state, identity.clone(), metadata);
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());
        let transition = Self::mutation_transition(
            request.proposal_id,
            &request.principal,
            &request.required_capability,
            request.correlation_id,
            request.causality_id,
            ProposalLifecycleState::Applied,
            Vec::new(),
        );
        Ok(WorkspaceFileMutationApplied {
            identity: identity.clone(),
            fingerprint: Some(fingerprint.to_protocol()),
            file_content_version: identity.content_version,
            workspace_generation: state.generation,
            response: ProposalResponse::Applied(transition),
        })
    }

    /// Delete a closed file through workspace VFS authority and proposal lifecycle context.
    #[allow(clippy::result_large_err)]
    pub fn delete_file_with_proposal(
        &self,
        request: WorkspaceDeleteFileRequest,
    ) -> WorkspaceFileMutationResult {
        let mut state_guard = self.state.lock().map_err(|_| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace state lock poisoned",
            )
        })?;
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace id does not match opened workspace",
            ));
        }
        let canonical = match self.canonicalize_candidate(state, &request.file.canonical_path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    request.file.canonical_path.clone(),
                    err.to_string(),
                ));
            }
        };
        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&canonical.to_string_lossy()),
            CapabilityRequestContext::default(),
        ) {
            return Err(Self::denied_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            ));
        }
        if state.generation != request.expected_workspace_generation {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                ProposalStaleReason::WorkspaceGenerationMismatch,
                ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: None,
                    snapshot_id: None,
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                None,
                "workspace generation changed before delete",
            ));
        }
        let metadata = self.fs.read_metadata(&canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let fingerprint = FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)
            .map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let actual_identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        if actual_identity.file_id != request.file.file_id
            || actual_identity.content_version != request.expected_file_content_version
            || fingerprint.to_protocol() != request.expected_fingerprint
        {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                ProposalStaleReason::FingerprintMismatch,
                ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: None,
                    snapshot_id: None,
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                Some(legion_protocol::VersionContext {
                    file_version: actual_identity.content_version,
                    buffer_version: BufferVersion(0),
                    snapshot_id: SnapshotId(0),
                    generation: state.generation,
                    file_content_version: actual_identity.content_version,
                    workspace_generation: state.generation,
                    fingerprint: Some(fingerprint.to_protocol()),
                    file_length: fingerprint.size,
                    modified_at: fingerprint.modified,
                }),
                "file changed before delete",
            ));
        }
        self.fs.remove_file(&canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        Self::remove_file_from_state(state, request.file.file_id, &request.file.canonical_path);
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());
        let transition = Self::mutation_transition(
            request.proposal_id,
            &request.principal,
            &request.required_capability,
            request.correlation_id,
            request.causality_id,
            ProposalLifecycleState::Applied,
            Vec::new(),
        );
        Ok(WorkspaceFileMutationApplied {
            identity: request.file.clone(),
            fingerprint: None,
            file_content_version: request.expected_file_content_version,
            workspace_generation: state.generation,
            response: ProposalResponse::Applied(transition),
        })
    }

    /// Rename a closed file through workspace VFS authority and proposal lifecycle context.
    #[allow(clippy::result_large_err)]
    pub fn rename_file_with_proposal(
        &self,
        request: WorkspaceRenameFileRequest,
    ) -> WorkspaceFileMutationResult {
        let mut state_guard = self.state.lock().map_err(|_| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace state lock poisoned",
            )
        })?;
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                "workspace id does not match opened workspace",
            ));
        }
        let source = match self.canonicalize_candidate(state, &request.file.canonical_path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    request.file.canonical_path.clone(),
                    err.to_string(),
                ));
            }
        };
        let destination = match self.canonicalize_candidate(state, &request.destination.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    request.destination.clone(),
                    err.to_string(),
                ));
            }
        };
        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&source.to_string_lossy()),
            CapabilityRequestContext::default(),
        ) {
            return Err(Self::denied_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            ));
        }
        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&destination.to_string_lossy()),
            CapabilityRequestContext::default(),
        ) {
            return Err(Self::denied_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.destination.clone(),
                err.to_string(),
            ));
        }
        if state.generation != request.expected_workspace_generation {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                ProposalStaleReason::WorkspaceGenerationMismatch,
                ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: None,
                    snapshot_id: None,
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                None,
                "workspace generation changed before rename",
            ));
        }
        if self.fs.read_metadata(&destination).is_ok() {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.destination.clone(),
                "rename destination already exists",
            ));
        }
        let metadata = self.fs.read_metadata(&source).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let fingerprint = FileFingerprint::from_metadata(&source, self.fs.as_ref(), &metadata)
            .map_err(|err| {
                Self::failed_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    request.file.canonical_path.clone(),
                    err.to_string(),
                )
            })?;
        let actual_identity =
            self.file_identity_from_platform_metadata(state, &source, &fingerprint, &metadata);
        if actual_identity.file_id != request.file.file_id
            || actual_identity.content_version != request.expected_file_content_version
            || fingerprint.to_protocol() != request.expected_fingerprint
        {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                ProposalStaleReason::FingerprintMismatch,
                ProposalVersionPreconditions {
                    file_version: Some(request.expected_file_content_version),
                    buffer_version: None,
                    snapshot_id: None,
                    generation: Some(request.expected_workspace_generation),
                    file_content_version: Some(request.expected_file_content_version),
                    workspace_generation: Some(request.expected_workspace_generation),
                    expected_fingerprint: Some(request.expected_fingerprint.clone()),
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                Some(legion_protocol::VersionContext {
                    file_version: actual_identity.content_version,
                    buffer_version: BufferVersion(0),
                    snapshot_id: SnapshotId(0),
                    generation: state.generation,
                    file_content_version: actual_identity.content_version,
                    workspace_generation: state.generation,
                    fingerprint: Some(fingerprint.to_protocol()),
                    file_length: fingerprint.size,
                    modified_at: fingerprint.modified,
                }),
                "file changed before rename",
            ));
        }
        self.fs.rename_path(&source, &destination).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let old_key = request.file.canonical_path.0.clone();
        let new_key = destination.to_string_lossy().into_owned();
        state.file_id_by_path.remove(&old_key);
        state
            .file_id_by_path
            .insert(new_key.clone(), request.file.file_id);
        state
            .file_path_by_id
            .insert(request.file.file_id, new_key.clone());
        state.last_scan.remove(&old_key);
        let metadata = self.fs.read_metadata(&destination).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                request.destination.clone(),
                err.to_string(),
            )
        })?;
        let new_fingerprint =
            FileFingerprint::from_metadata(&destination, self.fs.as_ref(), &metadata).map_err(
                |err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        request.destination.clone(),
                        err.to_string(),
                    )
                },
            )?;
        let new_identity = self.file_identity_from_platform_metadata(
            state,
            &destination,
            &new_fingerprint,
            &metadata,
        );
        let metadata = self
            .metadata_for_identity(state, new_identity.file_id)
            .expect("metadata inserted");
        state.last_scan.insert(new_key, new_fingerprint.clone());
        Self::upsert_tree_node(state, new_identity.clone(), metadata);
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());
        let transition = Self::mutation_transition(
            request.proposal_id,
            &request.principal,
            &request.required_capability,
            request.correlation_id,
            request.causality_id,
            ProposalLifecycleState::Applied,
            Vec::new(),
        );
        Ok(WorkspaceFileMutationApplied {
            identity: new_identity.clone(),
            fingerprint: Some(new_fingerprint.to_protocol()),
            file_content_version: new_identity.content_version,
            workspace_generation: state.generation,
            response: ProposalResponse::Applied(transition),
        })
    }

    fn rollback_target_path(target: &WorkspaceMutationRollbackTarget) -> CanonicalPath {
        match target {
            WorkspaceMutationRollbackTarget::CreatedFile { path } => path.clone(),
            WorkspaceMutationRollbackTarget::DeletedFile { file }
            | WorkspaceMutationRollbackTarget::RenamedFile { file, .. }
            | WorkspaceMutationRollbackTarget::SavedFile { file } => file.canonical_path.clone(),
        }
    }

    fn rollback_checkpoint_path(checkpoint: &WorkspaceMutationRollbackCheckpoint) -> CanonicalPath {
        match checkpoint {
            WorkspaceMutationRollbackCheckpoint::CreatedFile { path } => path.clone(),
            WorkspaceMutationRollbackCheckpoint::DeletedFile { file, .. }
            | WorkspaceMutationRollbackCheckpoint::SavedFile { file, .. } => {
                file.canonical_path.clone()
            }
            WorkspaceMutationRollbackCheckpoint::RenamedFile { destination, .. } => {
                destination.clone()
            }
        }
    }

    fn bump_generation_after_rollback(state: &mut WorkspaceState) {
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());
    }

    #[allow(clippy::result_large_err)]
    fn require_current_rollback_target(
        &self,
        state: &WorkspaceState,
        request: &WorkspaceMutationRollbackRequest,
        canonical: &Path,
        diagnostic_path: CanonicalPath,
    ) -> Result<FileFingerprint, ProposalResponse> {
        let key = canonical.to_string_lossy().into_owned();
        let Some(expected) = state.last_scan.get(&key) else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                diagnostic_path,
                "rollback target is not tracked after mutation",
            ));
        };
        let metadata = self.fs.read_metadata(canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                diagnostic_path.clone(),
                err.to_string(),
            )
        })?;
        let current = FileFingerprint::from_metadata(canonical, self.fs.as_ref(), &metadata)
            .map_err(|err| {
                Self::failed_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    diagnostic_path.clone(),
                    err.to_string(),
                )
            })?;
        if &current != expected {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                diagnostic_path,
                "rollback target changed after mutation; refusing to clobber external changes",
            ));
        }
        Ok(current)
    }

    #[allow(clippy::result_large_err)]
    fn capture_text_rollback_checkpoint(
        &self,
        state: &mut WorkspaceState,
        request: &WorkspaceMutationRollbackCheckpointRequest,
        file: &FileIdentity,
    ) -> Result<(FileIdentity, String), ProposalResponse> {
        if file.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                "rollback checkpoint file workspace does not match opened workspace",
            ));
        }
        let canonical = match self.canonicalize_candidate(state, &file.canonical_path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_mutation_response(
                    request.proposal_id,
                    &request.principal,
                    &request.required_capability,
                    request.correlation_id,
                    request.causality_id,
                    file.canonical_path.clone(),
                    err.to_string(),
                ));
            }
        };
        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&canonical.to_string_lossy()),
            CapabilityRequestContext::default(),
        ) {
            return Err(Self::denied_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                err.to_string(),
            ));
        }
        let metadata = self.fs.read_metadata(&canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let fingerprint = FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)
            .map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        let actual_identity =
            self.file_identity_from_platform_metadata(state, &canonical, &fingerprint, &metadata);
        if actual_identity.file_id != file.file_id
            || actual_identity.content_version != file.content_version
        {
            return Err(Self::stale_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                ProposalStaleReason::FileContentVersionMismatch,
                ProposalVersionPreconditions {
                    file_version: Some(file.content_version),
                    buffer_version: None,
                    snapshot_id: None,
                    generation: None,
                    file_content_version: Some(file.content_version),
                    workspace_generation: None,
                    expected_fingerprint: None,
                    expected_file_length: None,
                    expected_modified_at: None,
                },
                Some(legion_protocol::VersionContext {
                    file_version: actual_identity.content_version,
                    buffer_version: BufferVersion(0),
                    snapshot_id: SnapshotId(0),
                    generation: state.generation,
                    file_content_version: actual_identity.content_version,
                    workspace_generation: state.generation,
                    fingerprint: Some(fingerprint.to_protocol()),
                    file_length: fingerprint.size,
                    modified_at: fingerprint.modified,
                }),
                "file changed before rollback checkpoint",
            ));
        }
        let text = self.fs.read_text_file(&canonical).map_err(|err| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                file.canonical_path.clone(),
                err.to_string(),
            )
        })?;
        Ok((actual_identity, text))
    }

    /// Capture rollback material through workspace authority before an accepted file mutation.
    #[allow(clippy::result_large_err)]
    pub fn rollback_checkpoint_for_file_mutation(
        &self,
        request: WorkspaceMutationRollbackCheckpointRequest,
    ) -> WorkspaceMutationRollbackCheckpointResult {
        let target_path = Self::rollback_target_path(&request.target);
        let mut state_guard = self.state.lock().map_err(|_| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                target_path.clone(),
                "workspace state lock poisoned",
            )
        })?;
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                target_path,
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                target_path,
                "workspace id does not match opened workspace",
            ));
        }

        match &request.target {
            WorkspaceMutationRollbackTarget::CreatedFile { path } => {
                let canonical = match self.canonicalize_candidate(state, &path.0) {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            path.clone(),
                            err.to_string(),
                        ));
                    }
                };
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&canonical.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        path.clone(),
                        err.to_string(),
                    ));
                }
                match self.fs.read_metadata(&canonical) {
                    Ok(_) => Err(Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        path.clone(),
                        "rollback checkpoint expected create destination to be absent",
                    )),
                    Err(PlatformError::NotFound { .. }) => {
                        Ok(WorkspaceMutationRollbackCheckpoint::CreatedFile {
                            path: CanonicalPath(canonical.to_string_lossy().into_owned()),
                        })
                    }
                    Err(err) => Err(Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        path.clone(),
                        err.to_string(),
                    )),
                }
            }
            WorkspaceMutationRollbackTarget::DeletedFile { file } => {
                let (file, text) = self.capture_text_rollback_checkpoint(state, &request, file)?;
                Ok(WorkspaceMutationRollbackCheckpoint::DeletedFile { file, text })
            }
            WorkspaceMutationRollbackTarget::RenamedFile { file, destination } => {
                let canonical_source =
                    match self.canonicalize_candidate(state, &file.canonical_path.0) {
                        Ok(path) => path,
                        Err(err) => {
                            return Err(Self::denied_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                err.to_string(),
                            ));
                        }
                    };
                let canonical_destination = match self.canonicalize_candidate(state, &destination.0)
                {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            destination.clone(),
                            err.to_string(),
                        ));
                    }
                };
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&canonical_source.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    ));
                }
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&canonical_destination.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        destination.clone(),
                        err.to_string(),
                    ));
                }
                match self.fs.read_metadata(&canonical_destination) {
                    Ok(_) => {
                        return Err(Self::failed_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            destination.clone(),
                            "rollback checkpoint expected rename destination to be absent",
                        ));
                    }
                    Err(PlatformError::NotFound { .. }) => {}
                    Err(err) => {
                        return Err(Self::failed_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            destination.clone(),
                            err.to_string(),
                        ));
                    }
                }
                let metadata = self.fs.read_metadata(&canonical_source).map_err(|err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    )
                })?;
                let fingerprint =
                    FileFingerprint::from_metadata(&canonical_source, self.fs.as_ref(), &metadata)
                        .map_err(|err| {
                            Self::failed_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                err.to_string(),
                            )
                        })?;
                let actual_identity = self.file_identity_from_platform_metadata(
                    state,
                    &canonical_source,
                    &fingerprint,
                    &metadata,
                );
                if actual_identity.file_id != file.file_id
                    || actual_identity.content_version != file.content_version
                {
                    return Err(Self::stale_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        ProposalStaleReason::FileContentVersionMismatch,
                        ProposalVersionPreconditions {
                            file_version: Some(file.content_version),
                            buffer_version: None,
                            snapshot_id: None,
                            generation: None,
                            file_content_version: Some(file.content_version),
                            workspace_generation: None,
                            expected_fingerprint: None,
                            expected_file_length: None,
                            expected_modified_at: None,
                        },
                        Some(legion_protocol::VersionContext {
                            file_version: actual_identity.content_version,
                            buffer_version: BufferVersion(0),
                            snapshot_id: SnapshotId(0),
                            generation: state.generation,
                            file_content_version: actual_identity.content_version,
                            workspace_generation: state.generation,
                            fingerprint: Some(fingerprint.to_protocol()),
                            file_length: fingerprint.size,
                            modified_at: fingerprint.modified,
                        }),
                        "file changed before rollback checkpoint",
                    ));
                }
                Ok(WorkspaceMutationRollbackCheckpoint::RenamedFile {
                    file: actual_identity,
                    destination: CanonicalPath(
                        canonical_destination.to_string_lossy().into_owned(),
                    ),
                })
            }
            WorkspaceMutationRollbackTarget::SavedFile { file } => {
                let (file, text) = self.capture_text_rollback_checkpoint(state, &request, file)?;
                Ok(WorkspaceMutationRollbackCheckpoint::SavedFile { file, text })
            }
        }
    }

    /// Compensate an audit-failed file mutation through workspace authority.
    #[allow(clippy::result_large_err)]
    pub fn rollback_file_mutation_with_checkpoint(
        &self,
        request: WorkspaceMutationRollbackRequest,
    ) -> WorkspaceMutationRollbackResult {
        let checkpoint_path = Self::rollback_checkpoint_path(&request.checkpoint);
        let mut state_guard = self.state.lock().map_err(|_| {
            Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                checkpoint_path.clone(),
                "workspace state lock poisoned",
            )
        })?;
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                checkpoint_path,
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_mutation_response(
                request.proposal_id,
                &request.principal,
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                checkpoint_path,
                "workspace id does not match opened workspace",
            ));
        }

        match &request.checkpoint {
            WorkspaceMutationRollbackCheckpoint::CreatedFile { path } => {
                let canonical = match self.canonicalize_candidate(state, &path.0) {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            path.clone(),
                            err.to_string(),
                        ));
                    }
                };
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&canonical.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        path.clone(),
                        err.to_string(),
                    ));
                }
                let _ = self.require_current_rollback_target(
                    state,
                    &request,
                    &canonical,
                    path.clone(),
                )?;
                self.fs.remove_file(&canonical).map_err(|err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        path.clone(),
                        err.to_string(),
                    )
                })?;
                let key = canonical.to_string_lossy().into_owned();
                if let Some(file_id) = state.file_id_by_path.get(&key).copied() {
                    Self::remove_file_from_state(state, file_id, &CanonicalPath(key));
                } else {
                    state.last_scan.remove(&key);
                    state
                        .tree
                        .retain(|node| node.identity.canonical_path.0 != key);
                }
            }
            WorkspaceMutationRollbackCheckpoint::DeletedFile { file, text }
            | WorkspaceMutationRollbackCheckpoint::SavedFile { file, text } => {
                let canonical = match self.canonicalize_candidate(state, &file.canonical_path.0) {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            file.canonical_path.clone(),
                            err.to_string(),
                        ));
                    }
                };
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&canonical.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    ));
                }
                if matches!(
                    &request.checkpoint,
                    WorkspaceMutationRollbackCheckpoint::DeletedFile { .. }
                ) {
                    match self.fs.read_metadata(&canonical) {
                        Ok(_) => {
                            return Err(Self::failed_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                "rollback restore target already exists",
                            ));
                        }
                        Err(PlatformError::NotFound { .. }) => {}
                        Err(err) => {
                            return Err(Self::failed_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                err.to_string(),
                            ));
                        }
                    }
                }
                if matches!(
                    &request.checkpoint,
                    WorkspaceMutationRollbackCheckpoint::SavedFile { .. }
                ) {
                    let _ = self.require_current_rollback_target(
                        state,
                        &request,
                        &canonical,
                        file.canonical_path.clone(),
                    )?;
                }
                self.fs
                    .write_text_file_atomic(&canonical, text)
                    .map_err(|err| {
                        Self::failed_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            file.canonical_path.clone(),
                            err.to_string(),
                        )
                    })?;
                let metadata = self.fs.read_metadata(&canonical).map_err(|err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    )
                })?;
                let fingerprint =
                    FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)
                        .map_err(|err| {
                            Self::failed_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                err.to_string(),
                            )
                        })?;
                let key = canonical.to_string_lossy().into_owned();
                state.file_id_by_path.insert(key.clone(), file.file_id);
                state.file_path_by_id.insert(file.file_id, key.clone());
                let identity = self.file_identity_from_platform_metadata(
                    state,
                    &canonical,
                    &fingerprint,
                    &metadata,
                );
                let metadata = self
                    .metadata_for_identity(state, identity.file_id)
                    .expect("metadata inserted");
                state.last_scan.insert(key, fingerprint.clone());
                Self::upsert_tree_node(state, identity, metadata);
            }
            WorkspaceMutationRollbackCheckpoint::RenamedFile { file, destination } => {
                let source = match self.canonicalize_candidate(state, &file.canonical_path.0) {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            file.canonical_path.clone(),
                            err.to_string(),
                        ));
                    }
                };
                let destination = match self.canonicalize_candidate(state, &destination.0) {
                    Ok(path) => path,
                    Err(err) => {
                        return Err(Self::denied_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            destination.clone(),
                            err.to_string(),
                        ));
                    }
                };
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&destination.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        CanonicalPath(destination.to_string_lossy().into_owned()),
                        err.to_string(),
                    ));
                }
                if let Err(err) = self.decision_for_workspace_with_context(
                    state,
                    &request.required_capability.0,
                    Some(&source.to_string_lossy()),
                    CapabilityRequestContext::default(),
                ) {
                    return Err(Self::denied_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    ));
                }
                match self.fs.read_metadata(&source) {
                    Ok(_) => {
                        return Err(Self::failed_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            file.canonical_path.clone(),
                            "rollback source already exists",
                        ));
                    }
                    Err(PlatformError::NotFound { .. }) => {}
                    Err(err) => {
                        return Err(Self::failed_mutation_response(
                            request.proposal_id,
                            &request.principal,
                            &request.required_capability,
                            request.correlation_id,
                            request.causality_id,
                            file.canonical_path.clone(),
                            err.to_string(),
                        ));
                    }
                }
                let _ = self.require_current_rollback_target(
                    state,
                    &request,
                    &destination,
                    CanonicalPath(destination.to_string_lossy().into_owned()),
                )?;
                self.fs.rename_path(&destination, &source).map_err(|err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        CanonicalPath(destination.to_string_lossy().into_owned()),
                        err.to_string(),
                    )
                })?;
                let source_key = source.to_string_lossy().into_owned();
                let destination_key = destination.to_string_lossy().into_owned();
                state.file_id_by_path.remove(&destination_key);
                state
                    .file_id_by_path
                    .insert(source_key.clone(), file.file_id);
                state
                    .file_path_by_id
                    .insert(file.file_id, source_key.clone());
                state.last_scan.remove(&destination_key);
                let metadata = self.fs.read_metadata(&source).map_err(|err| {
                    Self::failed_mutation_response(
                        request.proposal_id,
                        &request.principal,
                        &request.required_capability,
                        request.correlation_id,
                        request.causality_id,
                        file.canonical_path.clone(),
                        err.to_string(),
                    )
                })?;
                let fingerprint =
                    FileFingerprint::from_metadata(&source, self.fs.as_ref(), &metadata).map_err(
                        |err| {
                            Self::failed_mutation_response(
                                request.proposal_id,
                                &request.principal,
                                &request.required_capability,
                                request.correlation_id,
                                request.causality_id,
                                file.canonical_path.clone(),
                                err.to_string(),
                            )
                        },
                    )?;
                let identity = self.file_identity_from_platform_metadata(
                    state,
                    &source,
                    &fingerprint,
                    &metadata,
                );
                let metadata = self
                    .metadata_for_identity(state, identity.file_id)
                    .expect("metadata inserted");
                state.last_scan.insert(source_key, fingerprint.clone());
                Self::upsert_tree_node(state, identity, metadata);
            }
        }

        Self::bump_generation_after_rollback(state);
        Ok(WorkspaceMutationRollbackApplied {
            workspace_generation: state.generation,
        })
    }

    /// Apply a save through mandatory proposal context and fail-closed fingerprint preconditions.
    #[allow(clippy::result_large_err)]
    pub fn save_file_with_proposal(&self, request: WorkspaceSaveRequest) -> WorkspaceSaveResult {
        let mut state_guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Err(Self::failed_save_response(
                    &request,
                    "workspace state lock poisoned",
                ));
            }
        };
        let Some(state) = state_guard.as_mut() else {
            return Err(Self::failed_save_response(
                &request,
                "workspace is not open",
            ));
        };
        if state.workspace_id != request.workspace_id {
            return Err(Self::failed_save_response(
                &request,
                "workspace id does not match opened workspace",
            ));
        }

        let canonical = match self.canonicalize_candidate(state, &request.path.0) {
            Ok(path) => path,
            Err(err) => {
                return Err(Self::denied_save_response(
                    &request,
                    ProposalDenialReason::PolicyDenied,
                    err.to_string(),
                ));
            }
        };

        if request.payload_byte_len != request.text.len() as u64 {
            return Err(Self::failed_save_response(
                &request,
                "payload byte length does not match text payload",
            ));
        }

        if let Err(err) = self.decision_for_workspace_with_context(
            state,
            &request.required_capability.0,
            Some(&canonical.to_string_lossy()),
            CapabilityRequestContext {
                write_byte_count: Some(request.payload_byte_len),
                ..CapabilityRequestContext::default()
            },
        ) {
            let sequence = Self::now_sequence(state);
            self.emit(security_denial_event(
                request.workspace_id,
                Some(request.file_id),
                Some(request.principal.clone()),
                &request.required_capability,
                request.correlation_id,
                request.causality_id,
                sequence,
                Some(&canonical.to_string_lossy()),
                err.to_string(),
            ));
            return Err(Self::denied_save_response(
                &request,
                ProposalDenialReason::CapabilityDenied,
                err.to_string(),
            ));
        }

        if state.generation != request.expected_workspace_generation {
            let sequence = Self::now_sequence(state);
            self.emit(stale_proposal_rejected_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                request.proposal_id,
                ProposalStaleReason::WorkspaceGenerationMismatch,
            ));
            return Err(self.stale_save_response(
                &request,
                ProposalStaleReason::WorkspaceGenerationMismatch,
                None,
                "workspace generation changed before save",
            ));
        }

        let fallback_policy = NonAtomicSaveFallbackPolicy::Disabled;

        let actual_metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => Some(metadata),
            Err(PlatformError::NotFound { .. }) => {
                if request.expected_file_content_version == FileContentVersion(0)
                    && request.expected_fingerprint.value.contains("hash=new:")
                {
                    None
                } else {
                    let identity = FileIdentity {
                        file_id: request.file_id,
                        workspace_id: request.workspace_id,
                        canonical_path: request.path.clone(),
                        content_version: FileContentVersion(0),
                        content_hash: None,
                    };
                    return Err(self.conflict_save_response(
                        state,
                        &request,
                        identity,
                        None,
                        "file disappeared from disk before save",
                    ));
                }
            }
            Err(err) => {
                return Err(Self::failed_save_response(&request, err.to_string()));
            }
        };
        let actual_fingerprint = match actual_metadata.as_ref() {
            Some(metadata) if metadata.is_file() => {
                match FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), metadata) {
                    Ok(fingerprint) => fingerprint,
                    Err(err) => {
                        return Err(Self::failed_save_response(&request, err.to_string()));
                    }
                }
            }
            Some(_) => FileFingerprint::from_dir(),
            None => FileFingerprint::for_new_file(&canonical),
        };
        let actual_protocol_fingerprint = actual_fingerprint.to_protocol();

        let actual_identity = if let Some(metadata) = actual_metadata.as_ref() {
            self.file_identity_from_platform_metadata(
                state,
                &canonical,
                &actual_fingerprint,
                metadata,
            )
        } else {
            self.file_identity_for_new_path(state, &canonical, &actual_fingerprint)
        };

        if actual_identity.file_id != request.file_id {
            return Err(self.conflict_save_response(
                state,
                &request,
                actual_identity,
                Some(actual_protocol_fingerprint),
                "file identity changed before save",
            ));
        }

        if actual_identity.content_version != request.expected_file_content_version {
            let actual_context = legion_protocol::VersionContext {
                file_version: actual_identity.content_version,
                buffer_version: request.buffer_version,
                snapshot_id: request.snapshot_id,
                generation: state.generation,
                file_content_version: actual_identity.content_version,
                workspace_generation: state.generation,
                fingerprint: Some(actual_protocol_fingerprint.clone()),
                file_length: actual_fingerprint.size,
                modified_at: actual_fingerprint.modified,
            };
            let sequence = Self::now_sequence(state);
            self.emit(stale_proposal_rejected_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                request.proposal_id,
                ProposalStaleReason::FileContentVersionMismatch,
            ));
            return Err(self.stale_save_response(
                &request,
                ProposalStaleReason::FileContentVersionMismatch,
                Some(actual_context),
                "file content version changed before save",
            ));
        }

        if actual_protocol_fingerprint != request.expected_fingerprint {
            return Err(self.conflict_save_response(
                state,
                &request,
                actual_identity,
                Some(actual_protocol_fingerprint),
                "disk fingerprint changed before save",
            ));
        }

        if let Err(err) = self.fs.write_text_file_atomic(&canonical, &request.text) {
            let fallback_status = match fallback_policy {
                NonAtomicSaveFallbackPolicy::Disabled => {
                    "non-atomic fallback disabled; failing closed"
                }
            };
            let sequence = Self::now_sequence(state);
            self.emit(fallback_denied_event(
                request.workspace_id,
                request.file_id,
                request.correlation_id,
                request.causality_id,
                sequence,
                fallback_status,
            ));
            return Err(Self::failed_save_response(
                &request,
                format!("{err}; {fallback_status}"),
            ));
        }

        let metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(err) => {
                return Err(Self::failed_save_response(&request, err.to_string()));
            }
        };
        let new_fingerprint =
            match FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata) {
                Ok(fingerprint) => fingerprint,
                Err(err) => return Err(Self::failed_save_response(&request, err.to_string())),
            };
        let new_identity = self.file_identity_from_platform_metadata(
            state,
            &canonical,
            &new_fingerprint,
            &metadata,
        );
        let metadata = self
            .metadata_for_identity(state, new_identity.file_id)
            .unwrap_or_else(|| FileMetadata {
                canonical_path: new_identity.canonical_path.clone(),
                file_id: Some(new_identity.file_id),
                workspace_id: Some(new_identity.workspace_id),
                kind: FileKind::File,
                size_bytes: new_fingerprint.size,
                modified_at: new_fingerprint.modified,
                read_only: new_fingerprint.read_only,
                permissions: None,
                hash: new_fingerprint.hash.clone(),
                fingerprint: Some(new_fingerprint.to_protocol()),
                content_version: Some(new_identity.content_version),
                workspace_generation: Some(state.generation),
                schema_version: 1,
            });
        let key = new_identity.canonical_path.0.clone();
        state.last_scan.insert(key.clone(), new_fingerprint.clone());
        if !state
            .tree
            .iter()
            .any(|node| node.identity.file_id == new_identity.file_id)
        {
            state.tree.push(FileTreeNode {
                identity: new_identity.clone(),
                name: key
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
                children: Vec::new(),
                metadata: Some(metadata.clone()),
            });
        } else if let Some(node) = state
            .tree
            .iter_mut()
            .find(|node| node.identity.file_id == new_identity.file_id)
        {
            node.identity = new_identity.clone();
            node.metadata = Some(metadata.clone());
        }
        state.generation = WorkspaceGeneration(state.generation.0.saturating_add(1));
        state.config.captured_at = TimestampMillis(now_millis());

        let transition =
            Self::save_transition(&request, ProposalLifecycleState::Applied, Vec::new());
        Ok(WorkspaceSaveApplied {
            identity: new_identity.clone(),
            fingerprint: new_fingerprint.to_protocol(),
            file_content_version: new_identity.content_version,
            workspace_generation: state.generation,
            modified_at: metadata.modified_at,
            file_length: metadata.size_bytes,
            used_non_atomic_fallback: false,
            fallback_status: Some("atomic-write-only; non-atomic fallback disabled".to_string()),
            response: ProposalResponse::Applied(transition),
        })
    }

    /// Return a workspace-authored semantic discovery snapshot from cached project authority state.
    pub fn semantic_discovery_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<WorkspaceDiscoverySnapshot> {
        let state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_ref()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        Ok(WorkspaceDiscoverySnapshot {
            schema_version: 1,
            workspace_id,
            workspace_root_id: Some(state.workspace_root_id),
            workspace_generation: state.generation,
            captured_at: TimestampMillis(now_millis()),
            records: state.discovery_records.clone(),
            diagnostics: Vec::new(),
        })
    }

    /// Build a workspace-authored semantic discovery delta from watcher events.
    pub fn semantic_discovery_delta_from_watcher_events(
        &self,
        workspace_id: WorkspaceId,
        events: &[WatcherEvent],
    ) -> WorkspaceResult<WorkspaceDiscoveryDelta> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        let mut records = Vec::new();
        let mut sequence = EventSequence(0);
        for event in events {
            sequence = EventSequence(sequence.0.max(event.sequence.0));
            records.push(self.discovery_record_for_watcher_event(state, event));
        }

        Ok(WorkspaceDiscoveryDelta {
            schema_version: 1,
            workspace_id,
            workspace_generation: state.generation,
            sequence,
            records,
            diagnostics: Vec::new(),
        })
    }

    fn discovery_record_for_watcher_event(
        &self,
        state: &mut WorkspaceState,
        event: &WatcherEvent,
    ) -> WorkspaceDiscoveryRecord {
        let raw_path = PathBuf::from(&event.path.0);
        let canonical = match self.canonicalize_with_parent_fallback(&raw_path) {
            Ok(path) => path,
            Err(_) => {
                return self.discovery_record(
                    state,
                    Some(&raw_path),
                    None,
                    None,
                    Some(WorkspaceDiscoverySkipReason::External),
                    Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
                );
            }
        };

        let root = match self.canonicalize_root_path(state) {
            Ok(root) => root,
            Err(_) => state.root_path.clone(),
        };
        if !Self::path_is_within_root(&root, &canonical) {
            return self.discovery_record(
                state,
                Some(&canonical),
                None,
                None,
                Some(WorkspaceDiscoverySkipReason::External),
                Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
            );
        }

        if matches!(event.kind, WatcherEventKind::Deleted) {
            let key = canonical.to_string_lossy().into_owned();
            let identity = state.file_id_by_path.get(&key).map(|file_id| FileIdentity {
                file_id: *file_id,
                workspace_id: state.workspace_id,
                canonical_path: CanonicalPath(key.clone()),
                content_version: state
                    .file_metadata
                    .get(file_id)
                    .and_then(|metadata| metadata.content_version)
                    .unwrap_or(FileContentVersion(0)),
                content_hash: state
                    .file_metadata
                    .get(file_id)
                    .and_then(|metadata| metadata.hash.clone()),
            });
            let metadata = identity
                .as_ref()
                .and_then(|identity| state.file_metadata.get(&identity.file_id).cloned());
            return self.discovery_record(
                state,
                Some(&canonical),
                identity,
                metadata,
                Some(WorkspaceDiscoverySkipReason::Deleted),
                Some(WorkspaceDiscoveryChangeKind::Deleted),
            );
        }

        let metadata = match self.fs.read_metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(_) => {
                return self.discovery_record(
                    state,
                    Some(&canonical),
                    None,
                    None,
                    Some(WorkspaceDiscoverySkipReason::Deleted),
                    Some(WorkspaceDiscoveryChangeKind::Deleted),
                );
            }
        };
        let entry_name = canonical
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let skip_reason = self.skip_reason_for_entry(&entry_name, Some(&metadata));
        let fingerprint = if metadata.is_file() {
            FileFingerprint::from_metadata(&canonical, self.fs.as_ref(), &metadata)
                .unwrap_or_else(|_| FileFingerprint::from_dir())
        } else {
            FileFingerprint::from_dir()
        };
        let identity = if skip_reason.is_none() {
            Some(self.file_identity_from_platform_metadata(
                state,
                &canonical,
                &fingerprint,
                &metadata,
            ))
        } else {
            None
        };
        let metadata = identity
            .as_ref()
            .and_then(|identity| state.file_metadata.get(&identity.file_id).cloned())
            .or_else(|| {
                Some(FileMetadata {
                    canonical_path: CanonicalPath(canonical.to_string_lossy().into_owned()),
                    file_id: None,
                    workspace_id: Some(state.workspace_id),
                    kind: self.kind_for_platform_metadata(&metadata),
                    size_bytes: Some(metadata.length),
                    modified_at: metadata.modified_at.map(TimestampMillis),
                    read_only: metadata.read_only,
                    permissions: Some("workspace-discovery-delta".to_string()),
                    hash: fingerprint.hash.clone(),
                    fingerprint: Some(fingerprint.to_protocol()),
                    content_version: identity.as_ref().map(|identity| identity.content_version),
                    workspace_generation: Some(state.generation),
                    schema_version: 1,
                })
            });
        let change_kind = match event.kind {
            WatcherEventKind::Created | WatcherEventKind::Renamed => {
                WorkspaceDiscoveryChangeKind::Added
            }
            WatcherEventKind::Modified | WatcherEventKind::Overflow => {
                WorkspaceDiscoveryChangeKind::Changed
            }
            WatcherEventKind::Deleted => WorkspaceDiscoveryChangeKind::Deleted,
        };
        self.discovery_record(
            state,
            Some(&canonical),
            identity,
            metadata,
            skip_reason,
            Some(change_kind),
        )
    }

    /// Read current cached shallow tree.
    pub fn tree_snapshot(&self) -> WorkspaceResult<Vec<FileTreeNode>> {
        let state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        Ok(state
            .as_ref()
            .map(|state| state.tree.clone())
            .unwrap_or_default())
    }

    /// Set workspace trust state and bump config snapshot generation.
    pub fn set_trust(&self, workspace_id: WorkspaceId, trust: TrustState) -> WorkspaceResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;
        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        state.trust = trust;
        state.config.trust_state = trust_to_protocol(trust);
        state.config.captured_at = TimestampMillis(now_millis());
        Ok(())
    }

    /// Returns the current workspace root path if a workspace is loaded.
    pub fn current_workspace_root(&self) -> WorkspaceResult<PathBuf> {
        let state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state.as_ref().ok_or(WorkspaceError::WorkspaceMissing {
            workspace_id: WorkspaceId(0),
        })?;

        Ok(state.root_path.clone())
    }

    /// Drain debounced watcher events.
    pub fn poll_watcher_events(
        &self,
        workspace_id: WorkspaceId,
    ) -> WorkspaceResult<Vec<WatcherEvent>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WorkspaceError::Internal("workspace state lock poisoned"))?;
        let state = state
            .as_mut()
            .ok_or(WorkspaceError::WorkspaceMissing { workspace_id })?;

        if state.workspace_id != workspace_id {
            return Err(WorkspaceError::WorkspaceMissing { workspace_id });
        }

        let mut produced = self.collect_watcher_events(state)?;
        let queued = self.pop_watcher_events(state);
        produced.extend(queued);
        Ok(produced)
    }

    fn protocol_error(error: WorkspaceError) -> ProtocolError {
        ProtocolError {
            code: "workspace_error".to_string(),
            message: error.to_string(),
        }
    }
}

impl legion_protocol::WorkspacePort for WorkspaceActor {
    /// Handle protocol workspace request messages.
    fn handle(&self, request: WorkspaceRequest) -> ProtocolResult<WorkspaceResponse> {
        let response = match request {
            WorkspaceRequest::Open(request) => {
                let opened = self.open_workspace(request).map_err(Self::protocol_error)?;

                WorkspaceResponse::Opened(opened)
            }
            WorkspaceRequest::Close(WorkspaceCloseRequest {
                workspace_id,
                correlation_id: _,
                principal_id: _,
            }) => {
                let mut guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let existing = guard.take();
                if let Some(state) = existing {
                    if state.workspace_id == workspace_id {
                        WorkspaceResponse::Closed(WorkspaceClosed {
                            workspace_id,
                            correlation_id: CorrelationId(0),
                            success: true,
                        })
                    } else {
                        *guard = Some(state);
                        WorkspaceResponse::Closed(WorkspaceClosed {
                            workspace_id,
                            correlation_id: CorrelationId(0),
                            success: false,
                        })
                    }
                } else {
                    return Err(ProtocolError::unsupported("workspace not open"));
                }
            }
            WorkspaceRequest::ResolveFile { workspace_id, path } => {
                let identity = self
                    .resolve_file(workspace_id, path.0)
                    .map_err(Self::protocol_error)?;
                WorkspaceResponse::ResolvedFile(identity)
            }
            WorkspaceRequest::ReadConfig(workspace_id) => {
                let guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_ref().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing { workspace_id })
                })?;
                if state.workspace_id != workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id,
                    }));
                }
                WorkspaceResponse::Config(state.config.clone())
            }
            WorkspaceRequest::ReadTree(workspace_id) => {
                let guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_ref().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing { workspace_id })
                })?;
                if state.workspace_id != workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id,
                    }));
                }

                WorkspaceResponse::Tree(state.tree.clone())
            }
            WorkspaceRequest::ReadSemanticDiscoverySnapshot(workspace_id) => {
                WorkspaceResponse::SemanticDiscoverySnapshot(
                    self.semantic_discovery_snapshot(workspace_id)
                        .map_err(Self::protocol_error)?,
                )
            }
            WorkspaceRequest::BuildSemanticDiscoveryDelta {
                workspace_id,
                events,
            } => WorkspaceResponse::SemanticDiscoveryDelta(
                self.semantic_discovery_delta_from_watcher_events(workspace_id, &events)
                    .map_err(Self::protocol_error)?,
            ),
            WorkspaceRequest::ApplyTreeDelta(delta) => {
                let mut guard = self.state.lock().map_err(|_| {
                    Self::protocol_error(WorkspaceError::Internal("workspace state lock poisoned"))
                })?;
                let state = guard.as_mut().ok_or_else(|| {
                    Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id: delta.workspace_id,
                    })
                })?;
                if state.workspace_id != delta.workspace_id {
                    return Err(Self::protocol_error(WorkspaceError::WorkspaceMissing {
                        workspace_id: delta.workspace_id,
                    }));
                }

                self.apply_tree_delta_internal(state, delta)
                    .map_err(Self::protocol_error)?;
                WorkspaceResponse::Tree(state.tree.clone())
            }
        };

        Ok(response)
    }
}

impl legion_protocol::ProjectInfoPort for WorkspaceActor {
    fn resolve_project_for_file(
        &self,
        query: legion_protocol::ProjectInfoQuery,
    ) -> Result<legion_protocol::ProjectInfo, legion_protocol::ProjectServiceError> {
        let mut state_guard =
            self.state
                .lock()
                .map_err(|_| legion_protocol::ProjectServiceError {
                    code: "workspace_lock_poisoned".to_string(),
                    message: "project service is unavailable".to_string(),
                })?;

        let state = state_guard
            .as_mut()
            .ok_or_else(|| legion_protocol::ProjectServiceError {
                code: "workspace_not_open".to_string(),
                message: "no workspace opened in service".to_string(),
            })?;

        let identity = self
            .resolve_identity_internal(state, &query.file_path)
            .map_err(|err| legion_protocol::ProjectServiceError {
                code: "resolve_error".to_string(),
                message: err.to_string(),
            })?;

        Ok(legion_protocol::ProjectInfo {
            project_id: ProjectId(state.workspace_id.0),
            root_path: state.root_path.to_string_lossy().into_owned(),
            language_id: None,
            file_id: identity.file_id,
        })
    }

    fn notify_editor_transaction(
        &self,
        event: legion_protocol::EditorTransactionEvent,
    ) -> Result<(), legion_protocol::ProjectServiceError> {
        let _ = &event;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use legion_platform::{
        FileSystemFingerprint, FileSystemMetadata, NativeFileSystem, NativeWatcherService,
    };
    use legion_protocol::{
        WorkspaceOpenRequest, WorkspaceOpened, WorkspacePort, WorkspaceRequest, WorkspaceTrustState,
    };
    use legion_security::{DenyByDefaultBroker, SecurityPolicy};

    static TEST_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn next_test_temp_suffix() -> u64 {
        TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    #[test]
    fn stable_hash_is_deterministic_and_full_width() {
        // FNV-1a is a fixed specification, so these are stable across builds.
        let a = stable_hash("crates/legion-project");
        let b = stable_hash("crates/legion-project");
        assert_eq!(a, b, "stable_hash must be deterministic");
        assert_ne!(
            stable_hash("alpha"),
            stable_hash("beta"),
            "distinct inputs should not collide"
        );
        // Uses the full 128-bit width (DefaultHasher only filled the low 64).
        assert_ne!(
            a >> 64,
            0,
            "high 64 bits should carry entropy from the 128-bit hash"
        );
    }

    #[test]
    fn git_porcelain_status_parses_rename_copy_delete_untracked() {
        // `-z` payload: code+space+space+path, NUL-separated; rename/copy carry a
        // trailing source-path field.
        let payload = concat!(
            " M src/modified.rs\0",
            "?? src/new.rs\0",
            " D src/gone.rs\0",
            "R  src/renamed_to.rs\0src/renamed_from.rs\0",
            "C  src/copied_to.rs\0src/copied_from.rs\0"
        );
        let status = parse_git_porcelain_status(payload);

        assert_eq!(
            status.get("src/modified.rs").map(String::as_str),
            Some(" M")
        );
        assert_eq!(status.get("src/new.rs").map(String::as_str), Some("??"));
        assert_eq!(status.get("src/gone.rs").map(String::as_str), Some(" D"));
        // Rename/copy key on the trailing (source) path; the source field must
        // not leak in as a standalone entry.
        assert_eq!(
            status.get("src/renamed_from.rs").map(String::as_str),
            Some("R ")
        );
        assert_eq!(
            status.get("src/copied_from.rs").map(String::as_str),
            Some("C ")
        );
        assert!(!status.contains_key("src/renamed_to.rs"));
        assert!(!status.contains_key("src/copied_to.rs"));
        assert_eq!(status.len(), 5);
    }

    #[test]
    fn relative_git_path_handles_missing_file_under_existing_parent() {
        let root = std::env::temp_dir().join(format!(
            "legion_project_relative_git_path_missing_{}",
            next_test_temp_suffix()
        ));
        let parent = root.join("src");
        let missing = parent.join("new.rs");
        std::fs::create_dir_all(&parent).expect("create parent");

        assert_eq!(
            relative_git_path(&root, &missing),
            Some("src/new.rs".to_string())
        );
        let relative = Path::new("src").join("new.rs");
        assert_eq!(
            relative_git_path(&root, &relative),
            Some("src/new.rs".to_string())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn relative_git_path_handles_canonicalized_file_under_symlinked_root() {
        let root = std::env::temp_dir().join(format!(
            "legion_project_relative_git_path_{}",
            next_test_temp_suffix()
        ));
        let real_root = root.join("real");
        let link_root = root.join("link");
        let source = real_root.join("src").join("lib.rs");
        std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source");
        std::fs::write(&source, "pub fn test() {}\n").expect("write source");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlink");

        assert_eq!(
            relative_git_path(&link_root, &source),
            Some("src/lib.rs".to_string())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn relative_git_path_handles_missing_file_under_symlinked_root() {
        let root = std::env::temp_dir().join(format!(
            "legion_project_relative_git_path_missing_symlink_{}",
            next_test_temp_suffix()
        ));
        let real_root = root.join("real");
        let link_root = root.join("link");
        let parent = real_root.join("src");
        let missing = parent.join("new.rs");
        std::fs::create_dir_all(&parent).expect("create parent");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlink");

        assert_eq!(
            relative_git_path(&link_root, &missing),
            Some("src/new.rs".to_string())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[derive(Debug)]
    struct FailingAtomicFs {
        root: PathBuf,
        atomic_error: PlatformError,
    }

    impl FailingAtomicFs {
        fn new(root: PathBuf, atomic_error: PlatformError) -> Self {
            Self { root, atomic_error }
        }
    }

    impl PathNormalizationService for FailingAtomicFs {
        fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.normalize_path(path)
        }

        fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.canonicalize_path(path)
        }

        fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
            NativeFileSystem.is_within_base(base, candidate)
        }
    }

    impl FileSystemService for FailingAtomicFs {
        fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.read_text_file(path)
        }

        fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file(path, text)
        }

        fn write_text_file_atomic(&self, _path: &Path, _text: &str) -> Result<(), PlatformError> {
            Err(match &self.atomic_error {
                PlatformError::UnsupportedOperation {
                    operation,
                    path,
                    reason,
                } => PlatformError::UnsupportedOperation {
                    operation: operation.clone(),
                    path: path.clone(),
                    reason: reason.clone(),
                },
                PlatformError::PermissionDenied { operation, path } => {
                    PlatformError::PermissionDenied {
                        operation: operation.clone(),
                        path: path.clone(),
                    }
                }
                _ => PlatformError::UnsupportedOperation {
                    operation: "atomic write".to_string(),
                    path: self.root.join("unknown"),
                    reason: self.atomic_error.to_string(),
                },
            })
        }

        fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
            NativeFileSystem.read_metadata(path)
        }

        fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
            NativeFileSystem.read_fingerprint(path)
        }

        fn stable_hash(&self, bytes: &[u8]) -> String {
            NativeFileSystem.stable_hash(bytes)
        }

        fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.stable_hash_file(path)
        }

        fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
            NativeFileSystem.modified_timestamp(path)
        }

        fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
            NativeFileSystem.file_length(path)
        }

        fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
            NativeFileSystem.list_directory(path)
        }
    }

    #[derive(Debug)]
    struct InconsistentMetadataFs {
        metadata_calls: Mutex<u64>,
    }

    impl InconsistentMetadataFs {
        fn new(_root: PathBuf) -> Self {
            Self {
                metadata_calls: Mutex::new(0),
            }
        }
    }

    impl PathNormalizationService for InconsistentMetadataFs {
        fn normalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.normalize_path(path)
        }

        fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PlatformError> {
            NativeFileSystem.canonicalize_path(path)
        }

        fn is_within_base(&self, base: &Path, candidate: &Path) -> Result<bool, PlatformError> {
            NativeFileSystem.is_within_base(base, candidate)
        }
    }

    impl FileSystemService for InconsistentMetadataFs {
        fn read_text_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.read_text_file(path)
        }

        fn write_text_file(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file(path, text)
        }

        fn write_text_file_atomic(&self, path: &Path, text: &str) -> Result<(), PlatformError> {
            NativeFileSystem.write_text_file_atomic(path, text)
        }

        fn read_metadata(&self, path: &Path) -> Result<FileSystemMetadata, PlatformError> {
            let mut calls = self.metadata_calls.lock().expect("metadata calls lock");
            *calls = calls.saturating_add(1);
            let mut metadata = NativeFileSystem.read_metadata(path)?;
            if *calls > 1 && metadata.is_file() {
                metadata.length = metadata.length.saturating_add(1);
            }
            Ok(metadata)
        }

        fn read_fingerprint(&self, path: &Path) -> Result<FileSystemFingerprint, PlatformError> {
            let metadata = self.read_metadata(path)?;
            let stable_hash = if metadata.is_file() {
                Some(NativeFileSystem.stable_hash_file(path)?)
            } else {
                None
            };
            Ok(FileSystemFingerprint {
                path: path.to_path_buf(),
                algorithm: "inconsistent-test".to_string(),
                kind: metadata.kind,
                length: metadata
                    .is_file()
                    .then_some(metadata.length.saturating_add(1)),
                modified_at: metadata.modified_at,
                stable_hash,
                read_only: metadata.read_only,
            })
        }

        fn stable_hash(&self, bytes: &[u8]) -> String {
            NativeFileSystem.stable_hash(bytes)
        }

        fn stable_hash_file(&self, path: &Path) -> Result<String, PlatformError> {
            NativeFileSystem.stable_hash_file(path)
        }

        fn modified_timestamp(&self, path: &Path) -> Result<Option<u64>, PlatformError> {
            NativeFileSystem.modified_timestamp(path)
        }

        fn file_length(&self, path: &Path) -> Result<u64, PlatformError> {
            NativeFileSystem.file_length(path)
        }

        fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, PlatformError> {
            NativeFileSystem.list_directory(path)
        }
    }

    struct FakeWatcher;
    impl WatcherService for FakeWatcher {
        fn snapshot(
            &self,
            _workspace_id: WorkspaceId,
            _path: &Path,
        ) -> Result<Vec<WatcherEvent>, PlatformError> {
            Err(PlatformError::WatcherOverflow {
                path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                context: "fake overflow".to_string(),
            })
        }
    }

    fn root_workspace() -> (WorkspaceActor, WorkspaceOpened, PrincipalId) {
        let fs: Arc<ProjectFilesystem> = Arc::new(NativeFileSystem);
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::default(),
        );
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("main".to_string()),
            root_path: CanonicalPath(
                std::env::current_dir()
                    .expect("cwd")
                    .to_string_lossy()
                    .into_owned(),
            ),
            trust: Some(WorkspaceTrustState::Trusted),
        };
        let opened = actor.open_workspace(req).expect("open");
        (actor, opened, PrincipalId("main".to_string()))
    }

    fn temporary_workspace(
        trust: WorkspaceTrustState,
    ) -> (WorkspaceActor, WorkspaceOpened, PrincipalId, PathBuf) {
        let base = std::env::temp_dir();
        let unique = format!(
            "legion-project-test-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create temporary workspace directory");
        let canonical_root =
            std::fs::canonicalize(&root).expect("canonicalize temp workspace root");
        let root = canonical_root;
        let canonical_root = root.to_string_lossy().into_owned();

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.clone()];
        policy.path_policy.writable_roots = vec![canonical_root.clone()];

        let actor = WorkspaceActor::new(
            Arc::new(NativeFileSystem),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                legion_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(3),
            principal_id: PrincipalId("temp-principal".to_string()),
            root_path: CanonicalPath(canonical_root.clone()),
            trust: Some(trust),
        };

        let opened = actor.open_workspace(req).expect("open temporary workspace");
        (
            actor,
            opened,
            PrincipalId("temp-principal".to_string()),
            root,
        )
    }

    #[allow(clippy::result_large_err)]
    fn save_new_file_for_tests(
        actor: &WorkspaceActor,
        workspace_id: WorkspaceId,
        path: &str,
        text: &str,
    ) -> WorkspaceSaveResult {
        let opened = actor
            .open_new_file_text(workspace_id, path)
            .expect("open new file for proposal save");
        actor.save_file_with_proposal(WorkspaceSaveRequest {
            workspace_id,
            proposal_id: ProposalId(1),
            principal: PrincipalId("temp-principal".to_string()),
            required_capability: CapabilityId("fs.write".to_string()),
            file_id: opened.identity.file_id,
            path: opened.identity.canonical_path,
            expected_fingerprint: opened.fingerprint,
            expected_file_content_version: opened.file_content_version,
            expected_workspace_generation: opened.workspace_generation,
            buffer_version: BufferVersion(1),
            snapshot_id: SnapshotId(1),
            payload_byte_len: text.len() as u64,
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(Uuid::now_v7()),
            text: text.to_string(),
        })
    }

    fn rollback_checkpoint_request(
        workspace_id: WorkspaceId,
        proposal_id: ProposalId,
        principal: &PrincipalId,
        target: WorkspaceMutationRollbackTarget,
    ) -> WorkspaceMutationRollbackCheckpointRequest {
        WorkspaceMutationRollbackCheckpointRequest {
            workspace_id,
            proposal_id,
            principal: principal.clone(),
            required_capability: CapabilityId("fs.write".to_string()),
            target,
            correlation_id: CorrelationId(77),
            causality_id: CausalityId(Uuid::now_v7()),
        }
    }

    fn rollback_request(
        workspace_id: WorkspaceId,
        proposal_id: ProposalId,
        principal: &PrincipalId,
        checkpoint: WorkspaceMutationRollbackCheckpoint,
    ) -> WorkspaceMutationRollbackRequest {
        WorkspaceMutationRollbackRequest {
            workspace_id,
            proposal_id,
            principal: principal.clone(),
            required_capability: CapabilityId("fs.write".to_string()),
            checkpoint,
            correlation_id: CorrelationId(78),
            causality_id: CausalityId(Uuid::now_v7()),
        }
    }

    #[test]
    fn open_and_resolve_path_stays_inside_root() {
        let (actor, opened, _) = root_workspace();
        let root = std::env::current_dir().expect("cwd");
        let child = root.join("Cargo.toml");
        let identity = actor
            .resolve_file(opened.workspace_id, child.to_string_lossy())
            .expect("resolved");
        assert_eq!(identity.workspace_id, opened.workspace_id);
    }

    #[test]
    fn request_port_open_roundtrip_works() {
        let (actor, _, principal) = root_workspace();
        let response = actor
            .handle(WorkspaceRequest::ReadConfig(WorkspaceId(0)))
            .expect_err("read config should fail when workspace id mismatch");
        assert_eq!(response.code, "workspace_error");
        let _ = principal;
    }

    #[test]
    fn watcher_overflow_marks_recovery() {
        let fs: Arc<ProjectFilesystem> = Arc::new(NativeFileSystem);
        let actor = WorkspaceActor::new(fs, Arc::new(FakeWatcher), DenyByDefaultBroker::default());
        let req = WorkspaceOpenRequest {
            correlation_id: CorrelationId(1),
            principal_id: PrincipalId("u".to_string()),
            root_path: CanonicalPath(
                std::env::current_dir()
                    .expect("cwd")
                    .to_string_lossy()
                    .into_owned(),
            ),
            trust: Some(WorkspaceTrustState::Trusted),
        };
        let opened = actor.open_workspace(req).expect("open");
        actor.set_watchers_for_tests();
        let events = actor
            .poll_watcher_events(opened.workspace_id)
            .expect("poll");
        assert!(!events.is_empty());
        assert!(matches!(
            events[0].kind,
            legion_protocol::WatcherEventKind::Overflow
        ));
    }

    #[test]
    fn path_policy_rejects_outside_root_reads_and_writes() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Trusted);
        let outside = root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("outside-policy-check.txt");

        let read_err = actor
            .resolve_file(opened.workspace_id, outside.to_string_lossy())
            .expect_err("resolve should fail for outside root");
        assert!(matches!(read_err, WorkspaceError::PathOutsideRoot { .. }));

        let write_err = actor
            .open_new_file_text(opened.workspace_id, outside.to_string_lossy())
            .expect_err("new-file open should fail for outside root");
        assert!(matches!(write_err, WorkspaceError::PathOutsideRoot { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_is_blocked_for_untrusted_workspace() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Untrusted);
        let file_path = "blocked.txt";

        let write_err = actor
            .open_new_file_text(opened.workspace_id, file_path)
            .expect_err("untrusted workspace should not be able to create a new-file buffer");

        assert!(matches!(write_err, WorkspaceError::SecurityDenied { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_write_roundtrip_from_workspace_apis() {
        let (actor, opened, _, root) = temporary_workspace(WorkspaceTrustState::Trusted);
        let file_path = "integration.txt";

        let applied = save_new_file_for_tests(&actor, opened.workspace_id, file_path, "hello\n")
            .expect("proposal save via actor should succeed");
        assert!(!applied.used_non_atomic_fallback);
        assert_eq!(
            applied.fallback_status.as_deref(),
            Some("atomic-write-only; non-atomic fallback disabled")
        );

        let actual = actor
            .read_file_text(opened.workspace_id, file_path)
            .expect("read via actor should succeed");
        assert_eq!(actual, "hello\n");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rename_file_with_proposal_requires_destination_write_authorization() {
        let base = std::env::temp_dir();
        let unique = format!(
            "legion-project-rename-policy-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        let allowed = root.join("allowed");
        let blocked = root.join("blocked");
        std::fs::create_dir_all(&allowed).expect("create allowed directory");
        std::fs::create_dir_all(&blocked).expect("create blocked directory");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let allowed = std::fs::canonicalize(&allowed).expect("canonicalize allowed directory");
        let blocked = std::fs::canonicalize(&blocked).expect("canonicalize blocked directory");
        let source = allowed.join("source.txt");
        let destination = blocked.join("destination.txt");
        std::fs::write(&source, "seed").expect("seed source file");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![allowed.to_string_lossy().into_owned()];
        let actor = WorkspaceActor::new(
            Arc::new(NativeFileSystem),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                legion_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let principal = PrincipalId("temp-principal".to_string());
        let opened = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(21),
                principal_id: principal.clone(),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect("open workspace");
        let opened_file = actor
            .open_existing_file_text(opened.workspace_id, source.to_string_lossy())
            .expect("open source file");

        let response = actor
            .rename_file_with_proposal(WorkspaceRenameFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(205),
                principal,
                required_capability: CapabilityId("fs.write".to_string()),
                file: opened_file.identity.clone(),
                destination: CanonicalPath(destination.to_string_lossy().into_owned()),
                expected_fingerprint: opened_file.fingerprint.clone(),
                expected_file_content_version: opened_file.file_content_version,
                expected_workspace_generation: opened_file.workspace_generation,
                correlation_id: CorrelationId(205),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect_err("destination write policy should deny rename");

        assert!(matches!(response, ProposalResponse::Denied { .. }));
        assert!(source.exists());
        assert!(!destination.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rollback_checkpoints_compensate_file_mutations_through_workspace_authority() {
        let (actor, opened, principal, root) = temporary_workspace(WorkspaceTrustState::Trusted);

        let create_path = CanonicalPath(
            root.join("rollback-created.txt")
                .to_string_lossy()
                .into_owned(),
        );
        let create_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(201),
                &principal,
                WorkspaceMutationRollbackTarget::CreatedFile {
                    path: create_path.clone(),
                },
            ))
            .expect("capture create rollback checkpoint");
        actor
            .create_file_with_proposal(WorkspaceCreateFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(201),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                path: create_path.clone(),
                expected_workspace_generation: opened.generation,
                initial_content: "created".to_string(),
                correlation_id: CorrelationId(201),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect("create through workspace proposal");
        actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(201),
                &principal,
                create_checkpoint,
            ))
            .expect("rollback created file");
        assert!(!Path::new(&create_path.0).exists());

        let delete_path = root.join("rollback-delete.txt");
        std::fs::write(&delete_path, "delete seed").expect("seed delete file");
        let opened_delete = actor
            .open_existing_file_text(opened.workspace_id, delete_path.to_string_lossy())
            .expect("open delete target");
        let delete_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(202),
                &principal,
                WorkspaceMutationRollbackTarget::DeletedFile {
                    file: opened_delete.identity.clone(),
                },
            ))
            .expect("capture delete rollback checkpoint");
        actor
            .delete_file_with_proposal(WorkspaceDeleteFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(202),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                file: opened_delete.identity.clone(),
                expected_fingerprint: opened_delete.fingerprint.clone(),
                expected_file_content_version: opened_delete.file_content_version,
                expected_workspace_generation: opened_delete.workspace_generation,
                correlation_id: CorrelationId(202),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect("delete through workspace proposal");
        assert!(!delete_path.exists());
        actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(202),
                &principal,
                delete_checkpoint,
            ))
            .expect("rollback deleted file");
        assert_eq!(
            std::fs::read_to_string(&delete_path).expect("read restored delete file"),
            "delete seed"
        );

        let rename_source = root.join("rollback-rename.txt");
        let rename_destination = CanonicalPath(
            root.join("rollback-renamed.txt")
                .to_string_lossy()
                .into_owned(),
        );
        std::fs::write(&rename_source, "rename seed").expect("seed rename file");
        let opened_rename = actor
            .open_existing_file_text(opened.workspace_id, rename_source.to_string_lossy())
            .expect("open rename target");
        let rename_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(203),
                &principal,
                WorkspaceMutationRollbackTarget::RenamedFile {
                    file: opened_rename.identity.clone(),
                    destination: rename_destination.clone(),
                },
            ))
            .expect("capture rename rollback checkpoint");
        actor
            .rename_file_with_proposal(WorkspaceRenameFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(203),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                file: opened_rename.identity.clone(),
                destination: rename_destination.clone(),
                expected_fingerprint: opened_rename.fingerprint.clone(),
                expected_file_content_version: opened_rename.file_content_version,
                expected_workspace_generation: opened_rename.workspace_generation,
                correlation_id: CorrelationId(203),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect("rename through workspace proposal");
        assert!(!rename_source.exists());
        assert!(Path::new(&rename_destination.0).exists());
        actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(203),
                &principal,
                rename_checkpoint,
            ))
            .expect("rollback renamed file");
        assert_eq!(
            std::fs::read_to_string(&rename_source).expect("read restored rename file"),
            "rename seed"
        );
        assert!(!Path::new(&rename_destination.0).exists());

        let save_path = root.join("rollback-save.txt");
        std::fs::write(&save_path, "save seed").expect("seed save file");
        let opened_save = actor
            .open_existing_file_text(opened.workspace_id, save_path.to_string_lossy())
            .expect("open save target");
        let save_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(204),
                &principal,
                WorkspaceMutationRollbackTarget::SavedFile {
                    file: opened_save.identity.clone(),
                },
            ))
            .expect("capture save rollback checkpoint");
        actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(204),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_save.identity.file_id,
                path: opened_save.identity.canonical_path.clone(),
                expected_fingerprint: opened_save.fingerprint.clone(),
                expected_file_content_version: opened_save.file_content_version,
                expected_workspace_generation: opened_save.workspace_generation,
                buffer_version: BufferVersion(4),
                snapshot_id: SnapshotId(4),
                payload_byte_len: "save mutated".len() as u64,
                correlation_id: CorrelationId(204),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "save mutated".to_string(),
            })
            .expect("save through workspace proposal");
        actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(204),
                &principal,
                save_checkpoint,
            ))
            .expect("rollback saved file");
        assert_eq!(
            std::fs::read_to_string(&save_path).expect("read restored save file"),
            "save seed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rollback_checkpoint_refuses_to_clobber_external_changes() {
        let (actor, opened, principal, root) = temporary_workspace(WorkspaceTrustState::Trusted);

        let create_path = CanonicalPath(
            root.join("rollback-created-external.txt")
                .to_string_lossy()
                .into_owned(),
        );
        let create_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(301),
                &principal,
                WorkspaceMutationRollbackTarget::CreatedFile {
                    path: create_path.clone(),
                },
            ))
            .expect("capture create rollback checkpoint");
        actor
            .create_file_with_proposal(WorkspaceCreateFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(301),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                path: create_path.clone(),
                expected_workspace_generation: opened.generation,
                initial_content: "created".to_string(),
                correlation_id: CorrelationId(301),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect("create through workspace proposal");
        std::fs::write(&create_path.0, "external replacement")
            .expect("external create target replacement");
        let create_response = actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(301),
                &principal,
                create_checkpoint,
            ))
            .expect_err("rollback should not delete externally changed create target");
        assert!(matches!(create_response, ProposalResponse::Failed { .. }));
        assert_eq!(
            std::fs::read_to_string(&create_path.0).expect("read external create target"),
            "external replacement"
        );

        let save_path = root.join("rollback-save-external.txt");
        std::fs::write(&save_path, "save seed").expect("seed save file");
        let opened_save = actor
            .open_existing_file_text(opened.workspace_id, save_path.to_string_lossy())
            .expect("open save target");
        let save_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(302),
                &principal,
                WorkspaceMutationRollbackTarget::SavedFile {
                    file: opened_save.identity.clone(),
                },
            ))
            .expect("capture save rollback checkpoint");
        actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(302),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_save.identity.file_id,
                path: opened_save.identity.canonical_path.clone(),
                expected_fingerprint: opened_save.fingerprint.clone(),
                expected_file_content_version: opened_save.file_content_version,
                expected_workspace_generation: opened_save.workspace_generation,
                buffer_version: BufferVersion(5),
                snapshot_id: SnapshotId(5),
                payload_byte_len: "save mutated".len() as u64,
                correlation_id: CorrelationId(302),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "save mutated".to_string(),
            })
            .expect("save through workspace proposal");
        std::fs::write(&save_path, "external save replacement")
            .expect("external save target replacement");
        let save_response = actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(302),
                &principal,
                save_checkpoint,
            ))
            .expect_err("rollback should not overwrite externally changed save target");
        assert!(matches!(save_response, ProposalResponse::Failed { .. }));
        assert_eq!(
            std::fs::read_to_string(&save_path).expect("read external save target"),
            "external save replacement"
        );

        let rename_source = root.join("rollback-rename-external.txt");
        let rename_destination = CanonicalPath(
            root.join("rollback-renamed-external.txt")
                .to_string_lossy()
                .into_owned(),
        );
        std::fs::write(&rename_source, "rename seed").expect("seed rename file");
        let opened_rename = actor
            .open_existing_file_text(opened.workspace_id, rename_source.to_string_lossy())
            .expect("open rename target");
        let rename_checkpoint = actor
            .rollback_checkpoint_for_file_mutation(rollback_checkpoint_request(
                opened.workspace_id,
                ProposalId(303),
                &principal,
                WorkspaceMutationRollbackTarget::RenamedFile {
                    file: opened_rename.identity.clone(),
                    destination: rename_destination.clone(),
                },
            ))
            .expect("capture rename rollback checkpoint");
        actor
            .rename_file_with_proposal(WorkspaceRenameFileRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(303),
                principal: principal.clone(),
                required_capability: CapabilityId("fs.write".to_string()),
                file: opened_rename.identity.clone(),
                destination: rename_destination.clone(),
                expected_fingerprint: opened_rename.fingerprint.clone(),
                expected_file_content_version: opened_rename.file_content_version,
                expected_workspace_generation: opened_rename.workspace_generation,
                correlation_id: CorrelationId(303),
                causality_id: CausalityId(Uuid::now_v7()),
            })
            .expect("rename through workspace proposal");
        std::fs::write(&rename_destination.0, "external rename replacement")
            .expect("external rename target replacement");
        let rename_response = actor
            .rollback_file_mutation_with_checkpoint(rollback_request(
                opened.workspace_id,
                ProposalId(303),
                &principal,
                rename_checkpoint,
            ))
            .expect_err("rollback should not rename externally changed destination");
        assert!(matches!(rename_response, ProposalResponse::Failed { .. }));
        assert!(!rename_source.exists());
        assert_eq!(
            std::fs::read_to_string(&rename_destination.0).expect("read external rename target"),
            "external rename replacement"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_write_failure_fails_closed_without_plain_write() {
        let base = std::env::temp_dir();
        let unique = format!(
            "legion-project-atomic-failure-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create atomic failure workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("atomic-failure.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let fs: Arc<ProjectFilesystem> = Arc::new(FailingAtomicFs::new(
            canonical_root.clone(),
            PlatformError::UnsupportedOperation {
                operation: "atomic replace".to_string(),
                path: target.clone(),
                reason: "synthetic unsupported atomic replacement".to_string(),
            },
        ));
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                legion_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let opened = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(11),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect("open workspace");
        let opened_file = actor
            .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
            .expect("open target");

        let response = actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(99),
                principal: PrincipalId("temp-principal".to_string()),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_file.identity.file_id,
                path: opened_file.identity.canonical_path,
                expected_fingerprint: opened_file.fingerprint,
                expected_file_content_version: opened_file.file_content_version,
                expected_workspace_generation: opened_file.workspace_generation,
                buffer_version: BufferVersion(2),
                snapshot_id: SnapshotId(2),
                payload_byte_len: "replacement".len() as u64,
                correlation_id: CorrelationId(99),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "replacement".to_string(),
            })
            .expect_err("atomic write failure should fail closed");

        assert_eq!(
            std::fs::read_to_string(&target).expect("target content"),
            "seed"
        );
        match response {
            ProposalResponse::Failed { transition, .. } => {
                assert!(transition.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("non-atomic fallback disabled; failing closed")
                }));
            }
            other => panic!("expected failed response, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn permission_failure_from_platform_is_reported_without_plain_write() {
        let base = std::env::temp_dir();
        let unique = format!(
            "legion-project-permission-failure-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create permission failure workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("permission-failure.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let fs: Arc<ProjectFilesystem> = Arc::new(FailingAtomicFs::new(
            canonical_root.clone(),
            PlatformError::PermissionDenied {
                operation: "atomic write".to_string(),
                path: target.clone(),
            },
        ));
        let actor = WorkspaceActor::new(
            fs,
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                legion_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let opened = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(12),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect("open workspace");
        let opened_file = actor
            .open_existing_file_text(opened.workspace_id, target.to_string_lossy())
            .expect("open target");

        let response = actor
            .save_file_with_proposal(WorkspaceSaveRequest {
                workspace_id: opened.workspace_id,
                proposal_id: ProposalId(100),
                principal: PrincipalId("temp-principal".to_string()),
                required_capability: CapabilityId("fs.write".to_string()),
                file_id: opened_file.identity.file_id,
                path: opened_file.identity.canonical_path,
                expected_fingerprint: opened_file.fingerprint,
                expected_file_content_version: opened_file.file_content_version,
                expected_workspace_generation: opened_file.workspace_generation,
                buffer_version: BufferVersion(2),
                snapshot_id: SnapshotId(2),
                payload_byte_len: "replacement".len() as u64,
                correlation_id: CorrelationId(100),
                causality_id: CausalityId(Uuid::now_v7()),
                text: "replacement".to_string(),
            })
            .expect_err("permission failure should fail closed");

        assert_eq!(
            std::fs::read_to_string(&target).expect("target content"),
            "seed"
        );
        match response {
            ProposalResponse::Failed { transition, .. } => {
                assert!(
                    transition
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("permission denied"))
                );
            }
            other => panic!("expected failed response, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn metadata_inconsistency_blocks_open_before_save_preconditions() {
        let base = std::env::temp_dir();
        let unique = format!(
            "legion-project-metadata-inconsistent-{}-{}-{}",
            std::process::id(),
            now_millis(),
            next_test_temp_suffix()
        );
        let root = base.join(unique);
        std::fs::create_dir_all(&root).expect("create metadata workspace");
        let canonical_root = std::fs::canonicalize(&root).expect("canonicalize root");
        let target = canonical_root.join("metadata.txt");
        std::fs::write(&target, "seed").expect("seed target");

        let mut policy = SecurityPolicy::default();
        policy.path_policy.readable_roots = vec![canonical_root.to_string_lossy().into_owned()];
        policy.path_policy.writable_roots = vec![canonical_root.to_string_lossy().into_owned()];

        let actor = WorkspaceActor::new(
            Arc::new(InconsistentMetadataFs::new(canonical_root.clone())),
            Arc::new(NativeWatcherService),
            DenyByDefaultBroker::new(
                policy,
                legion_protocol::CapabilityNamespace("test".to_string()),
            ),
        );
        let err = actor
            .open_workspace(WorkspaceOpenRequest {
                correlation_id: CorrelationId(13),
                principal_id: PrincipalId("temp-principal".to_string()),
                root_path: CanonicalPath(canonical_root.to_string_lossy().into_owned()),
                trust: Some(WorkspaceTrustState::Trusted),
            })
            .expect_err("metadata inconsistency should block workspace open");
        assert!(matches!(
            err,
            WorkspaceError::Platform(PlatformError::MetadataInconsistent { .. })
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    impl WorkspaceActor {
        fn set_watchers_for_tests(&self) {
            let mut state = self.state.lock().expect("lock");
            if let Some(state) = state.as_mut() {
                state.watcher_sequence += 1;
            }
        }
    }
}
