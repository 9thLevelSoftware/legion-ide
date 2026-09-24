//! Language toolchain settings and language tooling projection DTOs.
//!
//! Extracted verbatim from `lib.rs` under the chokepoint rule: every type
//! below is the same bytes that lived in `lib.rs`, and `lib.rs` re-exports
//! this module with `pub use language_toolchain::*` so
//! `legion_protocol::LanguageToolchainSettingsRecord` and every other name
//! here keeps resolving exactly as before. The only addition made after the
//! move is [`PythonToolchainSettings`] and the `python` field it fills.

use serde::{Deserialize, Serialize};

use crate::{
    BufferId, CallHierarchyDirection, CanonicalPath, FileId, LanguageBreadcrumbProjection,
    LanguageCodeActionProjection, LanguageCodeLensProjection, LanguageCompletionProjection,
    LanguageHoverProjection, LanguageInlayHintProjection, LanguageLocationProjection,
    LanguageOutlineSymbolProjection, LanguageProblemProjection, LanguageQuickFixProjection,
    LanguageStickyScopeProjection, LanguageToolingOperationProjection, LanguageToolingStatusKind,
    LspServerHealthRecord, LspSessionLogProjection, LspSessionStatusProjection, RedactionHint,
    TimestampMillis, WorkspaceId,
};

/// Runtime configuration status for a language toolchain projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LanguageToolchainConfigurationStatus {
    /// No local toolchain configuration has been selected.
    #[default]
    Unconfigured,
    /// Metadata has been selected but has not completed validation.
    Draft,
    /// The app has validated the selected metadata for use in this session.
    Configured,
}

/// Runtime-only TypeScript toolchain projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeScriptToolchainProjection {
    /// Selected metadata-only settings, when present.
    #[serde(default)]
    pub settings: Option<TypeScriptToolchainSettings>,
    /// Current app-owned validation state.
    #[serde(default)]
    pub status: LanguageToolchainConfigurationStatus,
}

impl Default for TypeScriptToolchainProjection {
    fn default() -> Self {
        Self {
            settings: None,
            status: LanguageToolchainConfigurationStatus::Unconfigured,
        }
    }
}

/// Projection-only language tooling panel state for the active editor buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolingProjection {
    /// Runtime-only TypeScript toolchain configuration projection.
    #[serde(default)]
    pub typescript_toolchain: TypeScriptToolchainProjection,
    /// Workspace represented by the projection, when one is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active editor buffer represented by the projection, when one is open.
    pub buffer_id: Option<BufferId>,
    /// Active file represented by the projection, when one is open.
    pub file_id: Option<FileId>,
    /// High-level projection status.
    pub status: LanguageToolingStatusKind,
    /// Bounded metadata-only status message.
    pub status_message: String,
    /// Diagnostic/problem rows.
    pub problems: Vec<LanguageProblemProjection>,
    /// Quick-fix rows derived from diagnostic/problem rows.
    pub quick_fixes: Vec<LanguageQuickFixProjection>,
    /// Bounded metadata-only live code-action candidates.
    #[serde(default)]
    pub code_action_candidates: Vec<LanguageCodeActionProjection>,
    /// Breadcrumb rows for the active cursor position.
    pub breadcrumbs: Vec<LanguageBreadcrumbProjection>,
    /// Sticky scope rows for the active cursor position.
    pub sticky_scopes: Vec<LanguageStickyScopeProjection>,
    /// Inlay hint rows for the active buffer.
    pub inlay_hints: Vec<LanguageInlayHintProjection>,
    /// Code lens rows for the active buffer.
    pub code_lenses: Vec<LanguageCodeLensProjection>,
    /// Current hover result.
    pub hover: Option<LanguageHoverProjection>,
    /// Current completion rows.
    pub completions: Vec<LanguageCompletionProjection>,
    /// Current definition locations.
    pub definitions: Vec<LanguageLocationProjection>,
    /// Current reference locations.
    pub references: Vec<LanguageLocationProjection>,
    /// Current call-hierarchy rows, in the direction last asked for.
    pub call_hierarchy: Vec<LanguageLocationProjection>,
    /// Direction the call-hierarchy rows answer, when any have been requested.
    pub call_hierarchy_direction: Option<CallHierarchyDirection>,
    /// Whether a call-hierarchy question has been asked and not yet answered.
    ///
    /// Three states, not two. Empty rows with a direction means "the server
    /// answered: nobody calls this", which is a real answer and must be shown
    /// as one. Empty rows while waiting is a different thing entirely, and
    /// rendering it as the first would state a conclusion the product does not
    /// have — permanently, if the answer never arrives because the server
    /// lacks the capability or the caret was on whitespace.
    pub call_hierarchy_awaiting: bool,
    /// Current outline rows.
    pub outline: Vec<LanguageOutlineSymbolProjection>,
    /// Recent operation status rows.
    pub operations: Vec<LanguageToolingOperationProjection>,
    /// Count of stale results discarded before projection.
    pub stale_result_count: u32,
    /// Count of cancellation acknowledgements projected.
    pub cancellation_count: u32,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints for the whole projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
    /// Live LSP server health records for the active workspace (D2).
    ///
    /// Populated by `AppComposition::shell_projection_snapshot()` from the
    /// background `LspSessionHandle`.  Empty when no LSP session is active.
    /// `lsp_health_rows()` in `legion-desktop` renders these into the
    /// language tooling status section.
    #[serde(default)]
    pub lsp_health_records: Vec<LspServerHealthRecord>,
    /// LSP session lifecycle status including backoff countdown (PKT-LSP-C T3).
    ///
    /// `Some` once a session has been attempted (Starting/Live/BackingOff/
    /// Refused/Failed); `None` when the session is `Idle` (no startup yet).
    #[serde(default)]
    pub lsp_session_status: Option<LspSessionStatusProjection>,
    /// Redacted ring-buffer projection of the LSP server stderr (PKT-LSP-C T4).
    ///
    /// `Some` only when the session is `Live` and the ring contains at least
    /// one line.  `None` when the session is `Idle`, `Starting`, or failed,
    /// or when no stderr output has been received yet.
    #[serde(default)]
    pub lsp_session_log: Option<LspSessionLogProjection>,
}

impl LanguageToolingProjection {
    /// Construct an empty language tooling projection.
    pub fn empty() -> Self {
        Self {
            typescript_toolchain: TypeScriptToolchainProjection::default(),
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            status: LanguageToolingStatusKind::Idle,
            status_message: "Language tooling idle".to_string(),
            problems: Vec::new(),
            quick_fixes: Vec::new(),
            code_action_candidates: Vec::new(),
            breadcrumbs: Vec::new(),
            sticky_scopes: Vec::new(),
            inlay_hints: Vec::new(),
            code_lenses: Vec::new(),
            hover: None,
            completions: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            call_hierarchy: Vec::new(),
            call_hierarchy_direction: None,
            call_hierarchy_awaiting: false,
            outline: Vec::new(),
            operations: Vec::new(),
            stale_result_count: 0,
            cancellation_count: 0,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
            lsp_health_records: Vec::new(),
            lsp_session_status: None,
            lsp_session_log: None,
        }
    }
}

impl Default for LanguageToolingProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// Persisted metadata-only settings for locally configured language toolchains.
///
/// This describes operator-selected paths for later app-owned validation. It
/// is not an authorization, grant, receipt, or artifact integrity record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolchainSettingsRecord {
    /// DTO schema version.
    pub schema_version: u16,
    /// Optional TypeScript bundle settings.
    #[serde(default)]
    pub typescript: Option<TypeScriptToolchainSettings>,
    /// Optional Python interpreter/formatter settings.
    ///
    /// Absent sections are skipped on serialization so a record written
    /// before this field existed and a record written after it with no
    /// Python configuration produce byte-identical JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python: Option<PythonToolchainSettings>,
}

impl Default for LanguageToolchainSettingsRecord {
    fn default() -> Self {
        Self {
            schema_version: 1,
            typescript: None,
            python: None,
        }
    }
}

/// Metadata-only paths for a TypeScript language-server/compiler bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeScriptToolchainSettings {
    /// Local language-server archive path.
    pub server_archive: CanonicalPath,
    /// Local compiler archive path.
    pub compiler_archive: CanonicalPath,
    /// Explicit Node runtime path.
    pub node_executable: CanonicalPath,
}

/// Metadata-only paths for a locally selected Python toolchain.
///
/// Both paths are operator-selected canonical absolute paths. Neither is a
/// grant, a receipt, or permission to spawn anything; the app records them so
/// a later launch never has to consult `PATH`. A bare executable name is not
/// representable here by construction: the app refuses one before it reaches
/// this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonToolchainSettings {
    /// Explicit Python interpreter executable path.
    pub interpreter_executable: CanonicalPath,
    /// Explicit Python formatter executable path.
    pub formatter_executable: CanonicalPath,
}
