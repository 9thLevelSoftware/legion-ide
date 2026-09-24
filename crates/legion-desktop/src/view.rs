//! Projection rendering for the desktop adapter.

mod about;
#[cfg(feature = "ai")]
mod assistant_rail;
mod brand_mark;
/// Call-hierarchy rows for the language section: direction and degraded marking.
mod call_hierarchy;
/// Snapshot-backed status, language, trust, and assistant rows.
mod status_rows;
use status_rows::*;
mod code_canvas_painter;
mod components;
/// Debug call-stack and variable inspector.
mod debug_inspector;
/// Applying persisted dock splitter fractions, and observing new ones.
pub mod dock_geometry;
#[cfg(feature = "ai")]
pub mod ghost_text;
pub mod rail_icons;
/// Source-control panel: status rows, remote verbs, and per-hunk staging.
mod source_control;
/// Bounded source-to-atlas layout for oversized logical lines.
mod streamed_layout;
#[cfg(test)]
#[path = "view/streamed_multiline_projection_tests.rs"]
mod streamed_multiline_projection_tests;
/// The editor tab strip: tabs, close affordance, drag-to-reorder.
mod tab_strip;
/// Test explorer tree for the Tests surface.
mod test_explorer;

use call_hierarchy::call_hierarchy_rows;
use debug_inspector::{
    DEBUG_STACK_FRAME_RENDER_LIMIT, debug_frame_navigation_action,
    debug_selected_stack_frame_index, render_debug_inspector, set_debug_selected_stack_frame_index,
};
use source_control::{
    active_git_relative_path, git_hunk_marker_for_line, git_inline_blame_label,
    git_next_hunk_cursor, git_previous_hunk_cursor, git_rows, render_git_controls,
};
use tab_strip::render_tab_strip;
use test_explorer::render_test_explorer_tree;

use proposal_cards::render_proposal_cards;

/// Agent communication row parsing and rendering.
pub mod agent_comm;
/// Files as draggable cards in an infinite 2D space.
pub mod canvas_workspace;
mod keymap_dispatch;

pub(crate) use keymap_dispatch::*;
/// Install / update / remove controls for signed extension artifacts (P7.F2).
pub mod assist_rail_commands;
pub mod cloud_lane;
pub mod extensions_panel;
/// Projection-backed Legion workflow board.
pub mod fleet_board;
/// Projection-backed Legion workflow cards.
pub mod fleet_card;
/// Inline edit diff overlay view model and per-hunk accept/reject helpers (PKT-INLINE).
pub mod inline_edit;
/// Interactive text fields (terminal input, BYOK, settings paths) outside the code-canvas gate.
pub(crate) mod interactive_fields;
/// Pre-invocation context manifest panel with per-item exclusion toggles.
pub mod manifest_panel;
/// Editable plan editor projection.
pub mod plan_editor;
/// The proposal ledger rendered as Approve / Review / Reject cards.
pub mod proposal_cards;
/// Proposal review and checkpoint timeline view models.
pub mod proposal_review;
/// Risk strip view model and row projections for proposal review surfaces.
pub mod risk_strip;
/// Sandbox panel projection.
pub mod sandbox_panel;
/// Renderer-backed scope picker for delegated tasks.
pub mod scope_picker;
/// Terminal panel render-model helpers.
pub mod terminal_panel;
/// Worker panel view model and renderer for active delegated task monitoring.
pub mod worker_panel;

#[cfg(feature = "ai")]
pub use assistant_rail::{
    AssistantRailCodeBlockViewModel, AssistantRailCommandViewModel, AssistantRailRowViewModel,
    AssistantRailSegmentViewModel, assistant_rail_rows, bind_proposals_to_blocks,
    rail_command_view_models, render_streaming_assistant_rows, streaming_rail_rows,
};
#[cfg(feature = "ai")]
pub use ghost_text::{GhostTextOverlayViewModel, GhostTextState, ghost_text_from_prediction};
pub use inline_edit::{
    InlineEditApplyResult, InlineEditError, InlineEditOverlayState, InlineEditOverlayViewModel,
    accumulate_inline_edit_chunks, apply_inline_edit_with_undo_group,
    build_inline_edit_audit_record, check_inline_edit_anchor_freshness,
    inline_edit_from_instruction, inline_edit_to_workspace_proposal,
    set_inline_edit_hunk_disposition,
};
pub use manifest_panel::{
    DesktopManifestItemToggleViewModel, manifest_item_toggle_view_models, preview_rows,
    toggle_manifest_item_inclusion,
};
pub use plan_editor::{
    DesktopPlanEditorViewModel, DesktopPlanSectionViewModel, edited_sections_from_plan_editor_draft,
};
pub use risk_strip::{DesktopProposalRiskStripViewModel, risk_strip_rows, risk_strip_view_model};
pub use scope_picker::{DesktopScopePickerViewModel, ScopeRiskTolerance, ScopeTargetKind};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use code_canvas_painter::{CodeCanvasPainter, EguiCodeCanvasPainter, semantic_token_color};
use components::{
    disclosure_row, empty_state, prerequisite_card, primary_button, primary_button_enabled,
    section_header as section_label, segmented_tab as console_tab, selectable_pill_button,
    soft_button, status_badge as pill, surface_card, top_bar_command_button,
};

use legion_protocol::{
    AssistedAiProviderAvailabilityState, BufferId, ByteRange, CANONICAL_PRODUCT_MODES,
    CanonicalPath, CanonicalProductMode, CaretAffinity, ContextManifestEgressStatus,
    ContextManifestInclusionState, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRiskTolerance, DelegatedTaskRuntimeActivationState, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, DelegatedTaskToolPermissionDecision, FileId,
    LanguageCodeActionProjection, LanguageInlayHintProjection, LanguageLocationProjection,
    LanguageProblemProjection, LanguageToolchainConfigurationStatus, LegionToolKind,
    LineWrappingPolicy, PluginCommandDescriptor, PluginContribution, PluginContributionProjection,
    PrivacyInspectorRedactionState, ProposalId, ProposalLifecycleState, ProposalRejectionReason,
    ProposalRiskLabel, ProtocolDiagnosticSeverity, ProtocolTextRange, TextCoordinate,
    TypeScriptToolchainProjection, Utf16Range, ViewportLineTruncationState, ViewportProjectionMode,
    ViewportScroll, ViewportSemanticTokenKind, ViewportSemanticTokenOverlay,
    VisualNavigationPosition, VisualNavigationRow, VisualNavigationSourceRow, VisualNavigationStop,
    VisualNavigationX, WorkspaceId,
};
use legion_ui::{
    ActiveBufferProjection, DebugStepKindProjection, DockLayout, DockMode, DockSide,
    DockSideLayout, PaletteMode, PaletteProjection, PaletteResultKind, PanelId, PanelRegistry,
    SearchScopeProjection, SettingsProjection, ShellProjectionSnapshot, StatusSeverity,
    ThemePreferenceProjection, ToastActionProjection, ToastStackProjection,
    ToastVerbosityProjection, palette_command_group,
};

use crate::{
    bridge::DesktopAction, health::DesktopOperationalHealthSnapshot,
    search::DesktopSearchViewModel, theme,
};

pub(crate) use streamed_layout::{
    DesktopLineChunk, DesktopLineSource, DesktopSourceIdentity, StreamedLayoutCachePool,
    StreamedNavigationRequest, StreamedNavigationRows, StreamedRequestedRow,
};

const COMMAND_PALETTE_VISIBLE_RESULT_ROWS: usize = 10;
const LEGION_WORDMARK: &str = "Legion";
/// Minimum actual editor allocation retained below an expanded center workbench.
const MIN_USABLE_EDITOR_HEIGHT: f32 = 180.0;
/// Space retained for tabs, breadcrumbs, and editor-frame margins above the code canvas.
const EDITOR_CHROME_HEIGHT_RESERVE: f32 = 76.0;
/// Maximum viewport height for expanded center-workbench content.
const MAX_ADVANCED_WORKBENCH_HEIGHT: f32 = 220.0;
/// Maximum Unicode scalar values retained in an adapter-local Delegate draft.
pub const DELEGATE_TASK_DRAFT_MAX_CHARS: usize = 4_096;
/// Maximum UTF-8 bytes retained in an adapter-local Delegate draft.
///
/// Four bytes per scalar keeps this consistent with [`DELEGATE_TASK_DRAFT_MAX_CHARS`]
/// while making the dispatch boundary explicit.
pub const DELEGATE_TASK_DRAFT_MAX_BYTES: usize = DELEGATE_TASK_DRAFT_MAX_CHARS * 4;
/// Chat turns kept visible in the Delegate rail before older ones are counted.
const DELEGATE_CHAT_VISIBLE_TURNS: usize = 8;

/// Action emitted by the top-bar `Command` control.
pub fn command_palette_control_action() -> DesktopAction {
    DesktopAction::OpenPalette {
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
    }
}

/// Responsive outer-shell dimensions derived from the available viewport.
///
/// The policy intentionally contains every viewport threshold used by the
/// renderer so product-mode rendering cannot move the editor between modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellGeometry {
    /// Whether the viewport uses deterministic compact panel dimensions.
    pub compact: bool,
    /// Whether secondary panes collapse into overlay drawers.
    pub ultra_compact: bool,
    /// Fixed top command-bar height.
    pub top_bar_height: f32,
    /// Width reserved for the left activity rail.
    pub activity_rail_width: f32,
    /// Width reserved for the explorer beside the activity rail.
    pub explorer_width: f32,
    /// Combined activity-rail and explorer width.
    pub left_width: f32,
    /// Minimum left-panel width at desktop sizes.
    pub left_min_width: f32,
    /// Right inspector width.
    pub right_width: f32,
    /// Minimum right-inspector width at desktop sizes.
    pub right_min_width: f32,
    /// Maximum right-inspector width at desktop sizes.
    pub right_max_width: f32,
    /// Bottom console height.
    pub bottom_height: f32,
    /// Minimum bottom-console height at desktop sizes.
    pub bottom_min_height: f32,
    /// Fixed status-bar height.
    pub status_bar_height: f32,
}

impl ShellGeometry {
    const COMPACT_WIDTH: f32 = 1_184.0;
    const COMPACT_HEIGHT: f32 = 530.0;
    const ULTRA_COMPACT_WIDTH: f32 = 720.0;
    const MIN_EDITOR_WIDTH: f32 = 360.0;
    const MIN_STANDARD_EDITOR_WIDTH: f32 = 560.0;
    const TOP_BAR_HEIGHT: f32 = 42.0;
    const STATUS_BAR_HEIGHT: f32 = 24.0;
    const ACTIVITY_RAIL_WIDTH: f32 = 46.0;
    const DESKTOP_EXPLORER_WIDTH: f32 = 248.0;
    const COMPACT_EXPLORER_WIDTH: f32 = 204.0;
    const RIGHT_WIDTH: f32 = 325.0;
    const BOTTOM_HEIGHT: f32 = 192.0;

    /// Derives deterministic shell dimensions from the available viewport.
    pub fn for_available_size(available_width: f32, available_height: f32) -> Self {
        let compact =
            available_width < Self::COMPACT_WIDTH || available_height < Self::COMPACT_HEIGHT;
        let ultra_compact = available_width < Self::ULTRA_COMPACT_WIDTH;
        let desired_explorer_width = if compact {
            Self::COMPACT_EXPLORER_WIDTH
        } else {
            Self::DESKTOP_EXPLORER_WIDTH
        };
        let desired_left_width = Self::ACTIVITY_RAIL_WIDTH + desired_explorer_width;
        let left_width = if compact {
            0.0
        } else {
            desired_left_width.min(
                (available_width - Self::RIGHT_WIDTH - Self::MIN_EDITOR_WIDTH)
                    .max(Self::ACTIVITY_RAIL_WIDTH),
            )
        };
        let right_width = if compact { 0.0 } else { Self::RIGHT_WIDTH };
        let bottom_height = if ultra_compact {
            0.0
        } else {
            let editor_min_height = if compact { 180.0 } else { 240.0 };
            let compact_strip_height = if compact { 28.0 } else { 0.0 };
            Self::BOTTOM_HEIGHT.min(
                (available_height
                    - Self::TOP_BAR_HEIGHT
                    - 28.0
                    - Self::STATUS_BAR_HEIGHT
                    - compact_strip_height
                    - editor_min_height
                    - 4.0)
                    .max(112.0),
            )
        };

        Self {
            compact,
            ultra_compact,
            top_bar_height: Self::TOP_BAR_HEIGHT,
            activity_rail_width: Self::ACTIVITY_RAIL_WIDTH,
            explorer_width: if compact {
                0.0
            } else {
                (left_width - Self::ACTIVITY_RAIL_WIDTH).max(0.0)
            },
            left_width,
            left_min_width: Self::ACTIVITY_RAIL_WIDTH + 160.0,
            right_width,
            right_min_width: 288.0,
            right_max_width: 480.0,
            bottom_height,
            bottom_min_height: 112.0,
            status_bar_height: Self::STATUS_BAR_HEIGHT,
        }
    }

    /// Returns the editor canvas width remaining after the side regions.
    pub fn editor_width(self, available_width: f32) -> f32 {
        (available_width - self.left_width - self.right_width).max(0.0)
    }

    fn top_bar_content_height(self) -> f32 {
        self.top_bar_height - 2.0 * 6.0
    }

    fn status_bar_content_height(self) -> f32 {
        self.status_bar_height - 2.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopBarDensity {
    Desktop,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TopBarComposition {
    density: TopBarDensity,
    shows_mode_switch: bool,
    shows_command_palette: bool,
    shows_workspace_context: bool,
}

fn top_bar_composition(geometry: ShellGeometry) -> TopBarComposition {
    if geometry.compact {
        TopBarComposition {
            density: TopBarDensity::Compact,
            shows_mode_switch: true,
            shows_command_palette: true,
            shows_workspace_context: false,
        }
    } else {
        TopBarComposition {
            density: TopBarDensity::Desktop,
            shows_mode_switch: true,
            shows_command_palette: true,
            shows_workspace_context: true,
        }
    }
}

/// Adapter-local view state layered over app-owned projections.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopProjectionViewState {
    /// Canonical explorer paths currently expanded by the adapter.
    pub expanded_explorer_paths: BTreeSet<String>,
    /// Adapter-local explorer selection override, if a native control is ahead of projection.
    pub selected_explorer_file: Option<FileId>,
    /// Where the person placed each canvas card, keyed by canonical path.
    pub canvas_positions: BTreeMap<String, canvas_workspace::SavedPosition>,
    /// Connections the person drew, as ordered  canonical paths.
    pub canvas_edges: Vec<(String, String)>,
    /// Which surface the centre shows.
    pub center_surface: CenterSurface,
    /// App-authoritative bottom-panel selection persisted across renderer frames.
    pub selected_bottom_panel: BottomPanelTab,
    /// Canonical workspace root projected by the runtime for scoped Delegate work.
    ///
    /// `None` means the runtime has not projected a workspace root yet; the
    /// renderer never probes the filesystem to infer one.
    pub canonical_workspace_root: Option<CanonicalPath>,
    /// Adapter-local mode-scoped dock layouts.
    pub dock_layouts: Vec<DockLayout>,
    /// Whether [`Self::dock_layouts`] is an arrangement the user made, rather
    /// than the shipped defaults.
    ///
    /// Only a user arrangement may override the panel sizes in
    /// [`ShellGeometry`]. `DockLayout::standard_all_modes` carries splitter
    /// fractions that disagree with those constants — it was written when
    /// nothing read them — and the constants are what the prototype-fidelity
    /// tests hold the shell to. Applying the defaults' fractions would silently
    /// resize every panel in the product.
    pub dock_layouts_user_arranged: bool,
    /// Adapter-local toast ids dismissed by the renderer.
    pub dismissed_toast_ids: BTreeSet<u64>,
    /// Whether the first-run onboarding card should be rendered.
    pub first_run_onboarding_visible: bool,
    /// Whether the LSP completion popup is currently visible (T6).
    pub completion_popup_open: bool,
    /// Zero-based index of the selected completion item (T6).
    pub completion_selected_index: usize,
    /// Whether the LSP hover tooltip is currently visible (T7).
    pub hover_tooltip_visible: bool,
    /// Keyboard-focused row index in the Problems panel (T4).
    pub problems_selected_index: usize,
    /// Keyboard-focused hunk index in the proposal review surface (PKT-DIFF).
    pub review_hunk_selected_index: usize,
    /// Durable checkpoint timeline rows from the checkpoint store (PKT-CKPT).
    pub durable_checkpoint_timeline_rows:
        Vec<crate::view::proposal_review::DesktopCheckpointTimelineRow>,
    /// Preferred product AI route label (`auto` / `ollama` / `anthropic` / `deterministic`).
    pub preferred_ai_provider: String,
    /// Accumulated product AI stream chunks for the assistant rail (Assist / Delegate).
    pub product_ai_stream_chunks: Vec<String>,
    /// Metadata label for the last product stream (`provider/model/operation`).
    pub product_ai_stream_label: String,
    /// Whether the last stream used multi-delta SSE.
    pub product_ai_streamed: bool,
    /// Whether a product AI stream is currently in flight.
    pub product_ai_stream_in_flight: bool,
}

impl Default for DesktopProjectionViewState {
    fn default() -> Self {
        Self {
            expanded_explorer_paths: BTreeSet::new(),
            selected_explorer_file: None,
            canvas_positions: BTreeMap::new(),
            canvas_edges: Vec::new(),
            center_surface: CenterSurface::Editor,
            selected_bottom_panel: BottomPanelTab::Terminal,
            canonical_workspace_root: None,
            dock_layouts: DockLayout::standard_all_modes(),
            dock_layouts_user_arranged: false,
            dismissed_toast_ids: BTreeSet::new(),
            first_run_onboarding_visible: false,
            completion_popup_open: false,
            completion_selected_index: 0,
            hover_tooltip_visible: false,
            problems_selected_index: 0,
            review_hunk_selected_index: 0,
            durable_checkpoint_timeline_rows: Vec::new(),
            preferred_ai_provider: "auto".to_string(),
            product_ai_stream_chunks: Vec::new(),
            product_ai_stream_label: String::new(),
            product_ai_streamed: false,
            product_ai_stream_in_flight: false,
        }
    }
}

/// Adapter-local IME composition overlay tracked per active buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ImeCompositionProjection {
    /// Whether composition is currently active.
    pub active: bool,
    /// Preedit text currently being composed.
    pub preedit: String,
}

pub(crate) fn ime_composition_state_id(buffer_id: BufferId) -> egui::Id {
    egui::Id::new(("legion-ime-composition", buffer_id))
}

pub(crate) fn ime_composition_state(
    ui: &egui::Ui,
    buffer_id: BufferId,
) -> Option<ImeCompositionProjection> {
    ui.ctx().data_mut(|data| {
        data.get_temp::<ImeCompositionProjection>(ime_composition_state_id(buffer_id))
    })
}

/// Structured status-bar projection derived from app-owned shell data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopStatusBarViewModel {
    /// Active product-mode label.
    pub product_mode: String,
    /// Display-safe state flags such as dirty, degraded, or no active buffer.
    pub flags: Vec<String>,
    /// Active file path when a file-backed buffer is selected.
    pub path: Option<String>,
    /// Active workspace identifier.
    pub workspace_id: Option<u128>,
    /// Active file identifier.
    pub file_id: Option<u128>,
    /// Active buffer identifier.
    pub buffer_id: Option<u128>,
    /// Text encoding when an active text buffer exists.
    pub encoding: Option<String>,
    /// Detected line ending when the bounded projection has enough evidence.
    pub line_ending: Option<String>,
    /// Primary cursor position from the bounded viewport projection.
    pub cursor: Option<DesktopStatusCursor>,
    /// Language label inferred from the active file path.
    pub language: Option<String>,
    /// Real connection state when projected by the application layer.
    pub connection: Option<String>,
    /// Workspace trust state when explicitly projected by the application.
    pub trust: Option<String>,
    /// Language/LSP lifecycle when the projection is not idle.
    pub lsp: Option<String>,
}

/// One-based cursor display coordinates for the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopStatusCursor {
    /// One-based line number.
    pub line: u32,
    /// One-based column number.
    pub column: u32,
}

/// Structured code-canvas line derived from active-buffer viewport projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopCodeLineViewModel {
    /// One-based display line number.
    pub number: u32,
    /// Visible text for this code-canvas row.
    pub text: String,
    /// Semantic highlight spans scoped to this visible row.
    pub highlights: Vec<DesktopCodeHighlightSpan>,
    /// Truncation state for the visible viewport slice backing this row.
    pub truncation_state: ViewportLineTruncationState,
    /// Snapshot byte range represented by this visible slice.
    pub byte_range: ByteRange,
    /// Snapshot UTF-16 range represented by this visible slice.
    pub utf16_range: Utf16Range,
    /// Snapshot byte origin of the logical line, when exact metrics provide it.
    pub line_start_byte_offset: Option<u64>,
    /// Absolute snapshot byte end of the complete logical line, when known.
    pub logical_end_byte: Option<u64>,
    /// Snapshot UTF-16 origin of the logical line, when exact metrics provide it.
    pub line_start_utf16_offset: Option<u64>,
}

/// Renderer-ready semantic highlight span for a single visible code line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopCodeHighlightSpan {
    /// Zero-based starting display column within the line.
    pub start_col: u32,
    /// Exclusive ending display column within the line.
    pub end_col: u32,
    /// Semantic token kind to map into the active desktop theme.
    pub kind: ViewportSemanticTokenKind,
}

impl DesktopStatusBarViewModel {
    fn from_snapshot(snapshot: &ShellProjectionSnapshot, flags: &[String]) -> Self {
        let active = &snapshot.active_buffer_projection;
        Self {
            product_mode: snapshot.product_mode.label().to_string(),
            flags: flags.to_vec(),
            // Display form: the status bar showed `\\?\D:\...` verbatim, which
            // is the Windows extended-length prefix and belongs to the kernel,
            // not to the person reading the bar.
            path: active
                .file_path
                .as_ref()
                .map(|path| crate::path_display::display_path(&path.0).into_owned()),
            workspace_id: active.workspace_id.map(|workspace| workspace.0),
            file_id: active.file_id.map(|file| file.0),
            buffer_id: active.buffer_id.map(|buffer| buffer.0),
            encoding: active.buffer_id.map(|_| "UTF-8".to_string()),
            line_ending: status_line_ending(active),
            cursor: active
                .viewport
                .as_ref()
                .map(|viewport| DesktopStatusCursor {
                    line: viewport.cursor.line.saturating_add(1),
                    column: viewport.cursor.character.saturating_add(1),
                }),
            language: active
                .file_path
                .as_ref()
                .map(|path| status_language_for_path(&path.0)),
            connection: None,
            trust: snapshot
                .context_manifest_projection
                .manifest
                .workspace_trust_state
                .as_ref()
                .map(|trust| format!("{trust:?}")),
            lsp: snapshot
                .language_tooling_projection
                .lsp_session_status
                .as_ref()
                .map(|status| format!("{:?}", status.lifecycle)),
        }
    }
}

/// Structured command-palette overlay model for renderer-owned drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopCommandPaletteOverlayViewModel {
    /// Whether the overlay should be rendered.
    pub open: bool,
    /// Human-readable mode label.
    pub mode_label: String,
    /// Current query text.
    pub query: String,
    /// Search scope label for search-oriented modes.
    pub scope_label: String,
    /// Projected result rows.
    pub result_rows: Vec<DesktopCommandPaletteResultViewModel>,
}

/// Structured command-palette result row for renderer-owned drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopCommandPaletteResultViewModel {
    /// Stable result identifier.
    pub id: String,
    /// Product-facing command group.
    pub group_label: String,
    /// Human-readable result kind.
    pub kind_label: String,
    /// Result title.
    pub title: String,
    /// Secondary result metadata.
    pub detail: Option<String>,
    /// Shortcut or action hint label.
    pub shortcut_label: Option<String>,
    /// Character indices matched by the app-owned scorer.
    pub match_indices: Vec<usize>,
    /// Whether this row is currently selected.
    pub selected: bool,
    /// Disabled reason when the row is visible but not dispatchable.
    pub disabled_reason: Option<String>,
}

/// Structured foreground toast stack for renderer-owned drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopToastStackViewModel {
    /// Visible toast notifications.
    pub visible: Vec<DesktopToastViewModel>,
    /// Additional notification count hidden by the visible cap.
    pub overflow_count: usize,
}

/// Structured foreground toast notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopToastViewModel {
    /// Stable toast identifier used for dismissal.
    pub id: u64,
    /// Severity classification.
    pub severity: StatusSeverity,
    /// Primary notification title.
    pub title: String,
    /// Optional secondary notification text.
    pub body: Option<String>,
    /// Optional action routed through existing command authority.
    pub action: Option<ToastActionProjection>,
    /// Whether the toast should remain until dismissed.
    pub sticky: bool,
}

/// Structured workbench settings view model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSettingsViewModel {
    /// Active theme preference.
    pub theme_preference: ThemePreferenceProjection,
    /// Active theme preference display label.
    pub theme_label: String,
    /// UI zoom percentage.
    pub zoom_percent: u16,
    /// Editor font size in points.
    pub editor_font_size_pt: u16,
    /// Editor font family label.
    pub editor_font_family: String,
    /// Metadata-only font fallback diagnostic rows.
    pub font_fallback_rows: Vec<String>,
    /// Toast verbosity preference.
    pub toast_verbosity: ToastVerbosityProjection,
    /// Toast verbosity display label.
    pub toast_verbosity_label: String,
    /// Whether line numbers are visible.
    pub line_numbers_visible: bool,
    /// Whether current-line highlighting is enabled.
    pub current_line_highlight: bool,
    /// Whether sticky headers are visible.
    pub sticky_headers_visible: bool,
    /// Whether code folding indicators are visible.
    pub code_folding_visible: bool,
    /// Whether the minimap is visible.
    pub minimap_visible: bool,
    /// Whether whitespace guides are visible.
    pub whitespace_guides_visible: bool,
    /// Whether indent guides are visible.
    pub indent_guides_visible: bool,
    /// Whether workspace search may use the optional indexed backend.
    pub indexed_workspace_search_enabled: bool,
    /// Whether next-edit prediction should auto-trigger after edits.
    pub next_edit_prediction_enabled: bool,
    /// Whether smooth scrolling is enabled.
    pub smooth_scrolling_enabled: bool,
    /// Editor line wrapping policy.
    pub line_wrapping_policy: LineWrappingPolicy,
    /// Optional fixed wrapping column.
    pub wrap_column: Option<u32>,
    /// Stable wrapping policy row for deterministic renderer evidence.
    pub wrapping_row: String,
    /// Whether crash reports are enabled.
    pub crash_reports_enabled: bool,
    /// Telemetry consent display label.
    pub telemetry_label: String,
    /// Projection schema version.
    pub schema_version: u16,
}

impl DesktopSettingsViewModel {
    fn from_projection(projection: &SettingsProjection) -> Self {
        let normalized = projection.clone().normalized();
        let font_fallback_rows = font_fallback_rows(&normalized);
        let wrapping_row = match normalized.editor.line_wrapping_policy {
            LineWrappingPolicy::Off => "wrapping: off".to_string(),
            LineWrappingPolicy::Viewport => "wrapping: viewport".to_string(),
            LineWrappingPolicy::FixedColumn => format!(
                "wrapping: fixed_column {}",
                normalized.editor.wrap_column.unwrap_or(120)
            ),
        };
        Self {
            theme_preference: normalized.theme_preference,
            theme_label: normalized.theme_preference.label().to_string(),
            zoom_percent: normalized.zoom_percent,
            editor_font_size_pt: normalized.editor_font_size_pt,
            editor_font_family: normalized.editor_font_family.clone(),
            font_fallback_rows,
            toast_verbosity: normalized.toast_verbosity,
            toast_verbosity_label: normalized.toast_verbosity.label().to_string(),
            line_numbers_visible: normalized.editor.line_numbers_visible,
            current_line_highlight: normalized.editor.current_line_highlight,
            sticky_headers_visible: normalized.editor.sticky_headers_visible,
            code_folding_visible: normalized.editor.code_folding_visible,
            minimap_visible: normalized.editor.minimap_visible,
            whitespace_guides_visible: normalized.editor.whitespace_guides_visible,
            indent_guides_visible: normalized.editor.indent_guides_visible,
            indexed_workspace_search_enabled: normalized.indexed_workspace_search_enabled,
            next_edit_prediction_enabled: normalized.next_edit_prediction_enabled,
            smooth_scrolling_enabled: normalized.editor.smooth_scrolling_enabled,
            line_wrapping_policy: normalized.editor.line_wrapping_policy,
            wrap_column: normalized.editor.wrap_column,
            wrapping_row,
            crash_reports_enabled: normalized.telemetry.crash_reports_enabled,
            telemetry_label: normalized.telemetry.consent_label.clone(),
            schema_version: normalized.schema_version,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TypeScriptToolchainDraft {
    server_archive: String,
    compiler_archive: String,
    node_executable: String,
}

const TYPESCRIPT_TOOLCHAIN_PATH_MAX_CHARS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopSetupChecklistItem {
    title: &'static str,
    detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopSetupChecklistViewModel {
    items: [DesktopSetupChecklistItem; 4],
}

impl DesktopSetupChecklistViewModel {
    fn from_snapshot(
        snapshot: &ShellProjectionSnapshot,
        settings: &DesktopSettingsViewModel,
    ) -> Self {
        let provider_count = snapshot
            .assisted_ai_projection
            .providers
            .iter()
            .filter(|provider| {
                provider.availability == AssistedAiProviderAvailabilityState::Available
            })
            .count();
        let manifest = &snapshot.context_manifest_projection.manifest;
        let workspace_projected = snapshot.active_buffer_projection.workspace_id.is_some()
            || manifest.workspace_id.is_some()
            || manifest.workspace_trust_state.is_some();
        let workspace_detail = if !workspace_projected {
            "No workspace is open. Open a workspace to begin.".to_string()
        } else {
            match manifest.workspace_trust_state.as_ref() {
                Some(legion_protocol::WorkspaceTrustState::Trusted) => {
                    "Workspace is open and trusted. Workspace tools are available.".to_string()
                }
                Some(legion_protocol::WorkspaceTrustState::Untrusted) => {
                    "Workspace is open but not trusted. Review workspace trust before running tools."
                        .to_string()
                }
                Some(legion_protocol::WorkspaceTrustState::Unknown) | None => format!(
                    "Workspace is open. Review its trust before running workspace tools. Current mode: {}.",
                    projected_product_mode(snapshot).label()
                ),
            }
        };
        Self {
            items: [
                DesktopSetupChecklistItem {
                    title: "Step 1 · Open and trust a workspace",
                    detail: workspace_detail,
                },
                DesktopSetupChecklistItem {
                    title: "Step 2 · Optionally configure an AI provider",
                    detail: format!(
                        "{provider_count} AI provider{} available. Credentials stay in the system keyring.",
                        if provider_count == 1 { " is" } else { "s are" }
                    ),
                },
                DesktopSetupChecklistItem {
                    title: "Step 3 · Review privacy and reporting",
                    detail: format!(
                        "Crash reporting is {}. Data sharing is {}.",
                        if settings.crash_reports_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        settings.telemetry_label
                    ),
                },
                DesktopSetupChecklistItem {
                    title: "Step 4 · Learn Manual, Assist, Delegate, and Legion Workflows",
                    detail: "Use the mode switch at the top. Legion confirms higher-authority modes before opening them."
                        .to_string(),
                },
            ],
        }
    }
}

fn font_fallback_rows(projection: &SettingsProjection) -> Vec<String> {
    if projection.font_fallback_diagnostics.is_empty() {
        return vec![format!(
            "font fallback: requested={} resolved={} coverage={} found={} message={}",
            sanitize_font_fallback_label(&projection.editor_font_family, "monospace"),
            "unreported",
            "cjk",
            false,
            "diagnostic-unreported"
        )];
    }

    projection
        .font_fallback_diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "font fallback: requested={} resolved={} coverage={} found={}",
                sanitize_font_fallback_label(
                    &diagnostic.requested_family_label,
                    "redacted-requested-font"
                ),
                sanitize_font_fallback_label(
                    &diagnostic.resolved_family_label,
                    "redacted-resolved-font"
                ),
                sanitize_font_fallback_label(&diagnostic.coverage_label, "unknown"),
                diagnostic.fallback_found
            )
        })
        .collect()
}

fn sanitize_font_fallback_label(value: &str, fallback: &str) -> String {
    let label = value.trim();
    if label.is_empty() || looks_like_font_path(label) {
        return fallback.to_string();
    }

    let normalized = label
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(64)
        .collect::<String>();
    if normalized.trim().is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn looks_like_font_path(value: &str) -> bool {
    value.contains('\\') || value.contains('/') || value.contains(':')
}

/// Testable display model derived only from a shell projection snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopAuthorityRibbonViewModel {
    /// Plain-language authority summary for the active product mode.
    pub summary: String,
    /// Workspace or scope detail when a richer projection exists.
    pub workspace_scope: Option<String>,
    /// Provider readiness detail when provider metadata is projected.
    pub provider_readiness: Option<String>,
    /// Approval-boundary detail when checklist metadata is projected.
    pub approval_boundary: Option<String>,
}

/// Renderer-local readiness for a mode-owned surface.
///
/// This presentation model never owns workspace, provider, task, or workflow
/// state. It only decides which projected surface can be shown truthfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceAvailability {
    /// The projection contains the prerequisites needed to render the surface.
    Ready,
    /// The surface is visible as one prerequisite card until the user resolves it.
    Blocked {
        /// User-facing outcome that explains why the surface cannot proceed.
        reason: String,
        /// User-facing next step.
        resolution: String,
    },
    /// The surface has no truthful content for the current mode state.
    Hidden,
}

/// Renderer lifecycle derived only from Delegate-owned app projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegateLifecycle {
    /// No submitted task is running; the rail may collect a new bounded task.
    Draft,
    /// A submitted task is planning, isolated, executing, or verifying.
    Running,
    /// A submitted task is waiting on approval or a resolvable prerequisite.
    Waiting,
    /// A submitted task completed, failed, was refused, or was cancelled.
    Terminal,
}

/// Coherent center/inspector presentation derived from projections and local UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeSurfaceModel {
    /// Availability of the optional mode-owned center surface above the editor.
    pub center: SurfaceAvailability,
    /// Availability of the contextual right inspector.
    pub inspector: SurfaceAvailability,
    /// Explicit Delegate lifecycle when Delegate is the projected product mode.
    pub delegate_lifecycle: Option<DelegateLifecycle>,
}

impl ModeSurfaceModel {
    fn from_snapshot(
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
    ) -> Self {
        match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => Self {
                center: SurfaceAvailability::Hidden,
                inspector: SurfaceAvailability::Hidden,
                delegate_lifecycle: None,
            },
            // Assist's inspector is the inline-prediction panel, and inline
            // prediction routes through the always-registered deterministic
            // local provider: it needs a buffer and nothing else.
            //
            // This arm used to block on `assisted_ai_projection.providers`
            // being empty, with the resolution "Settings". That projection
            // describes a *Phase-4 assisted-AI run* and is populated only as a
            // side effect of one, so in the shipped app it is empty until a run
            // happens — and no rendered control starts a run. Worse, the
            // resolution it named could not clear it: setting a preferred AI
            // provider in Settings never touches that list. Assist mode was
            // therefore blocked forever behind a card telling the user to do
            // something that would not help, while `Predict` — which works with
            // zero configuration — sat behind the block. The panel names the
            // provider that actually answered, so the honest gate is the buffer.
            DesktopProductMode::Assist => {
                // A build with neither `ai` nor `offline` has no inline
                // prediction provider at all: `legion-app` compiles the
                // `not(any(...))` implementation, which answers every request
                // with "AI feature is disabled". The proposal controls account
                // for that build and this gate did not, so `Predict` appeared
                // and could only ever fail.
                let inspector = if !cfg!(any(feature = "ai", feature = "offline")) {
                    SurfaceAvailability::Blocked {
                        reason: "This build has no inline prediction provider.".to_string(),
                        resolution: "Unavailable".to_string(),
                    }
                } else if snapshot.active_buffer_projection.buffer_id.is_none() {
                    SurfaceAvailability::Blocked {
                        reason: "Open a file to enable predictions.".to_string(),
                        resolution: "Open file".to_string(),
                    }
                } else {
                    SurfaceAvailability::Ready
                };
                Self {
                    center: SurfaceAvailability::Hidden,
                    inspector,
                    delegate_lifecycle: None,
                }
            }
            DesktopProductMode::Delegate => {
                let lifecycle = delegate_lifecycle(snapshot);
                let task_owned = delegated_task_owned_state_projected(snapshot);
                let blocked = task_owned && delegated_task_is_blocked(snapshot);
                Self {
                    center: if task_owned && !blocked {
                        SurfaceAvailability::Ready
                    } else {
                        SurfaceAvailability::Hidden
                    },
                    inspector: if blocked {
                        SurfaceAvailability::Blocked {
                            reason: "Task is blocked".to_string(),
                            resolution: "Review task scope and approvals, then retry.".to_string(),
                        }
                    } else if task_owned || state.canonical_workspace_root.is_some() {
                        SurfaceAvailability::Ready
                    } else {
                        SurfaceAvailability::Blocked {
                            reason: "Open a workspace to define Delegate scope.".to_string(),
                            resolution: "Open a trusted workspace, then try again.".to_string(),
                        }
                    },
                    delegate_lifecycle: Some(lifecycle),
                }
            }
            DesktopProductMode::LegionWorkflows => Self {
                center: if snapshot.legion_workflow_projection.rows.is_empty() {
                    SurfaceAvailability::Hidden
                } else {
                    SurfaceAvailability::Ready
                },
                inspector: SurfaceAvailability::Ready,
                delegate_lifecycle: None,
            },
        }
    }
}

/// Testable display model derived only from a shell projection snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProjectionViewModel {
    /// Window or shell title.
    pub layout_title: String,
    /// Top command-bar rows.
    pub top_bar_rows: Vec<String>,
    /// Plain-language authority boundary rendered below the command bar.
    pub authority_ribbon: DesktopAuthorityRibbonViewModel,
    /// Read-only product-mode rows.
    pub product_mode_rows: Vec<String>,
    /// Four-step autonomy scale rows derived from the active product mode.
    pub autonomy_scale_rows: Vec<String>,
    /// Mode escalation confirmation and permission rows.
    pub mode_confirmation_rows: Vec<String>,
    /// Structured command-palette overlay model.
    pub command_palette_overlay: DesktopCommandPaletteOverlayViewModel,
    /// Structured foreground notification stack.
    pub toast_stack: DesktopToastStackViewModel,
    /// Structured workbench settings.
    pub settings: DesktopSettingsViewModel,
    /// Command-palette group and item rows.
    pub command_palette_rows: Vec<String>,
    /// Left sidebar summary rows.
    pub left_sidebar_rows: Vec<String>,
    /// Main code-canvas summary rows.
    pub main_canvas_rows: Vec<String>,
    /// Stable first-screen center surface shared by every product mode.
    pub center_surface: String,
    /// Renderer-local center/inspector availability for the active product mode.
    pub mode_surface: ModeSurfaceModel,
    /// Right dock directive and trust summary rows.
    pub directive_panel_rows: Vec<String>,
    /// First-run onboarding rows.
    pub onboarding_rows: Vec<String>,
    /// Bottom operational console rows.
    pub bottom_console_rows: Vec<String>,
    /// Mode-specific bottom tab rows.
    pub bottom_tab_rows: Vec<String>,
    /// Active dock registry/layout summary rows.
    pub dock_rows: Vec<String>,
    /// Visible dock panel rows after mode filtering.
    pub dock_panel_rows: Vec<String>,
    /// Compact status-bar projection.
    pub status_bar: DesktopStatusBarViewModel,
    /// Tab-strip display rows.
    pub tab_rows: Vec<String>,
    /// Explorer display rows.
    pub explorer_rows: Vec<String>,
    /// Explorer state rows with selection and expansion markers.
    pub explorer_state_rows: Vec<String>,
    /// Active-buffer viewport or small-buffer rows.
    pub active_buffer_lines: Vec<String>,
    /// Structured active-buffer code rows for editor-canvas rendering.
    pub active_buffer_code_lines: Vec<DesktopCodeLineViewModel>,
    /// Active editor metadata rows.
    pub editor_status_rows: Vec<String>,
    /// Dirty-close prompt rows.
    pub close_prompt_rows: Vec<String>,
    /// Per-buffer viewport metadata rows.
    pub viewport_metadata_rows: Vec<String>,
    /// Large-file degraded-mode capability banner rows.
    pub large_file_banner_rows: Vec<String>,
    /// Status rows.
    pub status_rows: Vec<String>,
    /// Proposal ledger summary rows.
    pub proposal_rows: Vec<String>,
    /// Trust, privacy, permission, approval, and checkpoint rows.
    pub trust_rows: Vec<String>,
    /// Assisted-AI and delegated-task summary rows.
    pub assistant_rows: Vec<String>,
    /// Legion workflow command-center rows.
    pub legion_workflow_rows: Vec<String>,
    /// Language tooling summary rows.
    pub language_rows: Vec<String>,
    /// LSP server health and download-refusal rows (projection-only, read-only).
    pub lsp_health_rows: Vec<String>,
    /// Structural search and replace summary rows.
    pub structural_search_rows: Vec<String>,
    /// Git status, diff, blame, graph, and conflict rows.
    pub git_rows: Vec<String>,
    /// Terminal panel summary rows.
    pub terminal_rows: Vec<String>,
    /// Test explorer summary rows.
    pub test_rows: Vec<String>,
    /// Debugger summary rows.
    pub debug_rows: Vec<String>,
    /// Operational health summary rows.
    pub operational_health_rows: Vec<String>,
    /// Manual-mode local control and trust-boundary rows derived from projections.
    pub manual_control_rows: Vec<String>,
    /// Plugin contribution summary rows.
    pub plugin_rows: Vec<String>,
    /// Extension catalog panel model (P7.F2).
    pub extensions_panel: extensions_panel::DesktopExtensionsPanelViewModel,
    /// Cloud Lane tasks and their cancel controls (P9.F3.T3).
    pub cloud_lane: cloud_lane::DesktopCloudLanePanelViewModel,
    /// Collaboration presence rows.
    pub collaboration_rows: Vec<String>,
    /// Remote workspace manager rows.
    pub remote_rows: Vec<String>,
    /// Sandbox panel rows for delegated task runtime state.
    pub sandbox_rows: Vec<String>,
    /// Empty, dirty, or degraded display flags.
    pub empty_or_degraded_flags: Vec<String>,
    /// Preferred product AI route label mirrored from app composition.
    pub preferred_ai_provider: String,
    /// Product AI stream chunks for progressive assistant-rail rendering.
    pub product_ai_stream_chunks: Vec<String>,
    /// Metadata label for the last product stream.
    pub product_ai_stream_label: String,
    /// Whether the last stream used multi-delta SSE.
    pub product_ai_streamed: bool,
    /// Whether a product AI stream is currently in flight.
    pub product_ai_stream_in_flight: bool,
}

impl DesktopProjectionViewModel {
    /// Emits stable, metadata-only rows for renderer evidence without raw source payloads.
    pub fn deterministic_editor_evidence(&self) -> Vec<String> {
        let mut rows = Vec::new();
        rows.push(format!("title={}", self.layout_title));
        rows.extend(self.editor_status_rows.iter().map(|row| {
            format!(
                "editor_status={}",
                Self::evidence_safe_editor_status_row(row)
            )
        }));
        rows.extend(
            self.viewport_metadata_rows
                .iter()
                .map(|row| format!("viewport={row}")),
        );
        rows.extend(
            self.empty_or_degraded_flags
                .iter()
                .map(|flag| format!("flag={flag}")),
        );
        rows.extend(self.active_buffer_code_lines.iter().take(8).map(|line| {
            format!(
                "code_line={} len={} truncation={:?}",
                line.number,
                line.text.chars().count(),
                line.truncation_state
            )
        }));
        rows.extend(
            self.large_file_banner_rows
                .iter()
                .map(|row| format!("large_file={row}")),
        );
        rows
    }

    fn evidence_safe_editor_status_row(row: &str) -> String {
        let Some((prefix, path)) = row.split_once(" path=") else {
            return row.to_string();
        };
        format!("{prefix} path={}", Self::evidence_safe_path_label(path))
    }

    fn evidence_safe_path_label(path: &str) -> String {
        if path == "<untitled>" || path.is_empty() {
            return path.to_string();
        }

        let Some(file_name) = path
            .rsplit(['\\', '/'])
            .find(|segment| !segment.trim().is_empty())
        else {
            return "metadata-redacted".to_string();
        };

        let sanitized = file_name
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
            .take(64)
            .collect::<String>();
        if sanitized.trim().is_empty() {
            "metadata-redacted".to_string()
        } else {
            sanitized
        }
    }

    /// Builds a display model from a projection snapshot without taking product-state ownership.
    pub fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        Self::from_snapshot_with_state(snapshot, &DesktopProjectionViewState::default())
    }

    /// Builds a display model from a projection snapshot plus adapter-local view state.
    pub fn from_snapshot_with_state(
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
    ) -> Self {
        let mut flags = Vec::new();
        let active = &snapshot.active_buffer_projection;
        if active.dirty {
            flags.push("dirty".to_string());
        }
        if active.degraded
            || active
                .viewport
                .as_ref()
                .is_some_and(|viewport| viewport.mode.defers_whole_file_work())
        {
            flags.push("degraded".to_string());
        }
        if snapshot.explorer_projection.nodes.is_empty() {
            flags.push("empty_explorer".to_string());
        }
        if active.buffer_id.is_none() {
            flags.push("no_active_buffer".to_string());
        }

        let product_mode_rows = product_mode_rows(snapshot);
        let autonomy_scale_rows = autonomy_scale_rows(snapshot);
        let mode_confirmation_rows = mode_confirmation_rows(snapshot);
        let command_palette_overlay = command_palette_overlay(snapshot);
        let toast_stack = toast_stack(snapshot, state);
        let settings = DesktopSettingsViewModel::from_projection(&snapshot.settings_projection);
        let command_palette_rows = command_palette_rows(snapshot);
        let dock_rows = dock_rows(snapshot, state);
        let dock_panel_rows = dock_panel_rows(snapshot, state);
        let sandbox_rows = if snapshot.product_mode == DockMode::Delegate {
            let state = sandbox_panel::SandboxPanelState::from_snapshot(snapshot);
            sandbox_panel::rows(snapshot, state)
        } else {
            Vec::new()
        };
        let onboarding_rows = onboarding_rows(snapshot, state);
        Self {
            layout_title: snapshot.layout_projection.layout.title.clone(),
            top_bar_rows: top_bar_rows(snapshot),
            authority_ribbon: authority_ribbon_view_model(snapshot),
            product_mode_rows,
            autonomy_scale_rows,
            mode_confirmation_rows,
            command_palette_overlay,
            toast_stack,
            settings,
            command_palette_rows,
            left_sidebar_rows: left_sidebar_rows(snapshot),
            main_canvas_rows: main_canvas_rows(snapshot),
            center_surface: center_surface_label(state.center_surface).to_string(),
            mode_surface: ModeSurfaceModel::from_snapshot(snapshot, state),
            directive_panel_rows: directive_panel_rows(snapshot),
            onboarding_rows,
            bottom_console_rows: bottom_console_rows(snapshot),
            bottom_tab_rows: bottom_tab_rows(snapshot, BottomPanelTab::Terminal, false),
            dock_rows,
            dock_panel_rows,
            status_bar: DesktopStatusBarViewModel::from_snapshot(snapshot, &flags),
            tab_rows: tab_rows(snapshot),
            explorer_rows: explorer_rows(snapshot, state),
            explorer_state_rows: explorer_rows(snapshot, state),
            active_buffer_lines: active_buffer_lines(snapshot),
            active_buffer_code_lines: active_buffer_code_lines(snapshot),
            editor_status_rows: editor_status_rows(snapshot),
            close_prompt_rows: close_prompt_rows(snapshot),
            viewport_metadata_rows: viewport_metadata_rows(snapshot),
            large_file_banner_rows: large_file_banner_rows(snapshot),
            status_rows: status_rows(snapshot),
            proposal_rows: proposal_rows(snapshot),
            trust_rows: trust_rows(snapshot),
            assistant_rows: assistant_rows(snapshot),
            legion_workflow_rows: legion_workflow_rows(snapshot),
            language_rows: language_rows(snapshot),
            lsp_health_rows: lsp_health_rows(snapshot),
            structural_search_rows: structural_search_rows(snapshot),
            git_rows: git_rows(snapshot),
            terminal_rows: terminal_rows(snapshot),
            test_rows: test_rows(snapshot),
            debug_rows: debug_rows(snapshot),
            operational_health_rows: operational_health_rows(snapshot),
            manual_control_rows: manual_control_rows(snapshot),
            plugin_rows: plugin_rows(snapshot),
            cloud_lane: cloud_lane::DesktopCloudLanePanelViewModel::from_snapshot(snapshot),
            extensions_panel: extensions_panel::DesktopExtensionsPanelViewModel::from_snapshot(
                snapshot,
            ),
            collaboration_rows: collaboration_rows(snapshot),
            remote_rows: remote_rows(snapshot),
            sandbox_rows,
            empty_or_degraded_flags: flags,
            preferred_ai_provider: state.preferred_ai_provider.clone(),
            product_ai_stream_chunks: state.product_ai_stream_chunks.clone(),
            product_ai_stream_label: state.product_ai_stream_label.clone(),
            product_ai_streamed: state.product_ai_streamed,
            product_ai_stream_in_flight: state.product_ai_stream_in_flight,
        }
    }
}

/// Physical panel allocations from the most recent composed shell frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellPanelRects {
    /// Full-width top command bar.
    pub top: egui::Rect,
    /// Full-width authority ribbon directly below the command bar.
    pub authority: egui::Rect,
    /// Full-width bottom status bar.
    pub status: egui::Rect,
    /// Full-height explorer/activity rail between authority and status.
    pub left: egui::Rect,
    /// Full-height mode rail between authority and status.
    pub right: egui::Rect,
    /// Center-column console above the status bar.
    pub bottom: egui::Rect,
    /// Remaining center editor region above the console.
    pub center: egui::Rect,
}

/// Renderer-owned projection view state.
#[derive(Debug)]
pub struct ProjectionView {
    theme_preference: theme::ThemePreference,
    selected_activity: ActivitySurface,
    utility_surface: Option<UtilitySurface>,
    settings_section: SettingsSection,
    utility_overlay_origin: Option<egui::Id>,
    utility_overlay_needs_focus: bool,
    utility_overlay_focus_bounds: Option<(egui::Id, egui::Id)>,
    utility_restore_focus: Option<egui::Id>,
    command_palette_origin: Option<egui::Id>,
    compact_drawer: Option<CompactDrawer>,
    compact_drawer_origin: Option<egui::Id>,
    compact_drawer_needs_focus: bool,
    compact_drawer_restore_focus: Option<egui::Id>,
    last_editor_rect: Option<egui::Rect>,
    last_shell_panel_rects: Option<ShellPanelRects>,
    /// Dock sizes from the previous frame, so a splitter drag can be told apart
    /// from a restored layout, a remembered egui size, or a window resize.
    last_dock_measurement: Option<dock_geometry::DockMeasurement>,
    /// Renderer-only presentation state. This is not product-mode authority.
    pending_mode_confirmation: Option<DockMode>,
    pending_mode_confirmation_source: Option<DockMode>,
    pending_mode_confirmation_origin: Option<egui::Id>,
    pending_mode_confirmation_needs_focus: bool,
    mode_confirmation_restore_focus: Option<egui::Id>,
    streamed_layout_cache: StreamedLayoutCachePool,
    typescript_toolchain_draft: TypeScriptToolchainDraft,
    typescript_toolchain_projection: Option<TypeScriptToolchainProjection>,
    typescript_toolchain_workspace: Option<WorkspaceId>,
}

/// Renderer-owned selection for the workspace activity rail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActivitySurface {
    /// Workspace file navigation.
    #[default]
    Explorer,
    /// Workspace text search.
    Search,
    /// Workspace symbol search.
    Symbols,
    /// Source-control tools.
    SourceControl,
    /// Test discovery and execution tools.
    Tests,
    /// Run and debug tools.
    Debug,
}

/// What the central panel is currently showing.
///
/// New concept. Until now the centre was always the editor —
/// `center_surface_label` returned a hard-coded `"editor"` and ignored its
/// argument — so there was nothing to switch. Renderer-owned, like the activity
/// rail selection: which files are open is the app's business, which of them you
/// are looking at and how is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CenterSurface {
    /// The code editor.
    #[default]
    Editor,
    /// The canvas workspace: every open file as a card in 2D space.
    Canvas,
}

impl CenterSurface {
    /// The label the status line and tests use for this surface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Canvas => "canvas",
        }
    }
}

/// Renderer-owned utility presentation that stays independent of product mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilitySurface {
    /// Bounded application settings overlay.
    Settings,
    /// Reopenable setup and welcome overlay.
    Setup,
    /// Help/About overlay (version, license, privacy).
    About,
    /// Raw internal diagnostics in the bottom panel.
    Diagnostics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SettingsSection {
    #[default]
    Appearance,
    Editor,
    LanguageTools,
    AiProviders,
    Extensions,
    Notifications,
    Privacy,
    Advanced,
}

impl SettingsSection {
    const ALL: [Self; 8] = [
        Self::Appearance,
        Self::Editor,
        Self::LanguageTools,
        Self::AiProviders,
        Self::Extensions,
        Self::Notifications,
        Self::Privacy,
        Self::Advanced,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::LanguageTools => "Language Tools",
            Self::AiProviders => "AI Providers",
            Self::Extensions => "Extensions",
            Self::Notifications => "Notifications",
            Self::Privacy => "Privacy",
            Self::Advanced => "Advanced",
        }
    }
}

/// Renderer-owned selection for the operational bottom panel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BottomPanelTab {
    /// Projected terminal/runtime stream.
    #[default]
    Terminal,
    /// Projected language-tooling problems.
    Problems,
    /// User-facing workspace activity, available in every product mode.
    Activity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactDrawer {
    Explorer,
    Inspector,
    BottomPanel,
}

impl Default for ProjectionView {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectionView {
    fn sync_typescript_toolchain_draft(
        &mut self,
        workspace_id: Option<WorkspaceId>,
        projection: &TypeScriptToolchainProjection,
    ) {
        if self.typescript_toolchain_workspace == workspace_id
            && self.typescript_toolchain_projection.as_ref() == Some(projection)
        {
            return;
        }
        self.typescript_toolchain_draft = projection
            .settings
            .as_ref()
            .map(|settings| TypeScriptToolchainDraft {
                server_archive: bounded_typescript_toolchain_path(
                    settings.server_archive.0.clone(),
                ),
                compiler_archive: bounded_typescript_toolchain_path(
                    settings.compiler_archive.0.clone(),
                ),
                node_executable: bounded_typescript_toolchain_path(
                    settings.node_executable.0.clone(),
                ),
            })
            .unwrap_or_default();
        self.typescript_toolchain_projection = Some(projection.clone());
        self.typescript_toolchain_workspace = workspace_id;
    }

    pub(crate) fn request_streamed_navigation(
        &mut self,
        identity: DesktopSourceIdentity,
        row: StreamedRequestedRow,
    ) {
        self.streamed_layout_cache
            .request_navigation(StreamedNavigationRequest { identity, row });
    }

    pub(crate) fn streamed_navigation_requests(&self) -> Vec<StreamedNavigationRequest> {
        self.streamed_layout_cache.streamed_navigation_requests()
    }

    pub(crate) fn streamed_navigation_rows(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedNavigationRows> {
        self.streamed_layout_cache.navigation_rows(identity)
    }

    #[cfg(test)]
    pub(crate) fn inject_streamed_navigation_error_for_test(
        &mut self,
        identity: DesktopSourceIdentity,
        row: StreamedRequestedRow,
        error: &str,
    ) {
        self.streamed_layout_cache
            .inject_navigation_error_for_test(identity, row, error);
    }

    /// Creates a projection view with no product-state ownership.
    pub fn new() -> Self {
        Self {
            theme_preference: theme::ThemePreference::all()[0],
            selected_activity: ActivitySurface::Explorer,
            utility_surface: None,
            settings_section: SettingsSection::Appearance,
            utility_overlay_origin: None,
            utility_overlay_needs_focus: false,
            utility_overlay_focus_bounds: None,
            utility_restore_focus: None,
            command_palette_origin: None,
            compact_drawer: None,
            compact_drawer_origin: None,
            compact_drawer_needs_focus: false,
            compact_drawer_restore_focus: None,
            last_editor_rect: None,
            last_shell_panel_rects: None,
            last_dock_measurement: None,
            pending_mode_confirmation: None,
            pending_mode_confirmation_source: None,
            pending_mode_confirmation_origin: None,
            pending_mode_confirmation_needs_focus: false,
            mode_confirmation_restore_focus: None,
            streamed_layout_cache: StreamedLayoutCachePool::default(),
            typescript_toolchain_draft: TypeScriptToolchainDraft::default(),
            typescript_toolchain_projection: None,
            typescript_toolchain_workspace: None,
        }
    }

    /// Returns the actual editor allocation recorded during the last render.
    #[doc(hidden)]
    pub fn last_editor_rect(&self) -> Option<egui::Rect> {
        self.last_editor_rect
    }

    /// Returns the physical panel allocations recorded during the last render.
    #[doc(hidden)]
    pub fn last_shell_panel_rects(&self) -> Option<ShellPanelRects> {
        self.last_shell_panel_rects
    }

    fn request_product_mode(
        &mut self,
        current: DockMode,
        target: DockMode,
        origin: egui::Id,
        actions: &mut Vec<DesktopAction>,
    ) {
        match mode_transition_policy(current, target) {
            ModeTransitionPolicy::NoAction => {}
            ModeTransitionPolicy::Immediate => {
                self.clear_mode_confirmation(false);
                actions.push(DesktopAction::SetProductMode { mode: target });
            }
            ModeTransitionPolicy::Confirm => {
                self.pending_mode_confirmation = Some(target);
                self.pending_mode_confirmation_source = Some(current);
                self.pending_mode_confirmation_origin = Some(origin);
                self.pending_mode_confirmation_needs_focus = true;
            }
        }
    }

    fn clear_mode_confirmation(&mut self, restore_focus: bool) {
        self.pending_mode_confirmation = None;
        self.pending_mode_confirmation_source = None;
        self.pending_mode_confirmation_needs_focus = false;
        let origin = self.pending_mode_confirmation_origin.take();
        self.mode_confirmation_restore_focus = if restore_focus { origin } else { None };
    }

    fn normalize_mode_confirmation(&mut self, projected_mode: DockMode) {
        if self.pending_mode_confirmation.is_some()
            && self.pending_mode_confirmation_source != Some(projected_mode)
        {
            self.clear_mode_confirmation(true);
        }
    }

    fn open_utility_overlay(&mut self, surface: UtilitySurface, origin: egui::Id) {
        debug_assert!(matches!(
            surface,
            UtilitySurface::Settings | UtilitySurface::Setup | UtilitySurface::About
        ));
        self.utility_surface = Some(surface);
        self.utility_overlay_origin = Some(origin);
        self.utility_overlay_needs_focus = true;
        self.utility_overlay_focus_bounds = None;
    }

    fn close_utility_overlay(&mut self, restore_focus: bool) {
        if !matches!(
            self.utility_surface,
            Some(UtilitySurface::Settings | UtilitySurface::Setup | UtilitySurface::About)
        ) {
            return;
        }
        self.utility_surface = None;
        self.utility_overlay_needs_focus = false;
        self.utility_overlay_focus_bounds = None;
        let origin = self.utility_overlay_origin.take();
        self.utility_restore_focus = if restore_focus { origin } else { None };
    }

    pub(crate) fn open_settings_from_palette(&mut self) {
        if let Some(origin) = self.command_palette_origin {
            self.open_utility_overlay(UtilitySurface::Settings, origin);
        }
    }

    /// Open the Help/About overlay from a palette dispatch.
    pub(crate) fn open_about_from_palette(&mut self) {
        if let Some(origin) = self.command_palette_origin {
            self.open_utility_overlay(UtilitySurface::About, origin);
        } else {
            self.utility_surface = Some(UtilitySurface::About);
            self.utility_overlay_needs_focus = true;
            self.utility_overlay_focus_bounds = None;
        }
    }

    fn open_compact_drawer(&mut self, drawer: CompactDrawer, origin: egui::Id) {
        self.compact_drawer = Some(drawer);
        self.compact_drawer_origin = Some(origin);
        self.compact_drawer_needs_focus = true;
        self.compact_drawer_restore_focus = None;
    }

    fn close_compact_drawer(&mut self, restore_focus: bool) {
        self.compact_drawer = None;
        self.compact_drawer_needs_focus = false;
        let origin = self.compact_drawer_origin.take();
        self.compact_drawer_restore_focus = if restore_focus { origin } else { None };
    }

    /// Renders the current projection snapshot into egui panels.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
    ) -> ProjectionViewOutput {
        self.render_with_state(ui, snapshot, &DesktopProjectionViewState::default())
    }

    /// Renders the current projection snapshot with adapter-local expansion state.
    pub fn render_with_state(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
    ) -> ProjectionViewOutput {
        self.render_with_state_and_source(ui, snapshot, state, None)
    }

    /// Render with an optional app-owned bounded source for oversized lines.
    pub fn render_with_state_and_source(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
        source: Option<&dyn DesktopLineSource>,
    ) -> ProjectionViewOutput {
        self.normalize_mode_confirmation(snapshot.product_mode);
        self.sync_typescript_toolchain_draft(
            snapshot.language_tooling_projection.workspace_id,
            &snapshot.language_tooling_projection.typescript_toolchain,
        );
        let mut selected_bottom_panel = state.selected_bottom_panel;
        // Filled in by the standard layout branch below. Compact layouts leave
        // it empty: their panel sizes are fixed, not user-arranged, and
        // recording them would overwrite the desktop arrangement whenever the
        // window was briefly made small.
        let mut observed_dock_fractions = dock_geometry::DockFractions::default();
        self.theme_preference =
            desktop_theme_preference(snapshot.settings_projection.theme_preference);
        let mut active_theme = self.theme_preference.resolve(ui.ctx());
        let settings = snapshot.settings_projection.clone().normalized();
        active_theme.typography.code = settings.editor_font_size_pt as u8;
        active_theme.typography.code_muted =
            settings.editor_font_size_pt.saturating_sub(1).max(11) as u8;
        ui.ctx()
            .set_zoom_factor(settings.zoom_percent as f32 / 100.0);
        theme::install(ui.ctx(), &active_theme);
        let current_interact_size = ui.style().spacing.interact_size;
        let minimum_target = f32::from(active_theme.control_height.compact);
        ui.style_mut().spacing.interact_size = egui::vec2(
            current_interact_size.x.max(minimum_target),
            current_interact_size.y.max(minimum_target),
        );
        let mut model = DesktopProjectionViewModel::from_snapshot_with_state(snapshot, state);
        model.bottom_tab_rows = bottom_tab_rows(
            snapshot,
            selected_bottom_panel,
            self.utility_surface == Some(UtilitySurface::Diagnostics),
        );
        let mut actions = Vec::new();
        let geometry =
            ShellGeometry::for_available_size(ui.available_width(), ui.available_height());

        let top = egui::Panel::top("legion_desktop_top")
            .exact_size(geometry.top_bar_height)
            .frame(theme::toolbar_frame())
            .show_inside(ui, |ui| {
                render_top_command_bar(ui, snapshot, &model, geometry, self, &mut actions);
            })
            .response
            .rect;

        let authority = egui::Panel::top("legion_desktop_authority")
            .exact_size(28.0)
            .frame(egui::Frame::NONE.fill(theme::tokens().surfaces.panel))
            .show_inside(ui, |ui| {
                render_authority_ribbon(ui, &model.authority_ribbon);
            })
            .response
            .rect;

        let status = egui::Panel::bottom("legion_desktop_status")
            .exact_size(geometry.status_bar_height)
            .frame(theme::status_frame(theme::tokens().bg.code))
            .show_inside(ui, |ui| {
                render_status_bar(ui, &model, geometry);
            })
            .response
            .rect;

        let (left, right, bottom) = if geometry.compact {
            egui::Panel::bottom("legion_desktop_compact_drawer_strip")
                .exact_size(28.0)
                .frame(theme::toolbar_frame())
                .show_inside(ui, |ui| render_compact_drawer_strip(ui, snapshot, self));
            let before_bottom = ui.available_rect_before_wrap();
            let bottom = if geometry.ultra_compact {
                egui::Rect::from_min_max(before_bottom.left_bottom(), before_bottom.right_bottom())
            } else {
                egui::Panel::bottom("legion_desktop_bottom_console")
                    .exact_size(geometry.bottom_height)
                    .frame(theme::pane_frame(theme::tokens().bg.code))
                    .show_inside(ui, |ui| {
                        render_bottom_console(
                            ui,
                            snapshot,
                            &model,
                            state.problems_selected_index,
                            &mut selected_bottom_panel,
                            self,
                            &mut actions,
                        );
                    })
                    .response
                    .rect
            };
            (
                egui::Rect::from_min_max(before_bottom.left_top(), before_bottom.left_bottom()),
                egui::Rect::from_min_max(before_bottom.right_top(), before_bottom.right_bottom()),
                bottom,
            )
        } else {
            let inspector_visible = projected_product_mode(snapshot) != DesktopProductMode::Manual;
            // The denominator for every splitter fraction this frame, captured
            // before any dock is placed. Measuring against `ui.available_*`
            // after a panel is added would give the *remaining* space, so a
            // fraction written on one frame would mean something different when
            // read on the next and the panels would creep on every restart.
            let dock_basis = ui.available_rect_before_wrap();
            // The size each dock was *asked* for this frame. A panel sitting at
            // the size we requested tells us nothing — only a difference means
            // the user moved the splitter. Comparing the rendered size against
            // the stored fraction instead would rewrite the user's preference
            // every time it got clamped, so merely making the window narrow
            // would destroy the arrangement permanently.
            // Panel frames consume one pixel on each horizontal edge. Reserve
            // that chrome in addition to the required 560 px editor canvas.
            let standard_editor_reserve = ShellGeometry::MIN_STANDARD_EDITOR_WIDTH + 2.0;
            let left_max_width = (ui.available_width()
                - if inspector_visible {
                    geometry.right_min_width
                } else {
                    0.0
                }
                - standard_editor_reserve)
                .max(geometry.left_min_width);
            let left_panel = egui::Panel::left("legion_desktop_explorer")
                .frame(theme::pane_frame(theme::tokens().bg.panel))
                .resizable(!geometry.compact);
            let left_panel = if geometry.compact {
                left_panel.exact_size(geometry.left_width)
            } else {
                // `default_size` is only consulted the first time egui lays this
                // panel out; afterwards egui remembers the user's drag in its
                // own memory. That memory is per-process, so on a fresh launch
                // this is what decides the width — which is exactly where a
                // restored fraction belongs.
                let left_default = dock_geometry::size_from_fraction(
                    dock_geometry::user_arranged_fraction(
                        &state.dock_layouts,
                        state.dock_layouts_user_arranged,
                        snapshot.product_mode,
                        legion_ui::DockSide::Left,
                    ),
                    dock_basis.width(),
                    geometry.left_width,
                    geometry.left_min_width,
                    left_max_width,
                );
                left_panel
                    .default_size(left_default)
                    .min_size(geometry.left_min_width)
                    .max_size(left_max_width)
            };
            let left = left_panel
                .show_inside(ui, |ui| {
                    render_left_sidebar(ui, snapshot, state, &model, geometry, self, &mut actions);
                })
                .response
                .rect;

            let right = if !inspector_visible {
                let remaining = ui.available_rect_before_wrap();
                egui::Rect::from_min_max(remaining.right_top(), remaining.right_bottom())
            } else {
                let right_max_width = (ui.available_width() - standard_editor_reserve)
                    .clamp(geometry.right_min_width, geometry.right_max_width);
                egui::Panel::right("legion_desktop_trust")
                    .frame(theme::pane_frame(theme::tokens().bg.panel))
                    .resizable(true)
                    .default_size(dock_geometry::size_from_fraction(
                        dock_geometry::user_arranged_fraction(
                            &state.dock_layouts,
                            state.dock_layouts_user_arranged,
                            snapshot.product_mode,
                            legion_ui::DockSide::Right,
                        ),
                        dock_basis.width(),
                        geometry.right_width,
                        geometry.right_min_width,
                        right_max_width,
                    ))
                    .min_size(geometry.right_min_width)
                    .max_size(right_max_width)
                    .show_inside(ui, |ui| {
                        render_right_dock(ui, snapshot, state, &model, self, &mut actions);
                    })
                    .response
                    .rect
            };

            let bottom_panel = egui::Panel::bottom("legion_desktop_bottom_console")
                .frame(theme::pane_frame(theme::tokens().bg.code))
                .resizable(!geometry.compact);
            let bottom_panel = if geometry.compact {
                bottom_panel.exact_size(geometry.bottom_height)
            } else {
                bottom_panel
                    .default_size(dock_geometry::size_from_fraction(
                        dock_geometry::user_arranged_fraction(
                            &state.dock_layouts,
                            state.dock_layouts_user_arranged,
                            snapshot.product_mode,
                            legion_ui::DockSide::Bottom,
                        ),
                        dock_basis.height(),
                        geometry.bottom_height,
                        geometry.bottom_min_height,
                        // The console may not swallow the editor; leave the
                        // canvas its documented minimum plus the chrome.
                        (dock_basis.height() - geometry.bottom_min_height)
                            .max(geometry.bottom_min_height),
                    ))
                    .min_size(geometry.bottom_min_height)
            };
            let bottom_content = bottom_panel
                .show_inside(ui, |ui| {
                    render_bottom_console(
                        ui,
                        snapshot,
                        &model,
                        state.problems_selected_index,
                        &mut selected_bottom_panel,
                        self,
                        &mut actions,
                    );
                })
                .response
                .rect;
            let bottom = egui::Rect::from_min_max(
                bottom_content.min,
                egui::pos2(bottom_content.right(), status.top()),
            );
            // A splitter drag is a change between consecutive frames, so that
            // is what gets reported. Compact layouts and the hidden Manual
            // inspector never reach here, so a panel that was not drawn stays
            // `None` rather than being recorded as a deliberate zero.
            let measurement = dock_geometry::DockMeasurement {
                basis_width: dock_basis.width(),
                basis_height: dock_basis.height(),
                left: Some(left.width()),
                right: inspector_visible.then(|| right.width()),
                bottom: Some(bottom_content.height()),
            };
            observed_dock_fractions =
                dock_geometry::dragged_fractions(measurement, self.last_dock_measurement);
            self.last_dock_measurement = Some(measurement);
            (left, right, bottom)
        };

        let _center_content = egui::CentralPanel::default()
            .frame(theme::pane_frame(theme::tokens().bg.code))
            .show_inside(ui, |ui| {
                // `last_editor_rect` stays populated whichever surface is up:
                // several suites and the panel-tiling gate assert against it,
                // and a canvas that returned nothing would fail them for a
                // reason unrelated to the canvas.
                self.last_editor_rect = Some(match state.center_surface {
                    CenterSurface::Editor => render_code_canvas(
                        ui,
                        snapshot,
                        &model,
                        &mut actions,
                        source,
                        &mut self.streamed_layout_cache,
                    ),
                    CenterSurface::Canvas => canvas_workspace::render_canvas_workspace(
                        ui,
                        snapshot,
                        &state.canvas_positions,
                        &state.canvas_edges,
                        &mut actions,
                    ),
                });
            })
            .response
            .rect;
        let center = egui::Rect::from_min_max(
            egui::pos2(left.right(), authority.bottom()),
            egui::pos2(right.left(), bottom.top()),
        );
        self.last_editor_rect = self.last_editor_rect.map(|rect| rect.intersect(center));
        self.last_shell_panel_rects = Some(ShellPanelRects {
            top,
            authority,
            status,
            left,
            right,
            bottom,
            center,
        });

        if geometry.compact {
            render_compact_drawer_overlay(
                ui.ctx(),
                snapshot,
                state,
                &model,
                geometry,
                self,
                &mut selected_bottom_panel,
                &mut actions,
            );
        } else {
            self.close_compact_drawer(false);
        }
        if let Some(origin) = self.compact_drawer_restore_focus.take() {
            ui.ctx().memory_mut(|memory| memory.request_focus(origin));
        }

        render_toast_overlay(ui.ctx(), &model, &mut actions);
        // Only over the editor.
        //
        // All three are overlays on the central region that belong to the
        // buffer. Two of them dispatch mutations and are reached by controls
        // rather than by keys -- the find bar's Replace and Replace All, and the
        // completion popup's Enter, Tab and row click -- so switching to the
        // canvas left them drawn and editing a file that was not on screen,
        // past a gate that only ever looked at key events.
        //
        // The hover tooltip mutates nothing, and was left out of this gate for
        // that reason. It still described a symbol in a file the canvas had
        // replaced: a tooltip that survives the thing it points at is a label
        // on the wrong object, and on this surface every card is a different
        // file it could plausibly belong to.
        if state.center_surface == CenterSurface::Editor {
            render_hover_tooltip(ui.ctx(), snapshot, state, &mut actions);
            render_completion_popup(ui.ctx(), snapshot, state, &mut actions);
            render_find_bar(ui.ctx(), snapshot, &mut actions);
        }
        if let Some(origin) = self.utility_restore_focus.take() {
            ui.ctx().memory_mut(|memory| memory.request_focus(origin));
        }
        render_utility_overlay(ui.ctx(), snapshot, &model, self, &mut actions);
        if let Some(origin) = self.mode_confirmation_restore_focus.take() {
            ui.ctx().memory_mut(|memory| memory.request_focus(origin));
        }
        // Shell-level, and last, alongside the other modal. It previously hung
        // off the end of the code canvas, which put it inside a panel whose
        // height was already fully allocated — so it rendered below the window
        // edge — and registered it before the panels drawn after it, so it did
        // not reliably win its own clicks either.
        render_close_dirty_prompt_modal(ui.ctx(), snapshot, &mut actions);
        render_mode_confirmation_dialog(ui.ctx(), ui.is_enabled(), self, &mut actions);
        model.bottom_tab_rows = bottom_tab_rows(
            snapshot,
            selected_bottom_panel,
            self.utility_surface == Some(UtilitySurface::Diagnostics),
        );

        ProjectionViewOutput {
            needs_repaint: false,
            displayed_title: model.layout_title,
            bottom_tab_rows: model.bottom_tab_rows,
            selected_bottom_panel,
            observed_dock_fractions,
            actions,
        }
    }
}

fn render_authority_ribbon(ui: &mut egui::Ui, model: &DesktopAuthorityRibbonViewModel) {
    let available_width = ui.available_width();
    ui.horizontal(|ui| {
        let summary = ui.add(
            egui::Label::new(theme::label(&model.summary)).wrap_mode(egui::TextWrapMode::Extend),
        );
        ui.ctx().accesskit_node_builder(summary.id, |node| {
            node.set_role(egui::accesskit::Role::Status);
        });
        for (minimum_width, detail) in [
            (680.0, model.approval_boundary.as_deref()),
            (820.0, model.provider_readiness.as_deref()),
            (960.0, model.workspace_scope.as_deref()),
        ] {
            if available_width >= minimum_width
                && let Some(detail) = detail
            {
                ui.separator();
                ui.add(
                    egui::Label::new(theme::muted(detail)).wrap_mode(egui::TextWrapMode::Extend),
                );
            }
        }
    });
}

fn render_compact_drawer_strip(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    view: &mut ProjectionView,
) {
    ui.horizontal_centered(|ui| {
        let mut drawers = vec![("Explorer drawer", CompactDrawer::Explorer)];
        if projected_product_mode(snapshot) != DesktopProductMode::Manual {
            drawers.push(("Inspector drawer", CompactDrawer::Inspector));
        }
        drawers.push(("Bottom panel drawer", CompactDrawer::BottomPanel));
        for (label, drawer) in drawers {
            let selected = view.compact_drawer == Some(drawer);
            let response = ui.add(
                egui::Button::new(theme::label(label))
                    .selected(selected)
                    .min_size(egui::vec2(
                        f32::from(theme::tokens().control_height.compact),
                        f32::from(theme::tokens().control_height.compact),
                    )),
            );
            if response.clicked() {
                if selected {
                    view.close_compact_drawer(true);
                } else {
                    view.open_compact_drawer(drawer, response.id);
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn render_compact_drawer_overlay(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    geometry: ShellGeometry,
    view: &mut ProjectionView,
    selected_bottom_panel: &mut BottomPanelTab,
    actions: &mut Vec<DesktopAction>,
) {
    let Some(drawer) = view.compact_drawer else {
        return;
    };
    let title = match drawer {
        CompactDrawer::Explorer => "Explorer",
        CompactDrawer::Inspector => "Inspector",
        CompactDrawer::BottomPanel => "Bottom panel",
    };
    let mut open = true;
    let mut close_requested = false;
    let request_initial_focus = std::mem::take(&mut view.compact_drawer_needs_focus);
    let escape_requested = ctx.input(|input| input.key_pressed(egui::Key::Escape))
        && !snapshot.palette_projection.open
        && !matches!(
            view.utility_surface,
            Some(UtilitySurface::Settings | UtilitySurface::Setup | UtilitySurface::About)
        )
        && view.pending_mode_confirmation.is_none();
    egui::Window::new(title)
        .id(egui::Id::new((
            "legion_desktop_compact_drawer",
            title,
            projected_product_mode(snapshot).label(),
        )))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width((ctx.content_rect().width() - 24.0).clamp(288.0, 420.0))
        .min_width(288.0)
        // egui's Window width excludes its 14 px outer resize/title chrome.
        .max_width(466.0)
        .max_height((ctx.content_rect().height() - 84.0).max(180.0))
        .show(ctx, |ui| {
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Dialog);
                node.set_label(format!("{title} drawer"));
            });
            let close = soft_button(ui, &format!("Close {title} drawer"));
            if request_initial_focus {
                close.request_focus();
            }
            close_requested = close.clicked();
            ui.separator();
            match drawer {
                CompactDrawer::Explorer => {
                    render_left_sidebar(ui, snapshot, state, model, geometry, view, actions);
                }
                CompactDrawer::Inspector => {
                    render_right_dock(ui, snapshot, state, model, view, actions);
                }
                CompactDrawer::BottomPanel => {
                    render_bottom_console(
                        ui,
                        snapshot,
                        model,
                        state.problems_selected_index,
                        selected_bottom_panel,
                        view,
                        actions,
                    );
                }
            }
        });
    if !open || close_requested || escape_requested {
        view.close_compact_drawer(true);
    }
}

fn render_top_command_bar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    geometry: ShellGeometry,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let level = projected_product_mode(snapshot);
    let composition = top_bar_composition(geometry);
    ui.set_max_height(geometry.top_bar_content_height());
    let available_width = ui.available_width();
    let (left_edge_width, right_edge_width) = if geometry.ultra_compact {
        (16.0, 74.0)
    } else if composition.density == TopBarDensity::Compact {
        (112.0, 112.0)
    } else {
        let edge = (available_width * 0.22).clamp(180.0, 280.0);
        (edge, edge)
    };
    let center_width = (available_width - left_edge_width - right_edge_width).max(0.0);
    let bar_rect = egui::Rect::from_min_size(
        ui.available_rect_before_wrap().min,
        egui::vec2(available_width, geometry.top_bar_content_height()),
    );
    ui.allocate_rect(bar_rect, egui::Sense::hover());
    let left_rect = egui::Rect::from_min_size(
        bar_rect.min,
        egui::vec2(left_edge_width, geometry.top_bar_content_height()),
    );
    let center_rect = egui::Rect::from_min_size(
        egui::pos2(left_rect.right(), bar_rect.top()),
        egui::vec2(center_width, geometry.top_bar_content_height()),
    );
    let right_rect = egui::Rect::from_min_max(
        egui::pos2(center_rect.right(), bar_rect.top()),
        bar_rect.max,
    );

    let mut left_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("legion_desktop_top_left")
            .max_rect(left_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    brand_mark::show(&mut left_ui, theme::tokens().accent.amber);
    if !geometry.ultra_compact {
        left_ui.label(theme::title(LEGION_WORDMARK));
    }
    if composition.shows_workspace_context {
        left_ui.label(theme::muted("·"));
        left_ui.label(theme::code_muted(trim_middle(&model.layout_title, 24)));
    }

    let mut center_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("legion_desktop_top_center")
            .max_rect(center_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if composition.shows_mode_switch {
        render_product_mode_switch(
            &mut center_ui,
            level,
            composition.density,
            geometry.ultra_compact,
            view,
            actions,
        );
    }

    let mut right_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("legion_desktop_top_right")
            .max_rect(right_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    if composition.shows_command_palette {
        let command = top_bar_command_button(&mut right_ui);
        view.command_palette_origin = Some(command.id);
        if command.clicked() {
            actions.push(command_palette_control_action());
        }
    }
    let presence_count = projected_presence_count_for_chrome(snapshot);
    if presence_count > 0 {
        right_ui.label(theme::code_muted(format!("{presence_count} present")));
    }
}

fn render_product_mode_switch(
    ui: &mut egui::Ui,
    active_level: DesktopProductMode,
    _density: TopBarDensity,
    ultra_compact: bool,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    let narrow = ultra_compact && ui.available_width() < 260.0;
    let button_width = |mode| product_mode_button_width(mode, ultra_compact, narrow);
    let switch_width = product_mode_switch_specs()
        .iter()
        .map(|spec| button_width(spec.mode))
        .sum::<f32>()
        + ui.spacing().item_spacing.x * 3.0
        + 8.0;
    let leading_space = ((ui.available_width() - switch_width) * 0.5).max(0.0);
    let focused_before_arrow = ui.ctx().memory(|memory| memory.focused());
    let (move_left, move_right) = ui.input(|input| {
        if input.modifiers.any() {
            (false, false)
        } else {
            (
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
            )
        }
    });
    egui::ScrollArea::horizontal()
        .id_salt("legion_desktop_product_mode_scroll")
        .show(ui, |ui| {
            ui.add_space(leading_space);
            egui::Frame::NONE
                .fill(tokens.surfaces.input)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(tokens.radius.md))
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut mode_ids = Vec::with_capacity(4);
                        for spec in product_mode_switch_specs() {
                            let canonical = canonical_mode_entry(spec.mode);
                            let active = spec.mode == active_level;
                            let color = level_color(spec.mode);
                            let label = product_mode_button_label(spec.mode, ultra_compact, narrow);
                            let response = ui
                                .push_id(("legion_desktop_product_mode", spec.ordinal), |ui| {
                                    ui.add_sized(
                                        [
                                            button_width(spec.mode),
                                            f32::from(tokens.control_height.standard),
                                        ],
                                        egui::Button::new(theme::accent(label, color))
                                            .selected(active),
                                    )
                                })
                                .inner;
                            mode_ids.push(response.id);
                            paint_control_focus_ring(ui, &response);
                            if response.has_focus() {
                                ui.ctx().memory_mut(|memory| {
                                    memory.set_focus_lock_filter(
                                        response.id,
                                        egui::EventFilter {
                                            horizontal_arrows: true,
                                            ..egui::EventFilter::default()
                                        },
                                    );
                                });
                            }
                            if response.gained_focus() {
                                ui.ctx().request_repaint();
                            }
                            ui.ctx().accesskit_node_builder(response.id, |node| {
                                node.set_label(canonical.label);
                                node.set_selected(active);
                                if active {
                                    node.set_aria_current(egui::accesskit::AriaCurrent::True);
                                } else {
                                    node.clear_aria_current();
                                }
                            });
                            if response.clicked() {
                                view.request_product_mode(
                                    active_level.to_dock_mode(),
                                    spec.mode.to_dock_mode(),
                                    response.id,
                                    actions,
                                );
                            }
                        }
                        if (move_left || move_right)
                            && let Some(index) = mode_ids
                                .iter()
                                .position(|id| Some(*id) == focused_before_arrow)
                        {
                            let next_index = if move_left {
                                index.saturating_sub(1)
                            } else {
                                (index + 1).min(mode_ids.len() - 1)
                            };
                            ui.ctx()
                                .memory_mut(|memory| memory.request_focus(mode_ids[next_index]));
                            ui.ctx().request_repaint();
                        }
                    });
                });
        });
}

fn paint_control_focus_ring(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            egui::CornerRadius::same(theme::tokens().radius.md),
            egui::Stroke::new(2.0_f32, theme::tokens().focus.ring),
            egui::epaint::StrokeKind::Inside,
        );
    }
}

fn product_mode_button_label(
    mode: DesktopProductMode,
    _ultra_compact: bool,
    _narrow: bool,
) -> &'static str {
    // Keep the visible label canonical at every density. The switch can
    // scroll horizontally in compact layouts, while full labels keep the
    // visual and accessibility representations consistent.
    canonical_mode_entry(mode).label
}

fn product_mode_button_width(mode: DesktopProductMode, ultra_compact: bool, narrow: bool) -> f32 {
    if !ultra_compact {
        return 116.0;
    }
    if narrow {
        match mode {
            DesktopProductMode::Manual
            | DesktopProductMode::Assist
            | DesktopProductMode::Delegate => 32.0,
            DesktopProductMode::LegionWorkflows => 60.0,
        }
    } else {
        match mode {
            DesktopProductMode::Manual | DesktopProductMode::Assist => 44.0,
            DesktopProductMode::Delegate => 48.0,
            DesktopProductMode::LegionWorkflows => 64.0,
        }
    }
}

const MODE_CONFIRMATION_BODY: &str = "Execution remains proposal-mediated and limited to bounded permissions. This presentation confirmation grants no permissions; operation-level app gates remain authoritative.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeTransitionPolicy {
    NoAction,
    Immediate,
    Confirm,
}

fn mode_transition_policy(current: DockMode, target: DockMode) -> ModeTransitionPolicy {
    match (current, target) {
        (DockMode::Manual, DockMode::Manual)
        | (DockMode::Assist, DockMode::Assist)
        | (DockMode::Delegate, DockMode::Delegate)
        | (DockMode::Automate, DockMode::Automate) => ModeTransitionPolicy::NoAction,
        (DockMode::Assist, DockMode::Manual)
        | (DockMode::Delegate, DockMode::Manual)
        | (DockMode::Automate, DockMode::Manual)
        | (DockMode::Manual, DockMode::Assist)
        | (DockMode::Delegate, DockMode::Assist)
        | (DockMode::Automate, DockMode::Assist)
        | (DockMode::Automate, DockMode::Delegate) => ModeTransitionPolicy::Immediate,
        (DockMode::Manual, DockMode::Delegate)
        | (DockMode::Assist, DockMode::Delegate)
        | (DockMode::Manual, DockMode::Automate)
        | (DockMode::Assist, DockMode::Automate)
        | (DockMode::Delegate, DockMode::Automate) => ModeTransitionPolicy::Confirm,
    }
}

fn mode_confirmation_title(target: DockMode) -> String {
    format!("Confirm {} mode", target.label())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeConfirmationDecision {
    Confirm,
    Cancel,
}

fn render_mode_confirmation_dialog(
    ctx: &egui::Context,
    controls_enabled: bool,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let Some(target) = view.pending_mode_confirmation else {
        return;
    };
    let title = mode_confirmation_title(target);
    let content_rect = ctx.content_rect();
    let dialog_width = (content_rect.width() - 48.0).clamp(280.0, 420.0);
    let dialog_height = 240.0;
    let dialog_offset = egui::vec2(
        (content_rect.width() - dialog_width) * 0.5,
        (content_rect.height() - dialog_height) * 0.5,
    );
    let dialog_id = egui::Id::new("legion_desktop_mode_confirmation");
    let mut decision = None;
    let modal = egui::Modal::new(dialog_id)
        .area(
            egui::Modal::default_area(dialog_id)
                .anchor(egui::Align2::LEFT_TOP, dialog_offset)
                .default_size([dialog_width, dialog_height]),
        )
        .frame(theme::pane_frame(theme::tokens().bg.panel))
        .show(ctx, |ui| {
            if !controls_enabled {
                ui.disable();
            }
            ui.set_min_width(dialog_width);
            ctx.accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Dialog);
                node.set_label(title.clone());
                node.set_description(MODE_CONFIRMATION_BODY);
                node.set_modal();
            });

            ui.label(theme::title(&title));
            ui.add_space(6.0);
            ui.label(theme::body(MODE_CONFIRMATION_BODY));
            ui.add_space(10.0);
            ui.label(theme::muted(
                "Mode selection alone does not start execution, grant a capability, or apply a change.",
            ));
            ui.add_space(12.0);
            let focused_before_tab = ui.ctx().memory(|memory| memory.focused());
            let tab_forward = ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
            });
            let tab_backward = ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
            });
            ui.horizontal(|ui| {
                let confirm = ui
                    .push_id("legion_desktop_mode_confirmation_confirm", |ui| {
                        ui.add(
                            egui::Button::new(theme::accent(
                                "Confirm",
                                theme::tokens().accent.amber,
                            ))
                            .min_size(egui::vec2(112.0, 28.0)),
                        )
                    })
                    .inner;
                paint_control_focus_ring(ui, &confirm);
                if view.pending_mode_confirmation_needs_focus {
                    confirm.request_focus();
                    view.pending_mode_confirmation_needs_focus = false;
                }
                if confirm.clicked() {
                    decision = Some(ModeConfirmationDecision::Confirm);
                }

                let cancel = ui
                    .push_id("legion_desktop_mode_confirmation_cancel", |ui| {
                        ui.add(egui::Button::new("Cancel").min_size(egui::vec2(112.0, 28.0)))
                    })
                    .inner;
                paint_control_focus_ring(ui, &cancel);
                if cancel.clicked() {
                    decision = Some(ModeConfirmationDecision::Cancel);
                }

                if tab_forward || tab_backward {
                    let next = if tab_backward {
                        if focused_before_tab == Some(confirm.id) {
                            cancel.id
                        } else {
                            confirm.id
                        }
                    } else if focused_before_tab == Some(cancel.id) {
                        confirm.id
                    } else {
                        cancel.id
                    };
                    ui.ctx().memory_mut(|memory| memory.request_focus(next));
                }
            });
        });

    // This dialog is renderer presentation, not the execution security boundary.
    // Operation-level app gates remain authoritative after the mode projection changes.
    match decision {
        Some(ModeConfirmationDecision::Confirm) => {
            view.clear_mode_confirmation(false);
            actions.push(DesktopAction::SetProductMode { mode: target });
        }
        Some(ModeConfirmationDecision::Cancel) => {
            view.clear_mode_confirmation(true);
        }
        None if modal.should_close() => {
            view.clear_mode_confirmation(true);
        }
        None => {}
    }
}

fn render_left_sidebar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    geometry: ShellGeometry,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(geometry.activity_rail_width, ui.available_height()),
            egui::Layout::top_down(egui::Align::Center),
            |ui| render_activity_rail(ui, snapshot, state, geometry, view, actions),
        );
        ui.separator();
        ui.vertical(|ui| {
            render_activity_sidebar(ui, snapshot, state, model, view.selected_activity, actions)
        });
    });
}

/// What a rail button shows: a drawn icon, or a character that does render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RailGlyph {
    /// Painted with the `rail_icons` geometry — no font involved.
    Drawn(rail_icons::RailIcon),
    /// A character verified to exist in the bundled font set.
    Text(&'static str),
}

/// One activity-rail button, sized and styled identically whichever kind of
/// glyph it carries, so a drawn icon and a character sit on the same grid.
fn render_rail_button(ui: &mut egui::Ui, glyph: RailGlyph, selected: bool) -> egui::Response {
    match glyph {
        RailGlyph::Text(text) => ui.add_sized(
            RAIL_BUTTON_SIZE,
            egui::Button::new(theme::label(text)).selected(selected),
        ),
        RailGlyph::Drawn(icon) => {
            let response = ui.add_sized(RAIL_BUTTON_SIZE, egui::Button::new("").selected(selected));
            // Painted after the button so the icon sits above its background;
            // the colour follows the same states the button's own text would.
            let color = if selected || response.hovered() {
                theme::tokens().text.primary
            } else {
                theme::tokens().text.muted
            };
            rail_icons::paint(ui.painter(), icon, response.rect, color);
            response
        }
    }
}

/// Rail buttons are a fixed size so the column reads as a column.
const RAIL_BUTTON_SIZE: [f32; 2] = [38.0, 28.0];

fn render_activity_rail(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    geometry: ShellGeometry,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let scope = snapshot.search_projection.scope;
    for (surface, label, glyph, palette) in [
        (
            ActivitySurface::Explorer,
            "Explorer",
            RailGlyph::Drawn(rail_icons::RailIcon::Explorer),
            None,
        ),
        (
            ActivitySurface::Search,
            "Search",
            RailGlyph::Drawn(rail_icons::RailIcon::Search),
            Some((PaletteMode::Search, "/")),
        ),
        (
            ActivitySurface::Symbols,
            "Symbols",
            RailGlyph::Text("ƒ"),
            Some((PaletteMode::Symbol, "")),
        ),
        (
            ActivitySurface::SourceControl,
            "Source Control",
            RailGlyph::Drawn(rail_icons::RailIcon::SourceControl),
            None,
        ),
        (
            ActivitySurface::Tests,
            "Tests",
            RailGlyph::Drawn(rail_icons::RailIcon::Tests),
            None,
        ),
        (
            ActivitySurface::Debug,
            "Run and Debug",
            RailGlyph::Drawn(rail_icons::RailIcon::Debug),
            None,
        ),
    ] {
        let response = ui
            .push_id(("legion_desktop_activity", label), |ui| {
                render_rail_button(ui, glyph, view.selected_activity == surface)
            })
            .inner
            .on_hover_text(label);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(label);
        });
        if response.clicked() {
            view.selected_activity = surface;
            // Source Control has no content until something asks the app for
            // it: the git projection is populated *only* by an explicit
            // `RefreshGit` command, and nothing issued one on workspace open or
            // on selecting the surface. Opening the panel in a repository with
            // uncommitted work therefore read "No source-control status", and
            // the remote verbs (gated on a projected branch label) rendered as
            // an empty row -- a panel that says a dirty repository is clean,
            // which is the worst thing a source-control view can say.
            //
            // Refreshing on the click is the same contract the Search and
            // Symbols entries already have in this table: selecting the surface
            // dispatches the action that gives it something to show. It is one
            // action per click, not per frame, so it cannot spin.
            if surface == ActivitySurface::SourceControl {
                actions.push(DesktopAction::RefreshGit);
            }
            if let Some((mode, query)) = palette {
                let query = if mode == PaletteMode::Search
                    && !snapshot.search_projection.query_label.trim().is_empty()
                {
                    snapshot.search_projection.query_label.clone()
                } else {
                    query.to_string()
                };
                actions.push(DesktopAction::OpenPalette { mode, query, scope });
            }
        }
    }
    // Canvas is a *centre* switch, not a side-panel one, so it is a toggle
    // rather than a member of the selection above: choosing Explorer while the
    // canvas is up should change the sidebar and leave the canvas alone.
    let canvas = ui
        .push_id(("legion_desktop_activity", "Canvas"), |ui| {
            render_rail_button(
                ui,
                RailGlyph::Text("◳"),
                state.center_surface == CenterSurface::Canvas,
            )
        })
        .inner
        .on_hover_text("Canvas");
    ui.ctx().accesskit_node_builder(canvas.id, |node| {
        node.set_label("Canvas");
    });
    if canvas.clicked() {
        actions.push(DesktopAction::SetCenterSurface {
            surface: match state.center_surface {
                CenterSurface::Editor => CenterSurface::Canvas,
                CenterSurface::Canvas => CenterSurface::Editor,
            },
        });
    }
    ui.separator();
    let diagnostics = ui
        .push_id(("legion_desktop_utility", "Diagnostics"), |ui| {
            render_rail_button(
                ui,
                RailGlyph::Drawn(rail_icons::RailIcon::Diagnostics),
                view.utility_surface == Some(UtilitySurface::Diagnostics),
            )
        })
        .inner
        .on_hover_text("Diagnostics");
    ui.ctx().accesskit_node_builder(diagnostics.id, |node| {
        node.set_label("Diagnostics");
    });
    if diagnostics.clicked() {
        view.utility_surface = Some(UtilitySurface::Diagnostics);
        if geometry.ultra_compact {
            view.compact_drawer = Some(CompactDrawer::BottomPanel);
            view.compact_drawer_needs_focus = true;
        }
    }
    for (surface, label, glyph) in [
        (UtilitySurface::Setup, "Setup", "?"),
        (UtilitySurface::Settings, "Settings", "⚙"),
    ] {
        let response = ui
            .push_id(("legion_desktop_utility", label), |ui| {
                render_rail_button(
                    ui,
                    RailGlyph::Text(glyph),
                    view.utility_surface == Some(surface),
                )
            })
            .inner
            .on_hover_text(label);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(label);
        });
        if response.clicked() {
            response.request_focus();
            view.open_utility_overlay(surface, response.id);
            if surface == UtilitySurface::Settings {
                actions.push(DesktopAction::OpenSettings);
            }
        }
    }
}

fn render_activity_sidebar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    selected: ActivitySurface,
    actions: &mut Vec<DesktopAction>,
) {
    match selected {
        ActivitySurface::Explorer => {
            render_explorer_sidebar(ui, snapshot, state, model, actions);
        }
        ActivitySurface::Search => {
            sidebar_header(ui, "SEARCH", model.layout_title.clone());
            ui.label(theme::muted("Search results appear in the search palette."));
        }
        ActivitySurface::Symbols => {
            sidebar_header(ui, "SYMBOLS", model.layout_title.clone());
            render_compact_rows(ui, &symbol_rows(snapshot), "No symbols in this file", 12);
        }
        ActivitySurface::SourceControl => {
            sidebar_header(ui, "SOURCE CONTROL", model.layout_title.clone());
            // Scrolled, because this surface is the tallest in the rail: up to
            // twelve hunk controls, then the untracked note, then conflict
            // actions, then twelve status rows. In a plain vertical `Ui` the
            // lower ones are simply clipped on a short window or at high
            // display zoom -- and conflict resolution is among them, which is
            // the control a person needs most and can least afford to lose.
            egui::ScrollArea::vertical()
                .id_salt("legion_desktop_source_control_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    render_git_controls(ui, snapshot, actions);
                    render_compact_rows(ui, &model.git_rows, "No source-control status", 12);
                });
        }
        ActivitySurface::Tests => {
            sidebar_header(ui, "TESTS", model.layout_title.clone());
            render_test_controls(ui, snapshot, actions);
            render_compact_rows(ui, &model.test_rows, "No tests discovered", 12);
        }
        ActivitySurface::Debug => {
            sidebar_header(ui, "RUN AND DEBUG", model.layout_title.clone());
            render_debug_controls(ui, snapshot, actions);
            render_debug_inspector(ui, snapshot, actions);
            render_compact_rows(ui, &model.debug_rows, "No debug configurations", 12);
        }
    }
}

fn render_explorer_sidebar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    sidebar_header(ui, "EXPLORER ·", model.layout_title.clone());
    render_project_tree_panel(ui, snapshot, state, actions);
}

fn render_code_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
    source: Option<&dyn DesktopLineSource>,
    streamed_cache: &mut StreamedLayoutCachePool,
) -> egui::Rect {
    render_advanced_center_surface(ui, snapshot, model, actions);
    render_editor_canvas(ui, snapshot, model, actions, source, streamed_cache)
}

fn render_advanced_center_surface(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    let mode = projected_product_mode(snapshot);
    if !matches!(model.mode_surface.center, SurfaceAvailability::Ready) {
        return;
    }
    let header_height = ui.spacing().interact_size.y;
    let body_height = (ui.available_height()
        - MIN_USABLE_EDITOR_HEIGHT
        - EDITOR_CHROME_HEIGHT_RESERVE
        - header_height)
        .clamp(96.0, MAX_ADVANCED_WORKBENCH_HEIGHT);
    if mode == DesktopProductMode::LegionWorkflows {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), body_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_fleet_canvas(ui, snapshot, model, actions),
        );
        return;
    }
    let label = match mode {
        DesktopProductMode::Manual | DesktopProductMode::Assist => return,
        DesktopProductMode::Delegate => "Delegate workbench",
        DesktopProductMode::LegionWorkflows => unreachable!("handled above"),
    };
    disclosure_row(
        ui,
        label,
        ("legion_desktop_advanced_center_surface", mode.label()),
        false,
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("legion_desktop_advanced_center_scroll", mode.label()))
                .max_height(body_height)
                .auto_shrink([false, true])
                .show(ui, |ui| match projected_product_mode(snapshot) {
                    DesktopProductMode::Manual | DesktopProductMode::Assist => {}
                    DesktopProductMode::Delegate => {
                        render_delegated_canvas(ui, snapshot, model, actions)
                    }
                    DesktopProductMode::LegionWorkflows => {}
                });
        },
    );
}

fn render_right_dock(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let mode = projected_product_mode(snapshot);
    egui::ScrollArea::vertical()
        .id_salt(("legion_desktop_right_rail_scroll", mode.label()))
        .auto_shrink([false, false])
        .show(ui, |ui| match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => {}
            DesktopProductMode::Assist => render_assist_rail(ui, snapshot, model, view, actions),
            DesktopProductMode::Delegate => {
                if delegated_task_owned_state_projected(snapshot) {
                    render_delegation_console(ui, snapshot, state, model, actions)
                } else if matches!(
                    model.mode_surface.inspector,
                    SurfaceAvailability::Blocked { .. }
                ) {
                    render_delegate_prerequisite_rail(ui, model)
                } else {
                    render_delegate_draft_rail(ui, snapshot, state, model, actions)
                }
            }
            DesktopProductMode::LegionWorkflows => {
                render_fleet_console(ui, snapshot, model, actions)
            }
        });
}

fn render_bottom_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    problems_selected_index: usize,
    selected: &mut BottomPanelTab,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let diagnostics_active = view.utility_surface == Some(UtilitySurface::Diagnostics);
    ui.horizontal(|ui| {
        for tab in bottom_tab_specs(snapshot, *selected, diagnostics_active) {
            let label = if let Some(count) = tab.count {
                format!("{} ({count})", tab.label)
            } else {
                tab.label.to_string()
            };
            if console_tab(ui, &label, tab.active, tab.color).clicked() {
                if let Some(selection) = tab.selection {
                    *selected = selection;
                    view.utility_surface = None;
                } else {
                    view.utility_surface = Some(UtilitySurface::Diagnostics);
                }
            }
        }
    });
    ui.separator();
    if view.utility_surface == Some(UtilitySurface::Diagnostics) {
        render_diagnostics_panel(ui, model);
    } else {
        match *selected {
            BottomPanelTab::Terminal => render_terminal_stream(ui, snapshot, model, actions),
            BottomPanelTab::Problems => {
                section_label(ui, "Problems", Some(theme::tokens().accent.red));
                theme::code_frame().show(ui, |ui| {
                    render_problem_rows(ui, snapshot, problems_selected_index, actions);
                });
            }
            BottomPanelTab::Activity => render_activity_stream(ui, snapshot, model),
        }
    }
}

fn render_status_bar(
    ui: &mut egui::Ui,
    model: &DesktopProjectionViewModel,
    geometry: ShellGeometry,
) {
    let status = &model.status_bar;
    ui.set_max_height(geometry.status_bar_content_height());
    ui.horizontal(|ui| {
        ui.label(theme::accent(
            &status.product_mode,
            theme::tokens().accent.amber,
        ));
        if let Some(trust) = &status.trust {
            ui.separator();
            ui.label(theme::code_muted(format!("trust: {trust}")));
        }
        if let Some(lsp) = &status.lsp {
            ui.separator();
            ui.label(theme::code_muted(format!("LSP: {lsp}")));
        }
        if let Some(path) = &status.path {
            ui.separator();
            ui.label(theme::code_muted(trim_middle(path, 56)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(cursor) = status.cursor {
                ui.label(theme::code_muted(format!(
                    "Ln {}, Col {}",
                    cursor.line, cursor.column
                )));
            }
        });
    });
}

fn render_toast_overlay(
    ctx: &egui::Context,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    if model.toast_stack.visible.is_empty() && model.toast_stack.overflow_count == 0 {
        return;
    }

    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_toast_stack".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -40.0))
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.vertical(|ui| {
                for toast in &model.toast_stack.visible {
                    let accent = toast_accent_color(toast.severity);
                    egui::Frame::new()
                        .fill(tokens.bg.panel)
                        .stroke(egui::Stroke::new(1.0_f32, accent))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    status_dot(ui, accent);
                                    ui.label(theme::body_strong(&toast.title));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if soft_button(ui, "Dismiss").clicked() {
                                                actions.push(DesktopAction::DismissToast {
                                                    toast_id: toast.id,
                                                });
                                            }
                                        },
                                    );
                                });
                                if let Some(body) = &toast.body {
                                    ui.add_space(3.0);
                                    ui.label(theme::muted(body));
                                }
                                if let Some(action) = &toast.action {
                                    ui.add_space(6.0);
                                    if soft_button(ui, &action.label).clicked() {
                                        actions.push(DesktopAction::InvokeToastAction {
                                            intent: action.intent.clone(),
                                        });
                                    }
                                }
                            });
                        });
                    ui.add_space(8.0);
                }
                if model.toast_stack.overflow_count > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::muted(format!(
                            "+{} more notifications",
                            model.toast_stack.overflow_count
                        )));
                    });
                }
            });
        });
}

/// Render the LSP completion popup overlay (T6).
///
/// Visible only when `state.completion_popup_open` is true AND the snapshot
/// carries at least one projected completion item.  Keyboard actions
/// (↓ next, ↑ prev, Tab/Enter accept, Esc dismiss) are appended to `actions`
/// so the runtime can handle them through the normal `handle_action` path.
fn render_completion_popup(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    if !state.completion_popup_open {
        return;
    }
    let completions = &snapshot.language_tooling_projection.completions;
    if completions.is_empty() {
        return;
    }

    // Keyboard navigation — consume before the popup frame so the editor does
    // not also receive these keys.
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            actions.push(DesktopAction::CompletionDismiss);
        }
        if i.key_pressed(egui::Key::ArrowDown) {
            actions.push(DesktopAction::CompletionNext);
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            actions.push(DesktopAction::CompletionPrev);
        }
        if i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter) {
            actions.push(DesktopAction::CompletionAccept);
        }
    });

    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_completion_popup".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(320.0, -60.0))
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .id_salt("completion_popup_scroll")
                        .show(ui, |ui| {
                            for (i, completion) in completions.iter().enumerate().take(10) {
                                let selected = i == state.completion_selected_index;
                                let bg = if selected {
                                    tokens.accent.blue.linear_multiply(0.2)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                let response = egui::Frame::new()
                                    .fill(bg)
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.set_min_width(280.0);
                                        ui.horizontal(|ui| {
                                            ui.label(theme::muted(format!(
                                                "[{}]",
                                                completion.kind_label
                                            )));
                                            ui.add_space(4.0);
                                            ui.label(if selected {
                                                theme::body_strong(&completion.label)
                                            } else {
                                                theme::body(&completion.label)
                                            });
                                            if let Some(detail) = &completion.detail_label {
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(theme::code_muted(detail));
                                                    },
                                                );
                                            }
                                        });
                                    })
                                    .response;
                                if response.clicked() {
                                    actions.push(DesktopAction::CompletionAccept);
                                }
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(theme::muted("↑↓ navigate  Tab accept  Esc dismiss"));
                    });
                });
        });
}

/// Render the LSP hover tooltip overlay (T7).
///
/// Visible only when `state.hover_tooltip_visible` is true AND the snapshot
/// carries hover data.  Esc appends `HoverDismiss` to `actions`.
/// The tooltip is redaction-safe: it shows `label` and `summary` fields from
/// `LanguageHoverProjection` which are already bounded/redacted by the app layer.
fn render_hover_tooltip(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    if !state.hover_tooltip_visible {
        return;
    }
    let Some(hover) = &snapshot.language_tooling_projection.hover else {
        return;
    };

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            actions.push(DesktopAction::HoverDismiss);
        }
    });

    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_hover_tooltip".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(320.0, -160.0))
        .show(ctx, |ui| {
            ui.set_max_width(400.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::body_strong(&hover.label));
                        if !hover.summary.is_empty() {
                            ui.add_space(4.0);
                            ui.label(theme::code_muted(&hover.summary));
                        }
                        if hover.degraded {
                            ui.add_space(2.0);
                            ui.label(theme::muted("(degraded)"));
                        }
                        ui.add_space(4.0);
                        ui.label(theme::muted("Esc dismiss"));
                    });
                });
        });
}

/// Map a string key label from `default_keymap()` to the corresponding `egui::Key`.
///
/// Only maps keys actually used in the default keymap.  Returns `None` for
/// unrecognised labels so the dispatch loop simply skips them.
fn key_label_to_egui(label: &str) -> Option<egui::Key> {
    match label {
        "S" => Some(egui::Key::S),
        "F" => Some(egui::Key::F),
        "H" => Some(egui::Key::H),
        "G" => Some(egui::Key::G),
        "P" => Some(egui::Key::P),
        "Z" => Some(egui::Key::Z),
        "W" => Some(egui::Key::W),
        "Tab" => Some(egui::Key::Tab),
        "F3" => Some(egui::Key::F3),
        "F5" => Some(egui::Key::F5),
        "F8" => Some(egui::Key::F8),
        "F9" => Some(egui::Key::F9),
        "F10" => Some(egui::Key::F10),
        "F11" => Some(egui::Key::F11),
        "F12" => Some(egui::Key::F12),
        "Escape" => Some(egui::Key::Escape),
        "ArrowUp" => Some(egui::Key::ArrowUp),
        "ArrowDown" => Some(egui::Key::ArrowDown),
        _ => None,
    }
}

fn active_buffer_for_keybinding(snapshot: &ShellProjectionSnapshot) -> Option<BufferId> {
    snapshot
        .daily_editing_projection
        .tabs
        .active_buffer_id
        .or(snapshot.active_buffer_projection.buffer_id)
}

fn adjacent_tab_for_keybinding(
    snapshot: &ShellProjectionSnapshot,
    direction: isize,
) -> Option<BufferId> {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return active_buffer_for_keybinding(snapshot);
    }
    let active = active_buffer_for_keybinding(snapshot)?;
    let active_index = tabs
        .iter()
        .position(|tab| tab.buffer_id == active)
        .or_else(|| tabs.iter().position(|tab| tab.active))
        .unwrap_or(0);
    let next = (active_index as isize + direction).rem_euclid(tabs.len() as isize) as usize;
    Some(tabs[next].buffer_id)
}

/// Adapter-local find bar text state stored in egui transient data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FindBarTextState {
    query: String,
    replace: String,
    was_visible: bool,
}

/// Render the in-editor find/replace bar overlay.
///
/// Anchored to the top-right of the editor area via `egui::Area`.  Only visible
/// when the snapshot's `find_bar_projection.visible` is true.  Emits find/replace
/// `DesktopAction` variants for query changes, navigation, and replace operations.
///
/// Text edit state is stored in egui transient data (not in `DesktopProjectionViewState`)
/// so the function works with an immutable `state` reference.
fn render_find_bar(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let find_bar = &snapshot.find_bar_projection;
    let state_id = egui::Id::new("legion_find_bar_text_state");

    if !find_bar.visible {
        // Clear was_visible when the bar is hidden.
        ctx.data_mut(|d| {
            if let Some(mut s) = d.get_temp::<FindBarTextState>(state_id) {
                s.was_visible = false;
                d.insert_temp(state_id, s);
            }
        });
        return;
    }

    // Load or initialize the local text state.
    let mut text_state: FindBarTextState =
        ctx.data_mut(|d| d.get_temp(state_id).unwrap_or_default());
    let just_opened = !text_state.was_visible;
    if just_opened {
        text_state.query = find_bar.query.clone();
        text_state.replace = find_bar.replace_text.clone();
    }
    text_state.was_visible = true;

    let tokens = theme::tokens();
    let find_bar_id = egui::Id::new("legion_find_bar");

    // Keyboard handling consumed before the popup frame.
    let mut enter_pressed = false;
    let mut shift_enter_pressed = false;
    let mut escape_pressed = false;
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            escape_pressed = true;
        }
        if i.key_pressed(egui::Key::Enter) && i.modifiers.shift {
            shift_enter_pressed = true;
        } else if i.key_pressed(egui::Key::Enter) {
            enter_pressed = true;
        }
    });

    if escape_pressed {
        actions.push(DesktopAction::CloseFindBar);
        ctx.data_mut(|d| d.insert_temp(state_id, text_state));
        return;
    }
    if shift_enter_pressed {
        actions.push(DesktopAction::FindPrevious);
    } else if enter_pressed {
        actions.push(DesktopAction::FindNext);
    }

    egui::Area::new(find_bar_id)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 8.0))
        .show(ctx, |ui| {
            ui.set_max_width(400.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    // Find row.
                    ui.horizontal(|ui| {
                        let query_response = ui.add(interactive_fields::find_bar_text_edit(
                            &mut text_state.query,
                            "Find...",
                            egui::Id::new("legion_find_query_input"),
                        ));
                        if just_opened {
                            query_response.request_focus();
                        }
                        if query_response.changed() {
                            actions.push(DesktopAction::SetFindQuery {
                                query: text_state.query.clone(),
                            });
                        }

                        // Match counter.
                        if find_bar.match_count > 0 {
                            ui.label(theme::muted(format!(
                                "{} of {}",
                                find_bar.current_match_index + 1,
                                find_bar.match_count
                            )));
                        } else if !find_bar.query.is_empty() {
                            ui.label(theme::muted("No results"));
                        }

                        // Navigation buttons.
                        if ui
                            .small_button("\u{25B2}")
                            .on_hover_text("Previous (Shift+Enter)")
                            .clicked()
                        {
                            actions.push(DesktopAction::FindPrevious);
                        }
                        if ui
                            .small_button("\u{25BC}")
                            .on_hover_text("Next (Enter)")
                            .clicked()
                        {
                            actions.push(DesktopAction::FindNext);
                        }

                        // Option toggles.
                        let case_label = if find_bar.case_sensitive {
                            theme::body_strong("Aa")
                        } else {
                            theme::muted("Aa")
                        };
                        if ui
                            .small_button(case_label)
                            .on_hover_text("Case sensitive")
                            .clicked()
                        {
                            actions.push(DesktopAction::SetFindCaseSensitive {
                                enabled: !find_bar.case_sensitive,
                            });
                        }
                        let word_label = if find_bar.whole_word {
                            theme::body_strong("W")
                        } else {
                            theme::muted("W")
                        };
                        if ui
                            .small_button(word_label)
                            .on_hover_text("Whole word")
                            .clicked()
                        {
                            actions.push(DesktopAction::SetFindWholeWord {
                                enabled: !find_bar.whole_word,
                            });
                        }
                        let regex_label = if find_bar.use_regex {
                            theme::body_strong(".*")
                        } else {
                            theme::muted(".*")
                        };
                        if ui
                            .small_button(regex_label)
                            .on_hover_text("Regex")
                            .clicked()
                        {
                            actions.push(DesktopAction::SetFindRegex {
                                enabled: !find_bar.use_regex,
                            });
                        }

                        // Toggle replace visibility.
                        if ui
                            .small_button(if find_bar.replace_visible {
                                "\u{25B4}"
                            } else {
                                "\u{25BE}"
                            })
                            .on_hover_text("Toggle replace")
                            .clicked()
                        {
                            actions.push(DesktopAction::ToggleFindReplace);
                        }

                        // Close button.
                        if ui
                            .small_button("\u{2715}")
                            .on_hover_text("Close (Esc)")
                            .clicked()
                        {
                            actions.push(DesktopAction::CloseFindBar);
                        }
                    });

                    // Replace row (only when visible).
                    if find_bar.replace_visible {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let replace_response = ui.add(interactive_fields::find_bar_text_edit(
                                &mut text_state.replace,
                                "Replace...",
                                egui::Id::new("legion_find_replace_input"),
                            ));
                            if replace_response.changed() {
                                actions.push(DesktopAction::SetFindReplaceText {
                                    text: text_state.replace.clone(),
                                });
                            }

                            if ui
                                .small_button("Replace")
                                .on_hover_text("Replace current match")
                                .clicked()
                            {
                                actions.push(DesktopAction::ReplaceOne);
                            }
                            if ui
                                .small_button("All")
                                .on_hover_text("Replace all matches")
                                .clicked()
                            {
                                actions.push(DesktopAction::ReplaceAll);
                            }
                        });
                    }
                });
        });

    // Persist the text state back to egui transient data.
    ctx.data_mut(|d| d.insert_temp(state_id, text_state));
}

fn toast_accent_color(severity: StatusSeverity) -> egui::Color32 {
    match severity {
        StatusSeverity::Info => theme::tokens().accent.blue,
        StatusSeverity::Warning => theme::tokens().accent.orange,
        StatusSeverity::Error => theme::tokens().accent.red,
    }
}

fn render_excerpt_surface(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let surface = &snapshot.excerpt_surface_projection;
    if surface.sections.is_empty() {
        return;
    }

    section_label(ui, "Excerpts", Some(theme::tokens().accent.cyan));
    theme::small_card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::muted(format!("{} buffers", surface.sections.len())));
            if let Some(active_id) = surface.active_excerpt_id.as_deref() {
                ui.label(theme::accent(
                    format!("active {active_id}"),
                    theme::tokens().accent.green,
                ));
            }
        });
        for section in &surface.sections {
            ui.separator();
            let is_active =
                surface.active_excerpt_id.as_deref() == Some(section.excerpt_id.as_str());
            ui.horizontal(|ui| {
                let mut title = section.title.clone();
                if section.dirty {
                    title.push_str(" *");
                }
                let response = ui.add(
                    egui::Button::new(theme::code(title))
                        .fill(if is_active {
                            theme::tokens().bg.code
                        } else {
                            theme::tokens().bg.panel
                        })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if is_active {
                                theme::tokens().border.default
                            } else {
                                theme::tokens().bg.panel
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(5)),
                );
                if response.clicked()
                    && let Some(buffer_id) = section.buffer_id
                {
                    actions.push(DesktopAction::SwitchTab { buffer_id });
                }
                ui.label(theme::muted(format!(
                    "lines={} cursor={}",
                    section.lines.len(),
                    section
                        .cursor
                        .map(|cursor| format!("{}:{}", cursor.line + 1, cursor.character + 1))
                        .unwrap_or_else(|| "-".to_string())
                )));
            });
            for line in section.lines.iter().take(4) {
                let label = format!("{:>4}: {}", line.line_number + 1, line.visible_text);
                let response = ui.selectable_label(false, theme::code(label));
                if response.clicked()
                    && let Some(buffer_id) = section.buffer_id
                {
                    actions.push(DesktopAction::SetCursor {
                        buffer_id: Some(buffer_id),
                        cursor: TextCoordinate {
                            line: line.line_number,
                            character: 0,
                            byte_offset: None,
                            utf16_offset: None,
                        },
                    });
                    actions.push(DesktopAction::SwitchTab { buffer_id });
                }
            }
            if section.lines.len() > 4 {
                ui.label(theme::muted(format!(
                    "… {} more lines",
                    section.lines.len() - 4
                )));
            }
        }
    });
}

fn render_project_tree_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    egui::ScrollArea::vertical()
        .id_salt("legion_desktop_explorer_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_explorer_controls(ui, snapshot, state, actions);
        });
}

fn render_editor_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
    source: Option<&dyn DesktopLineSource>,
    streamed_cache: &mut StreamedLayoutCachePool,
) -> egui::Rect {
    render_tab_strip(ui, snapshot, actions);
    if ui.available_height() >= 250.0 {
        render_breadcrumb_bar(ui, snapshot);
    }
    if !model.large_file_banner_rows.is_empty() {
        theme::card_frame_tinted(theme::tokens().bg.card, theme::tokens().accent.orange).show(
            ui,
            |ui| {
                for row in &model.large_file_banner_rows {
                    ui.label(theme::code_muted(row));
                }
            },
        );
        ui.add_space(6.0);
    }
    let editor_rect = theme::code_frame()
        .show(ui, |ui| {
            let minimap_visible = model.settings.minimap_visible;
            let minimap_width = if minimap_visible { MINIMAP_WIDTH } else { 0.0 };
            let full_rect = ui.available_rect_before_wrap();

            // Code area: left portion, up to the minimap boundary.
            let code_rect = egui::Rect::from_min_max(
                full_rect.min,
                egui::pos2(full_rect.right() - minimap_width, full_rect.bottom()),
            );
            let mut code_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(code_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            egui::ScrollArea::both()
                .id_salt("legion_desktop_code_canvas_scroll")
                .auto_shrink([false, false])
                .show(&mut code_ui, |ui| {
                    let mut painter = EguiCodeCanvasPainter;
                    painter.paint_lines(ui, snapshot, model, actions, source, streamed_cache);
                });

            // Minimap: right column.
            if minimap_visible {
                let minimap_rect = egui::Rect::from_min_max(
                    egui::pos2(full_rect.right() - minimap_width, full_rect.top()),
                    full_rect.max,
                );
                let mut minimap_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(minimap_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                render_minimap(&mut minimap_ui, snapshot, model, actions);
            }

            // Advance the parent layout past the area used by both children.
            ui.allocate_rect(full_rect, egui::Sense::hover());
        })
        .response
        .rect;
    render_excerpt_surface(ui, snapshot, actions);
    editor_rect
}

const MINIMAP_WIDTH: f32 = 100.0;
const MINIMAP_ASSUMED_MAX_COLS: f32 = 80.0;

/// Render a scaled-down code minimap to the right of the code area.
///
/// For small files (where `small_buffer_preview` is available) each source line
/// is drawn as a narrow colored bar representing its approximate text width.
/// For large/degraded files the minimap shows an empty background with a muted
/// placeholder.  A semi-transparent viewport indicator tracks the currently
/// visible region.  Click or drag on the minimap scrolls the editor.
fn render_minimap(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    let active = &snapshot.active_buffer_projection;
    let viewport = active.viewport.as_ref();

    // Determine total line count and per-line text lengths.
    let (total_lines, line_lengths): (usize, Option<Vec<usize>>) =
        if let Some(preview) = active.small_buffer_text() {
            let lines: Vec<usize> = preview.lines().map(|l| l.len()).collect();
            let count = lines.len().max(1);
            (count, Some(lines))
        } else {
            // Large/degraded: estimate from last visible line.
            let estimated = viewport
                .map(|v| (v.visible_range.end.line as usize).saturating_add(50))
                .unwrap_or(100);
            (estimated, None)
        };

    let minimap_rect = ui.available_rect_before_wrap();
    let panel_height = minimap_rect.height();
    let panel_width = minimap_rect.width();
    if panel_height < 4.0 || panel_width < 4.0 {
        ui.allocate_rect(minimap_rect, egui::Sense::hover());
        return;
    }

    // Background fill.
    ui.painter().rect_filled(minimap_rect, 0.0, tokens.bg.code);

    // Left-edge separator line.
    ui.painter().line_segment(
        [minimap_rect.left_top(), minimap_rect.left_bottom()],
        egui::Stroke::new(1.0_f32, tokens.border.subtle),
    );

    // Keep the document-to-panel scale exact for navigation. Visual bars use
    // a minimum height separately; clamping this value would make the bottom
    // portion of large documents unreachable from the minimap.
    let px_per_line = panel_height / total_lines as f32;
    let bar_height = (px_per_line - 0.5).max(0.5);

    if let Some(lengths) = &line_lengths {
        // Small file: render a colored bar per line.
        let bar_color = theme::dim(tokens.text.muted, 76);
        for (i, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let y = minimap_rect.top() + i as f32 * px_per_line;
            if y >= minimap_rect.bottom() {
                break;
            }
            let bar_w = (len as f32 / MINIMAP_ASSUMED_MAX_COLS * (panel_width - 8.0))
                .clamp(2.0, panel_width - 8.0);
            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(minimap_rect.left() + 4.0, y),
                egui::vec2(bar_w, bar_height),
            );
            ui.painter().rect_filled(bar_rect, 0.0, bar_color);
        }
    } else {
        // Large/degraded file: centered placeholder.
        ui.painter().text(
            minimap_rect.center(),
            egui::Align2::CENTER_CENTER,
            "...",
            egui::FontId::monospace(10.0),
            tokens.text.disabled,
        );
    }

    // Viewport indicator rectangle.
    if let Some(vp) = viewport {
        let top_line = vp.scroll.top_line as f32;
        let visible_count = model.active_buffer_code_lines.len().max(1) as f32;

        let ind_top = (minimap_rect.top() + top_line * px_per_line).max(minimap_rect.top());
        let ind_bot = (minimap_rect.top() + (top_line + visible_count) * px_per_line)
            .min(minimap_rect.bottom());
        let indicator = egui::Rect::from_min_max(
            egui::pos2(minimap_rect.left() + 1.0, ind_top),
            egui::pos2(minimap_rect.right() - 1.0, ind_bot),
        );

        let fill = theme::dim(tokens.bg.hover, 153);
        ui.painter().rect_filled(indicator, 0.0, fill);
        ui.painter().rect_stroke(
            indicator,
            0.0,
            egui::Stroke::new(1.0_f32, tokens.border.strong),
            egui::epaint::StrokeKind::Inside,
        );
    }

    // Click / drag to scroll: compute target line from pointer position and
    // center the viewport around it.
    let response = ui.allocate_rect(minimap_rect, egui::Sense::click_and_drag());
    if (response.clicked() || response.dragged())
        && total_lines > 0
        && let Some(pos) = response.interact_pointer_pos()
    {
        let click_y = pos.y - minimap_rect.top();
        let clicked_line =
            ((click_y / px_per_line).floor() as usize).min(total_lines.saturating_sub(1)) as u32;
        let visible_count = model.active_buffer_code_lines.len() as u32;
        let target_top = clicked_line.saturating_sub(visible_count / 2);

        actions.push(DesktopAction::SetViewportScroll {
            buffer_id: active.buffer_id,
            scroll: ViewportScroll {
                top_line: target_top,
                left_column: viewport.map_or(0, |v| v.scroll.left_column),
            },
        });
    }
}

fn render_breadcrumb_bar(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let language = &snapshot.language_tooling_projection;
    theme::pane_frame(theme::tokens().bg.code).show(ui, |ui| {
        ui.set_height(26.0);
        ui.horizontal(|ui| {
            // Trailing segments, not the absolute path. Every file in the
            // workspace shares the same leading directories, so printing them
            // filled the bar with the one part that carries no information —
            // and on Windows it led with the `\\?\` prefix, which made every
            // path in the product look corrupted.
            let trail = crate::path_display::breadcrumb_trail(current_path(snapshot), 3);
            for (index, segment) in trail.iter().enumerate() {
                if index > 0 {
                    ui.label(theme::muted("›"));
                }
                if index + 1 == trail.len() {
                    ui.label(theme::code(segment));
                } else {
                    ui.label(theme::code_muted(segment));
                }
            }
            for breadcrumb in language.breadcrumbs.iter().take(4) {
                ui.label(theme::muted("›"));
                ui.label(theme::code_muted(&breadcrumb.label));
            }
        });
    });
}

fn render_code_lines(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
    source: Option<&dyn DesktopLineSource>,
    streamed_cache: &mut StreamedLayoutCachePool,
) {
    if snapshot.active_buffer_projection.buffer_id.is_none() {
        ui.label(theme::muted("<no active buffer>"));
        if snapshot.active_buffer_projection.workspace_id.is_some() {
            let blocked_save = ui.add_enabled(
                false,
                egui::Button::new("Save active file").min_size(egui::vec2(
                    128.0,
                    f32::from(theme::tokens().control_height.standard),
                )),
            );
            ui.ctx().accesskit_node_builder(blocked_save.id, |node| {
                node.set_description("Open a file to enable saving.");
            });
            ui.label(theme::muted("Open a file to enable saving."));
        }
        return;
    }
    if model.active_buffer_code_lines.is_empty() && model.active_buffer_lines.is_empty() {
        ui.label(theme::muted("<active buffer has no visible text>"));
        return;
    }
    if !model.active_buffer_code_lines.is_empty() {
        let active_buffer_id = snapshot.active_buffer_projection.buffer_id;
        let show_line_numbers = model.settings.line_numbers_visible;
        let highlight_current_line = model.settings.current_line_highlight;
        let current_cursor = snapshot
            .active_buffer_projection
            .viewport
            .as_ref()
            .map(|viewport| viewport.cursor)
            .unwrap_or_else(|| projected_cursor(snapshot));
        let current_line_number = current_cursor.line + 1;
        let char_width = code_char_width();
        let ime_composition =
            active_buffer_id.and_then(|buffer_id| ime_composition_state(ui, buffer_id));
        let active_git_relative_path = active_git_relative_path(snapshot);
        let git_hunks = &snapshot.git_projection.hunks;
        let git_blame_lines = &snapshot.git_projection.blame_lines;

        ui.horizontal(|ui| {
            if git_previous_hunk_cursor(
                active_git_relative_path.as_deref(),
                git_hunks,
                current_line_number,
            )
            .is_some()
                || git_next_hunk_cursor(
                    active_git_relative_path.as_deref(),
                    git_hunks,
                    current_line_number,
                )
                .is_some()
            {
                ui.label(theme::code_muted("git"));
                if let Some(prev_cursor) = git_previous_hunk_cursor(
                    active_git_relative_path.as_deref(),
                    git_hunks,
                    current_line_number,
                ) && soft_button(ui, "Prev hunk").clicked()
                {
                    actions.push(DesktopAction::SetCursor {
                        buffer_id: active_buffer_id,
                        cursor: prev_cursor,
                    });
                }
                if let Some(next_cursor) = git_next_hunk_cursor(
                    active_git_relative_path.as_deref(),
                    git_hunks,
                    current_line_number,
                ) && soft_button(ui, "Next hunk").clicked()
                {
                    actions.push(DesktopAction::SetCursor {
                        buffer_id: active_buffer_id,
                        cursor: next_cursor,
                    });
                }
            }
        });

        let viewport = snapshot.active_buffer_projection.viewport.as_ref();

        // The strip that used to live here printed the *state of the editor
        // settings* above the file — "sticky headers <none> folding 0 ranges
        // smooth scrolling" — on every buffer. That is a readout of a settings
        // struct, not a feature: a setting that is on should show up as the
        // thing it does (a sticky header pinned to the top, a fold arrow in the
        // gutter), and a setting that is off should show up as nothing at all.
        // Naming them in a row above the code told the user nothing and cost
        // the first line of every file.
        //
        // The active sticky scope is still projected and is still rendered
        // where it belongs; see `sticky_scopes` in the language-tooling panel.

        let mut source_budget = streamed_layout::MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = streamed_layout::MAX_FRAME_ROWS;
        let mut glyph_budget = streamed_layout::MAX_FRAME_GLYPHS;
        if let Some(source) = source {
            let active = model
                .active_buffer_code_lines
                .iter()
                .filter(|line| !matches!(line.truncation_state, ViewportLineTruncationState::None))
                .map(|line| source.identity(line.number.saturating_sub(1) as usize))
                .collect::<Vec<_>>();
            streamed_cache.begin_frame(&active);
            if !streamed_cache.streamed_navigation_requests().is_empty() {
                let (_, _, wrap_width) = active_code_line_wrap_config(
                    model.settings.line_wrapping_policy,
                    model.settings.wrap_column,
                    viewport.map(|viewport| (viewport.line_wrapping_policy, viewport.wrap_column)),
                    ui.available_width(),
                );
                streamed_cache.service_one_navigation(
                    ui,
                    source,
                    streamed_layout::StreamedLayoutOptions {
                        format: egui::text::TextFormat {
                            font_id: egui::FontId::monospace(
                                model.settings.editor_font_size_pt as f32,
                            ),
                            color: theme::tokens().text.secondary,
                            ..Default::default()
                        },
                        pixels_per_point: ui.ctx().pixels_per_point(),
                        wrap_width,
                        break_anywhere: false,
                        visible_rows: 0..streamed_layout::MAX_FRAME_ROWS,
                        visible_bytes: 0..streamed_layout::MAX_FRAME_GLYPHS as u64,
                    },
                    streamed_layout::StreamedFrameBudget {
                        source_bytes: &mut source_budget,
                        rows: &mut row_budget,
                        glyphs: &mut glyph_budget,
                    },
                );
            }
        }
        for line in &model.active_buffer_code_lines {
            ui.horizontal(|ui| {
                let git_marker = git_hunk_marker_for_line(
                    active_git_relative_path.as_deref(),
                    git_hunks,
                    line.number,
                )
                .unwrap_or(" ");
                ui.add_sized(
                    [12.0, 18.0],
                    egui::Label::new(theme::code_muted(git_marker)),
                );
                if show_line_numbers {
                    let line_number = format!("{:>3}", line.number);
                    ui.add_sized(
                        [42.0, 18.0],
                        egui::Label::new(if line.number == current_line_number {
                            theme::accent(line_number, theme::tokens().code_canvas.cursor)
                        } else {
                            theme::accent(line_number, theme::tokens().code_canvas.line_number)
                        }),
                    );
                }
                ui.add_sized(
                    [16.0, 18.0],
                    egui::Label::new(theme::code_muted(code_line_truncation_marker(
                        line.truncation_state,
                    ))),
                );
                let snapshot_id = snapshot
                    .active_buffer_projection
                    .viewport
                    .as_ref()
                    .map(|viewport| viewport.snapshot_id);
                let (line_wrapping_policy, wrap_column, wrap_width) = active_code_line_wrap_config(
                    model.settings.line_wrapping_policy,
                    model.settings.wrap_column,
                    viewport.map(|viewport| (viewport.line_wrapping_policy, viewport.wrap_column)),
                    ui.available_width(),
                );
                if let Some(buffer_id) = active_buffer_id {
                    record_visual_navigation_paint_geometry(
                        ui,
                        VisualNavigationPaintGeometry {
                            buffer_id,
                            wrap_width,
                            line_wrapping_policy,
                            wrap_column,
                            paint_pass: ui.ctx().cumulative_pass_nr(),
                            pixels_per_point: ui.ctx().pixels_per_point(),
                            font_size_bucket: code_line_font_size_bucket(),
                        },
                    );
                }
                if let Some(source) = source
                    && !matches!(line.truncation_state, ViewportLineTruncationState::None)
                {
                    let format = egui::text::TextFormat {
                        font_id: egui::FontId::monospace(model.settings.editor_font_size_pt as f32),
                        color: theme::tokens().text.secondary,
                        ..Default::default()
                    };
                    if let Ok(result) = streamed_cache.paint_visible(
                        ui,
                        source,
                        line.number.saturating_sub(1) as usize,
                        streamed_layout::StreamedLayoutOptions {
                            format,
                            pixels_per_point: ui.ctx().pixels_per_point(),
                            wrap_width,
                            break_anywhere: false,
                            visible_rows: 0..streamed_layout::MAX_FRAME_ROWS,
                            visible_bytes: line.byte_range.start..line.byte_range.end,
                        },
                        streamed_layout::StreamedFrameBudget {
                            source_bytes: &mut source_budget,
                            rows: &mut row_budget,
                            glyphs: &mut glyph_budget,
                        },
                    ) {
                        let height = result
                            .rows
                            .iter()
                            .map(|row| row.rect.height())
                            .sum::<f32>()
                            .max(18.0);
                        let response = ui.allocate_response(
                            egui::vec2(ui.available_width(), height),
                            egui::Sense::click_and_drag(),
                        );
                        if let Some(pointer) = response.interact_pointer_pos()
                            && let Some((absolute_byte, affinity)) = result
                                .rows
                                .iter()
                                .filter(|row| {
                                    let y = pointer.y - response.rect.min.y;
                                    row.rect.y_range().contains(y)
                                })
                                .flat_map(|row| {
                                    row.stops.iter().filter_map(move |(byte, x)| {
                                        row.source_byte_range.contains(byte).then_some((
                                            *byte,
                                            *x,
                                            (pointer.x - response.rect.min.x - *x).abs(),
                                        ))
                                    })
                                })
                                .min_by(|left, right| left.2.total_cmp(&right.2))
                                .map(|(byte, x, _)| {
                                    (
                                        byte,
                                        if pointer.x - response.rect.min.x >= x {
                                            CaretAffinity::Downstream
                                        } else {
                                            CaretAffinity::Upstream
                                        },
                                    )
                                })
                            && let Some(buffer_id) = active_buffer_id
                            && let Some(viewport) = viewport
                        {
                            let origin =
                                line.line_start_byte_offset.unwrap_or(line.byte_range.start);
                            let coordinate = TextCoordinate {
                                line: line.number.saturating_sub(1),
                                character: absolute_byte.saturating_sub(origin) as u32,
                                byte_offset: Some(absolute_byte),
                                utf16_offset: None,
                            };
                            if response.clicked() {
                                actions.push(DesktopAction::SetVisualCursor {
                                    buffer_id: Some(buffer_id),
                                    expected_snapshot_id: viewport.snapshot_id,
                                    expected_buffer_version: viewport.buffer_version,
                                    cursor: coordinate,
                                    affinity,
                                });
                            }
                            if response.dragged() || response.drag_stopped() {
                                actions.push(DesktopAction::SetVisualDirectedSelection {
                                    buffer_id: Some(buffer_id),
                                    expected_snapshot_id: viewport.snapshot_id,
                                    expected_buffer_version: viewport.buffer_version,
                                    anchor: current_cursor,
                                    head: coordinate,
                                    head_affinity: affinity,
                                });
                            }
                        }
                        for row in result.rows {
                            let mut mesh = row.mesh;
                            mesh.translate(response.rect.min.to_vec2());
                            ui.painter().add(egui::Shape::mesh(mesh));
                        }
                        if result.needs_repaint {
                            ui.ctx().request_repaint();
                        }
                        let _ = response;
                        return;
                    }
                }
                let galley =
                    cached_code_line_galley(ui, active_buffer_id, snapshot_id, line, wrap_width);
                let response =
                    ui.add(egui::Label::new(galley.clone()).sense(egui::Sense::click_and_drag()));
                if let Some(position) = response.interact_pointer_pos()
                    && let Some(buffer_id) = active_buffer_id
                {
                    let Some((coordinate, affinity)) = editor_visual_coordinate_from_galley_pointer(
                        line,
                        galley.as_ref(),
                        position,
                        response.rect.min,
                    ) else {
                        return;
                    };
                    let drag_anchor_id = code_drag_anchor_id(buffer_id);
                    if response.drag_started() {
                        let drag_delta = response
                            .total_drag_delta()
                            .unwrap_or_else(|| response.drag_delta());
                        let Some((anchor, _anchor_affinity)) =
                            drag_anchor_for_line_pointer_with_galley_visual(
                                line,
                                galley.as_ref(),
                                position.x,
                                drag_delta,
                                position.y,
                                response.rect.min,
                            )
                        else {
                            return;
                        };
                        response
                            .ctx
                            .data_mut(|data| data.insert_temp(drag_anchor_id, anchor));
                    }
                    if response.triple_clicked() {
                        if let Some(range) = line_range_for_code_line(line) {
                            actions.push(DesktopAction::SetSelection {
                                buffer_id: Some(buffer_id),
                                range,
                            });
                        }
                    } else if response.double_clicked() {
                        if let Some(range) = word_range_for_coordinate(line, coordinate) {
                            actions.push(DesktopAction::SetSelection {
                                buffer_id: Some(buffer_id),
                                range,
                            });
                        }
                    } else if response.clicked() && ui.input(|i| i.modifiers.ctrl) {
                        actions.push(DesktopAction::GoToDefinition {
                            position: coordinate,
                        });
                    } else if response.clicked()
                        && let Some(viewport) = viewport
                    {
                        actions.push(DesktopAction::SetVisualCursor {
                            buffer_id: Some(buffer_id),
                            expected_snapshot_id: viewport.snapshot_id,
                            expected_buffer_version: viewport.buffer_version,
                            cursor: coordinate,
                            affinity,
                        });
                    }
                    if (response.drag_started() || response.dragged())
                        && let Some(viewport) = viewport
                    {
                        let anchor = response
                            .ctx
                            .data_mut(|data| data.get_temp::<TextCoordinate>(drag_anchor_id));
                        actions.push(DesktopAction::SetVisualDirectedSelection {
                            buffer_id: Some(buffer_id),
                            expected_snapshot_id: viewport.snapshot_id,
                            expected_buffer_version: viewport.buffer_version,
                            anchor: anchor.unwrap_or(current_cursor),
                            head: coordinate,
                            head_affinity: affinity,
                        });
                    }
                    if response.drag_stopped() {
                        response
                            .ctx
                            .data_mut(|data| data.remove::<TextCoordinate>(drag_anchor_id));
                    }
                }
                if highlight_current_line {
                    paint_current_line_highlight(ui, line, &response, current_cursor);
                }
                if let Some(viewport) = viewport {
                    paint_code_selections(
                        ui,
                        line,
                        &response,
                        &viewport.selections,
                        galley.as_ref(),
                    );
                }
                // Every cursor, not only the primary. The projection has
                // carried the full set all along; painting one made a
                // multi-cursor edit look like it came from nowhere.
                match viewport {
                    Some(viewport) if viewport.cursors.len() > 1 => {
                        for (index, cursor) in viewport.cursors.iter().enumerate() {
                            let affinity = viewport
                                .cursor_affinities
                                .get(index)
                                .copied()
                                .unwrap_or(CaretAffinity::Upstream);
                            paint_code_cursor(
                                ui,
                                line,
                                &response,
                                *cursor,
                                affinity,
                                index == 0,
                                galley.as_ref(),
                            );
                        }
                    }
                    _ => paint_code_cursor(
                        ui,
                        line,
                        &response,
                        current_cursor,
                        viewport
                            .and_then(|projection| projection.cursor_affinities.first().copied())
                            .unwrap_or(CaretAffinity::Upstream),
                        true,
                        galley.as_ref(),
                    ),
                }
                paint_find_match_highlights(
                    ui,
                    line,
                    &response,
                    &snapshot.find_bar_projection,
                    char_width,
                );
                paint_diagnostic_underlines(
                    ui,
                    line,
                    &response,
                    &snapshot.language_tooling_projection.problems,
                    char_width,
                );
                show_diagnostic_tooltip(
                    ui,
                    &response,
                    line,
                    &snapshot.language_tooling_projection.problems,
                    char_width,
                );
                paint_inlay_hints(
                    ui,
                    line,
                    &response,
                    &snapshot.language_tooling_projection.inlay_hints,
                    char_width,
                );
                // Ctrl+hover: underline word as a go-to-definition affordance.
                if response.hovered()
                    && ui.input(|i| i.modifiers.ctrl)
                    && let Some(hover_pos) = response.hover_pos()
                {
                    let Some(hover_coord) = editor_coordinate_from_galley_pointer(
                        line,
                        galley.as_ref(),
                        hover_pos,
                        response.rect.min,
                    ) else {
                        return;
                    };
                    if let Some(range) = word_range_for_coordinate(line, hover_coord) {
                        let start_x =
                            response.rect.left() + range.start.character as f32 * char_width;
                        let end_x = response.rect.left() + range.end.character as f32 * char_width;
                        let y = response.rect.bottom() - 1.0;
                        ui.painter().line_segment(
                            [egui::pos2(start_x, y), egui::pos2(end_x, y)],
                            egui::Stroke::new(1.0_f32, theme::tokens().chrome.breadcrumb_accent),
                        );
                    }
                }
                // Hover request: dispatch RequestHover when the hover position changes.
                if response.hovered()
                    && !ui.input(|i| i.modifiers.ctrl)
                    && let Some(hover_pos) = response.hover_pos()
                {
                    let Some(hover_coord) = editor_coordinate_from_galley_pointer(
                        line,
                        galley.as_ref(),
                        hover_pos,
                        response.rect.min,
                    ) else {
                        return;
                    };
                    let hover_pos_id = egui::Id::new("lsp_last_hover_pos");
                    let last_pos: Option<(u32, u32)> =
                        ui.ctx().data_mut(|d| d.get_temp(hover_pos_id));
                    let current_pos = (hover_coord.line, hover_coord.character);
                    if last_pos != Some(current_pos) {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(hover_pos_id, current_pos));
                        actions.push(DesktopAction::RequestHover {
                            position: hover_coord,
                        });
                    }
                }
                if let Some(ime_composition) = ime_composition.as_ref() {
                    paint_ime_composition(
                        ui,
                        line,
                        &response,
                        current_cursor,
                        viewport
                            .and_then(|projection| projection.cursor_affinities.first().copied())
                            .unwrap_or(CaretAffinity::Upstream),
                        galley.as_ref(),
                        ime_composition,
                    );
                }
                if line.number == current_line_number
                    && let Some(label) = git_inline_blame_label(
                        active_git_relative_path.as_deref(),
                        git_blame_lines,
                        line.number,
                    )
                {
                    ui.add_space(8.0);
                    ui.label(theme::code_muted(trim_middle(&label, 72)));
                }
            });
        }

        // Multiple definitions stay in the picker so the user can choose the
        // destination.  A single definition is navigated by
        // DesktopRuntime::refresh_projection after the queued request returns;
        // keeping that side effect out of rendering prevents a persistent
        // projection from re-enqueuing navigation on every frame.
        let definitions = &snapshot.language_tooling_projection.definitions;
        if definitions.len() > 1 {
            render_definition_picker(ui, definitions, actions);
        }
        if !snapshot.language_tooling_projection.references.is_empty() {
            render_reference_panel(
                ui,
                &snapshot.language_tooling_projection.references,
                actions,
            );
        }
        render_code_action_panel(
            ui,
            &snapshot.language_tooling_projection.code_action_candidates,
            actions,
        );

        return;
    }
    let show_line_numbers = model.settings.line_numbers_visible;
    for (index, row) in model.active_buffer_lines.iter().enumerate() {
        ui.horizontal(|ui| {
            if show_line_numbers {
                ui.add_sized(
                    [42.0, 18.0],
                    egui::Label::new(theme::code_muted(format!("{:>3}", index + 1))),
                );
            }
            ui.label(theme::code(row));
        });
    }
}

fn code_char_width() -> f32 {
    theme::tokens().typography.code as f32 * 0.62
}

fn code_line_wrap_width_for_policy(
    policy: LineWrappingPolicy,
    wrap_column: Option<u32>,
    available_width: f32,
) -> f32 {
    match policy {
        LineWrappingPolicy::Off => f32::INFINITY,
        LineWrappingPolicy::Viewport => available_width.max(1.0),
        LineWrappingPolicy::FixedColumn => {
            let column = wrap_column.unwrap_or(120).max(1);
            column as f32 * code_char_width()
        }
    }
}

fn active_code_line_wrap_config(
    settings_policy: LineWrappingPolicy,
    settings_wrap_column: Option<u32>,
    viewport: Option<(LineWrappingPolicy, Option<u32>)>,
    available_width: f32,
) -> (LineWrappingPolicy, Option<u32>, f32) {
    let (policy, wrap_column) = viewport.unwrap_or((settings_policy, settings_wrap_column));
    let width = code_line_wrap_width_for_policy(policy, wrap_column, available_width);
    (policy, wrap_column, width)
}

/// Return the same wrap width used by the code canvas for visual navigation.
pub fn visual_navigation_wrap_width(
    viewport: &legion_protocol::ViewportProjection,
    available_width: f32,
    _line_numbers_visible: bool,
    _item_spacing: f32,
) -> f32 {
    match viewport.line_wrapping_policy {
        LineWrappingPolicy::Off => f32::INFINITY,
        LineWrappingPolicy::Viewport => {
            // `available_width` is the width remaining after the row gutters
            // have been laid out. Do not reconstruct gutter dimensions here:
            // those values are theme- and renderer-dependent.
            available_width.max(1.0)
        }
        LineWrappingPolicy::FixedColumn => {
            viewport.wrap_column.unwrap_or(120).max(1) as f32 * code_char_width()
        }
    }
}

/// The layout facts captured while the code row is actually being painted.
///
/// Input handlers must use these facts instead of reconstructing the canvas
/// width from the outer panel (which has already consumed the gutters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualNavigationPaintGeometry {
    /// Buffer whose code row established this geometry.
    pub buffer_id: legion_protocol::BufferId,
    /// Measured galley wrap width after row gutters were laid out.
    pub wrap_width: f32,
    /// Wrapping policy used for the painted galley.
    pub line_wrapping_policy: LineWrappingPolicy,
    /// Fixed-column value used when applicable.
    pub wrap_column: Option<u32>,
    /// Egui cumulative pass in which the row was painted.
    pub paint_pass: u64,
    /// Device scale used for the painted layout.
    pub pixels_per_point: f32,
    /// Font-size bucket used by the galley cache.
    pub font_size_bucket: u32,
}

/// Read paint geometry for a buffer when it belongs to the current egui pass
/// and font scale. A missing or stale value means callers should request a
/// fresh paint before shaping input geometry.
pub fn visual_navigation_paint_geometry(
    ui: &egui::Ui,
    buffer_id: legion_protocol::BufferId,
) -> Option<VisualNavigationPaintGeometry> {
    let geometry = ui.ctx().data(|data| {
        data.get_temp::<VisualNavigationPaintGeometry>(egui::Id::new((
            "legion_desktop_visual_navigation_paint_geometry",
            buffer_id.0,
        )))
    });
    let geometry = geometry?;
    (geometry.paint_pass == ui.ctx().cumulative_pass_nr()
        && geometry.pixels_per_point.to_bits() == ui.ctx().pixels_per_point().to_bits()
        && geometry.font_size_bucket == code_line_font_size_bucket())
    .then_some(geometry)
}

fn record_visual_navigation_paint_geometry(ui: &egui::Ui, geometry: VisualNavigationPaintGeometry) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::new((
                "legion_desktop_visual_navigation_paint_geometry",
                geometry.buffer_id.0,
            )),
            geometry,
        );
    });
}

const CODE_LINE_GALLEY_CACHE_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CodeLineGalleyCacheKey {
    buffer_id: u128,
    snapshot_id: u128,
    content_fingerprint: u64,
    font_size_bucket: u32,
    width_bucket: u32,
}

#[derive(Clone)]
struct RenderPassCache<K, V> {
    pass_nr: Option<u64>,
    entries: HashMap<K, V>,
}

impl<K, V> Default for RenderPassCache<K, V> {
    fn default() -> Self {
        Self {
            pass_nr: None,
            entries: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash, V> RenderPassCache<K, V> {
    fn prepare_for_pass(&mut self, pass_nr: u64) {
        if self.pass_nr != Some(pass_nr) {
            self.pass_nr = Some(pass_nr);
            self.entries.clear();
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key)
    }

    fn insert_bounded(&mut self, key: K, value: V, limit: usize) {
        if limit == 0 {
            self.entries.clear();
            return;
        }
        if !self.entries.contains_key(&key) && self.entries.len() >= limit {
            self.entries.clear();
        }
        self.entries.insert(key, value);
    }
}

type CodeLineGalleyCache = RenderPassCache<CodeLineGalleyCacheKey, Arc<egui::Galley>>;

fn cached_code_line_galley(
    ui: &egui::Ui,
    buffer_id: Option<legion_protocol::BufferId>,
    snapshot_id: Option<legion_protocol::SnapshotId>,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
) -> Arc<egui::Galley> {
    let Some(buffer_id) = buffer_id else {
        return shape_code_line_galley(ui, line, wrap_width);
    };
    let cache_id = code_line_galley_cache_id(buffer_id);
    let key = code_line_galley_cache_key(Some(buffer_id), snapshot_id, line, wrap_width);
    let pass_nr = ui.ctx().cumulative_pass_nr();

    if let Some(cached_galley) = ui.ctx().data_mut(|data| {
        let mut cache = data
            .get_temp::<CodeLineGalleyCache>(cache_id)
            .unwrap_or_default();
        cache.prepare_for_pass(pass_nr);
        let cached_galley = cache.get(&key).cloned();
        data.insert_temp(cache_id, cache);
        cached_galley
    }) {
        return cached_galley;
    }

    let galley = shape_code_line_galley(ui, line, wrap_width);
    ui.ctx().data_mut(|data| {
        let mut cache = data
            .get_temp::<CodeLineGalleyCache>(cache_id)
            .unwrap_or_default();
        cache.prepare_for_pass(pass_nr);
        cache.insert_bounded(key, Arc::clone(&galley), CODE_LINE_GALLEY_CACHE_LIMIT);
        data.insert_temp(cache_id, cache);
    });
    galley
}

fn shape_code_line_galley(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
) -> Arc<egui::Galley> {
    let wrap_mode = if wrap_width.is_finite() {
        Some(egui::TextWrapMode::Wrap)
    } else {
        Some(egui::TextWrapMode::Extend)
    };
    egui::WidgetText::from(code_line_layout_job(line)).into_galley(
        ui,
        wrap_mode,
        wrap_width,
        egui::FontSelection::Default,
    )
}

/// Failure returned when a bounded renderer fragment cannot provide visual rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualNavigationGeometryError {
    /// The wrap width is NaN, negative infinity, or a nonpositive finite value.
    NonFiniteWrapWidth,
    /// The fragment does not carry an absolute logical byte origin.
    MissingLineOrigin,
    /// A requested byte column is not a UTF-8 boundary in the fragment.
    InvalidBoundary,
    /// The requested position is outside this bounded fragment and needs a wider window.
    NeedMoreWindow,
    /// The caller's stop budget is too small for the selected rows.
    StopBudgetExceeded,
    /// The supplied fragment exceeds the renderer's bounded shaping budget.
    FragmentTooLarge,
    /// Focused visual navigation requires a complete logical line.
    PartialFragment,
    /// A target preferred X must be finite and nonnegative.
    InvalidPreferredX,
}

/// Convert one bounded code-line galley into protocol visual-row facts.
///
/// `valid_byte_columns` is supplied as logical-line-relative columns by the editor/app window request. The renderer
/// does not infer grapheme boundaries or materialize an entire logical line: it only
/// emits stops for those validated logical byte columns that occur on selected rows.
pub fn visual_navigation_rows_for_line(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
    valid_byte_columns: &[u64],
    stop_budget: usize,
) -> Result<Vec<VisualNavigationRow>, VisualNavigationGeometryError> {
    visual_navigation_rows_for_line_with_focus(
        ui,
        line,
        wrap_width,
        valid_byte_columns,
        stop_budget,
        None,
    )
}

fn visual_navigation_rows_for_line_with_focus(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
    valid_byte_columns: &[u64],
    stop_budget: usize,
    focus_byte_column: Option<u64>,
) -> Result<Vec<VisualNavigationRow>, VisualNavigationGeometryError> {
    if wrap_width.is_nan()
        || wrap_width == f32::NEG_INFINITY
        || (wrap_width.is_finite() && wrap_width <= 0.0)
    {
        return Err(VisualNavigationGeometryError::NonFiniteWrapWidth);
    }
    let origin = line
        .line_start_byte_offset
        .ok_or(VisualNavigationGeometryError::MissingLineOrigin)?;
    let logical_base = line
        .byte_range
        .start
        .checked_sub(origin)
        .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
    const MAX_REQUESTED_STOPS: usize = 4_096;
    if valid_byte_columns.len() > MAX_REQUESTED_STOPS {
        return Err(VisualNavigationGeometryError::StopBudgetExceeded);
    }
    const MAX_FRAGMENT_BYTES: usize = 96 * 1024;
    if line.text.len() > MAX_FRAGMENT_BYTES {
        return Err(VisualNavigationGeometryError::FragmentTooLarge);
    }
    let galley = shape_code_line_galley(ui, line, wrap_width);
    let logical_line = line.number.saturating_sub(1);
    let complete_fragment = matches!(line.truncation_state, ViewportLineTruncationState::None);
    let mut requested = valid_byte_columns.to_vec();
    requested.sort_unstable();
    requested.dedup();
    if focus_byte_column.is_none() && requested.len() > stop_budget {
        return Err(VisualNavigationGeometryError::StopBudgetExceeded);
    }
    let mut scalar_start = 0usize;
    let mut rows = Vec::new();
    let mut remaining_budget = stop_budget;
    let mut emitted = vec![false; requested.len()];

    for (row_index, row) in galley.rows.iter().enumerate() {
        let scalar_end = scalar_start + row.char_count_excluding_newline();
        let start_byte = absolute_byte_at_scalar(line, scalar_start)
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        let end_byte = absolute_byte_at_scalar(line, scalar_end)
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        let start_column = logical_base
            .checked_add(start_byte)
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        let end_column = logical_base
            .checked_add(end_byte)
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        if requested.last().is_some_and(|&last| start_column > last) {
            break;
        }
        let mut stops = Vec::new();
        let first = requested.partition_point(|&column| column < start_column);
        let last = requested.partition_point(|&column| column <= end_column);
        let row_requested = &requested[first..last];
        // Keep one stop available for every later shaped row. This preserves
        // the actual adjacent target row when a complete line has many
        // boundaries, while the total emitted stops still obey the editor's
        // 4096-stop validation bound.
        let rows_remaining = galley.rows.len().saturating_sub(row_index + 1);
        let row_budget = remaining_budget.saturating_sub(rows_remaining).max(1);
        let selected = bounded_row_boundary_indices(row_requested, row_budget, focus_byte_column);
        for offset in selected {
            let byte_column = row_requested[offset];
            let request_index = first + offset;
            let local_byte = byte_column
                .checked_sub(logical_base)
                .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
            let Some(scalar) = scalar_for_local_byte(line, local_byte) else {
                return Err(VisualNavigationGeometryError::InvalidBoundary);
            };
            if scalar < scalar_start || scalar > scalar_end {
                continue;
            }
            if remaining_budget == 0 {
                return Err(VisualNavigationGeometryError::StopBudgetExceeded);
            }
            remaining_budget -= 1;
            emitted[request_index] = true;
            let affinity = if scalar == scalar_start && row_index > 0 {
                legion_protocol::CaretAffinity::Downstream
            } else {
                legion_protocol::CaretAffinity::Upstream
            };
            let x = galley
                .pos_from_cursor(egui::text::CCursor {
                    index: scalar,
                    prefer_next_row: affinity == legion_protocol::CaretAffinity::Downstream,
                })
                .min
                .x;
            stops.push(VisualNavigationStop {
                position: VisualNavigationPosition {
                    line: logical_line,
                    byte_column,
                },
                x: VisualNavigationX { value: x },
                affinity,
            });
        }
        if !stops.is_empty() {
            rows.push(VisualNavigationRow {
                logical_line,
                start: VisualNavigationPosition {
                    line: logical_line,
                    byte_column: start_column,
                },
                end: VisualNavigationPosition {
                    line: logical_line,
                    byte_column: end_column,
                },
                row_index: complete_fragment.then_some(row_index as u32),
                row_count: complete_fragment.then_some(galley.rows.len() as u32),
                stops,
            });
        }
        scalar_start = scalar_end;
    }
    if rows.is_empty() || emitted.iter().any(|&seen| !seen) {
        return Err(VisualNavigationGeometryError::NeedMoreWindow);
    }
    Ok(rows)
}

/// Select at most the renderer's stop budget while retaining the source or
/// preferred-X neighborhood. Complete row spans remain exact even when a
/// logical line has more than 4096 grapheme boundaries.
fn bounded_row_boundary_indices(
    boundaries: &[u64],
    budget: usize,
    focus: Option<u64>,
) -> Vec<usize> {
    if boundaries.len() <= budget {
        return (0..boundaries.len()).collect();
    }
    if budget == 0 {
        return Vec::new();
    }
    let pivot = focus
        .map(|value| boundaries.partition_point(|&boundary| boundary < value))
        .unwrap_or(boundaries.len() / 2)
        .min(boundaries.len().saturating_sub(1));
    let start = pivot
        .saturating_sub(budget / 2)
        .min(boundaries.len().saturating_sub(budget));
    (start..start + budget).collect()
}

/// Build one source-row fact and its preferred rendered X from a bounded galley.
pub fn visual_navigation_source_row_for_line(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
    source_byte_column: u64,
    source_affinity: legion_protocol::CaretAffinity,
    valid_byte_columns: &[u64],
    stop_budget: usize,
) -> Result<VisualNavigationSourceRow, VisualNavigationGeometryError> {
    let rows = visual_navigation_rows_for_line_with_focus(
        ui,
        line,
        wrap_width,
        valid_byte_columns,
        stop_budget,
        Some(source_byte_column),
    )?;
    for row in rows {
        let source_x = row
            .stops
            .iter()
            .find(|stop| {
                stop.position.byte_column == source_byte_column && stop.affinity == source_affinity
            })
            .map(|stop| stop.x);
        if let Some(source_x) = source_x {
            return Ok(VisualNavigationSourceRow { row, source_x });
        }
    }
    Err(VisualNavigationGeometryError::NeedMoreWindow)
}

/// Select exactly one shaped row without materializing every stop in a long
/// complete line. The supplied boundaries remain the sole source of valid
/// caret positions; the renderer never reconstructs grapheme boundaries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualNavigationRowRequest {
    /// Select the row containing this byte boundary and affinity.
    Source {
        /// Logical-line-relative UTF-8 byte column.
        byte_column: u64,
        /// Wrap-side affinity at the source boundary.
        affinity: legion_protocol::CaretAffinity,
    },
    /// Select a shaped visual row and nearest supplied stop to this X.
    Target {
        /// Zero-based visual row index.
        row_index: u32,
        /// Preferred row-local rendered X.
        preferred_x: f32,
    },
}

/// Result of focused row shaping. `source_x` is populated for a source
/// request and `target_stop` for a target request. The row contains at most
/// three stops: the selected stop and any supplied true row endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualNavigationSelectedRow {
    /// Exact shaped row span and bounded stops.
    pub row: VisualNavigationRow,
    /// Source stop X, when the request selected a source row.
    pub source_x: Option<VisualNavigationX>,
    /// Nearest target stop, when the request selected a target row.
    pub target_stop: Option<VisualNavigationStop>,
}

/// Shape one source or target row from a complete logical-line fragment.
///
/// Unlike `visual_navigation_rows_for_line`, this focused API accepts more
/// than 4096 supplied boundaries, validates the complete input up front, and
/// emits only the requested row's source/nearest-X stop plus valid endpoints.
pub fn visual_navigation_selected_row_for_line(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
    valid_byte_columns: &[u64],
    request: VisualNavigationRowRequest,
) -> Result<VisualNavigationSelectedRow, VisualNavigationGeometryError> {
    if wrap_width.is_nan()
        || wrap_width == f32::NEG_INFINITY
        || (wrap_width.is_finite() && wrap_width <= 0.0)
    {
        return Err(VisualNavigationGeometryError::NonFiniteWrapWidth);
    }
    let origin = line
        .line_start_byte_offset
        .ok_or(VisualNavigationGeometryError::MissingLineOrigin)?;
    let logical_base = line
        .byte_range
        .start
        .checked_sub(origin)
        .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
    const MAX_FRAGMENT_BYTES: usize = 96 * 1024;
    if line.text.len() > MAX_FRAGMENT_BYTES {
        return Err(VisualNavigationGeometryError::FragmentTooLarge);
    }
    if !matches!(line.truncation_state, ViewportLineTruncationState::None) {
        return Err(VisualNavigationGeometryError::PartialFragment);
    }
    if valid_byte_columns.len() > MAX_FRAGMENT_BYTES + 1 {
        return Err(VisualNavigationGeometryError::StopBudgetExceeded);
    }
    if let VisualNavigationRowRequest::Target { preferred_x, .. } = request
        && (!preferred_x.is_finite() || preferred_x < 0.0)
    {
        return Err(VisualNavigationGeometryError::InvalidPreferredX);
    }
    let mut byte_to_scalar = HashMap::with_capacity(line.text.chars().count() + 1);
    for (scalar, (byte, _)) in line.text.char_indices().enumerate() {
        byte_to_scalar.insert(byte as u64, scalar);
    }
    byte_to_scalar.insert(line.text.len() as u64, line.text.chars().count());
    let mut requested = valid_byte_columns.to_vec();
    requested.sort_unstable();
    requested.dedup();
    // Validate every supplied boundary before choosing a row. This prevents
    // a focused request from silently hiding a missing or invalid window edge.
    for &column in &requested {
        let local = column
            .checked_sub(logical_base)
            .ok_or(VisualNavigationGeometryError::NeedMoreWindow)?;
        if !byte_to_scalar.contains_key(&local) {
            return Err(VisualNavigationGeometryError::NeedMoreWindow);
        }
    }
    let galley = shape_code_line_galley(ui, line, wrap_width);
    let logical_line = line.number.saturating_sub(1);
    let complete_fragment = matches!(line.truncation_state, ViewportLineTruncationState::None);
    let wanted_row = match request {
        VisualNavigationRowRequest::Source { .. } => None,
        VisualNavigationRowRequest::Target { row_index, .. } => Some(row_index),
    };
    let mut scalar_start = 0usize;
    for (row_index, shaped_row) in galley.rows.iter().enumerate() {
        let scalar_end = scalar_start + shaped_row.char_count_excluding_newline();
        let start_column = logical_base
            .checked_add(
                absolute_byte_at_scalar(line, scalar_start)
                    .ok_or(VisualNavigationGeometryError::InvalidBoundary)?,
            )
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        let end_column = logical_base
            .checked_add(
                absolute_byte_at_scalar(line, scalar_end)
                    .ok_or(VisualNavigationGeometryError::InvalidBoundary)?,
            )
            .ok_or(VisualNavigationGeometryError::InvalidBoundary)?;
        let source_match = match request {
            VisualNavigationRowRequest::Source {
                byte_column,
                affinity,
            } => {
                requested.binary_search(&byte_column).is_ok()
                    && byte_to_scalar
                        .get(
                            &byte_column
                                .checked_sub(logical_base)
                                .ok_or(VisualNavigationGeometryError::NeedMoreWindow)?,
                        )
                        .is_some_and(|&scalar| {
                            scalar >= scalar_start
                                && scalar <= scalar_end
                                && (affinity == legion_protocol::CaretAffinity::Downstream
                                    || row_index == 0
                                    || scalar != scalar_start)
                        })
            }
            VisualNavigationRowRequest::Target { .. } => false,
        };
        if matches!(request, VisualNavigationRowRequest::Source { .. }) && !source_match {
            scalar_start = scalar_end;
            continue;
        }
        if wanted_row.is_some_and(|wanted| wanted != row_index as u32) {
            scalar_start = scalar_end;
            continue;
        }
        let first = requested.partition_point(|&column| column < start_column);
        let last = requested.partition_point(|&column| column <= end_column);
        let mut candidates = Vec::with_capacity(last.saturating_sub(first));
        for &byte_column in &requested[first..last] {
            let scalar = *byte_to_scalar
                .get(
                    &byte_column
                        .checked_sub(logical_base)
                        .ok_or(VisualNavigationGeometryError::NeedMoreWindow)?,
                )
                .ok_or(VisualNavigationGeometryError::NeedMoreWindow)?;
            if scalar < scalar_start || scalar > scalar_end {
                continue;
            }
            let affinity = if scalar == scalar_start && row_index > 0 {
                legion_protocol::CaretAffinity::Downstream
            } else {
                legion_protocol::CaretAffinity::Upstream
            };
            let x = galley
                .pos_from_cursor(egui::text::CCursor {
                    index: scalar,
                    prefer_next_row: affinity == legion_protocol::CaretAffinity::Downstream,
                })
                .min
                .x;
            candidates.push(VisualNavigationStop {
                position: VisualNavigationPosition {
                    line: logical_line,
                    byte_column,
                },
                x: VisualNavigationX { value: x },
                affinity,
            });
        }
        let selected = match request {
            VisualNavigationRowRequest::Source {
                byte_column,
                affinity,
            } => candidates
                .iter()
                .find(|stop| stop.position.byte_column == byte_column && stop.affinity == affinity)
                .cloned(),
            VisualNavigationRowRequest::Target { preferred_x, .. } if preferred_x.is_finite() => {
                candidates.iter().cloned().min_by(|left, right| {
                    let ld = (left.x.value - preferred_x).abs();
                    let rd = (right.x.value - preferred_x).abs();
                    ld.partial_cmp(&rd)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.position.byte_column.cmp(&right.position.byte_column))
                })
            }
            VisualNavigationRowRequest::Target { .. } => None,
        };
        let Some(selected) = selected else {
            scalar_start = scalar_end;
            continue;
        };
        let mut stops = Vec::with_capacity(3);
        for stop in candidates.iter().filter(|stop| {
            stop.position.byte_column == start_column || stop.position.byte_column == end_column
        }) {
            if !stops
                .iter()
                .any(|existing: &VisualNavigationStop| existing.position == stop.position)
            {
                stops.push(stop.clone());
            }
        }
        if !stops.iter().any(|stop| stop.position == selected.position) {
            stops.push(selected.clone());
        }
        stops.sort_by(|left, right| {
            left.position
                .byte_column
                .cmp(&right.position.byte_column)
                .then_with(|| {
                    affinity_sort_key(left.affinity).cmp(&affinity_sort_key(right.affinity))
                })
        });
        stops.truncate(3);
        let row = VisualNavigationRow {
            logical_line,
            row_index: complete_fragment.then_some(row_index as u32),
            row_count: complete_fragment.then_some(galley.rows.len() as u32),
            start: VisualNavigationPosition {
                line: logical_line,
                byte_column: start_column,
            },
            end: VisualNavigationPosition {
                line: logical_line,
                byte_column: end_column,
            },
            stops,
        };
        return Ok(VisualNavigationSelectedRow {
            row,
            source_x: matches!(request, VisualNavigationRowRequest::Source { .. })
                .then_some(selected.x),
            target_stop: matches!(request, VisualNavigationRowRequest::Target { .. })
                .then_some(selected),
        });
    }
    Err(VisualNavigationGeometryError::NeedMoreWindow)
}

fn affinity_sort_key(affinity: legion_protocol::CaretAffinity) -> u8 {
    match affinity {
        legion_protocol::CaretAffinity::Upstream => 0,
        legion_protocol::CaretAffinity::Downstream => 1,
    }
}

fn absolute_byte_at_scalar(line: &DesktopCodeLineViewModel, scalar: usize) -> Option<u64> {
    let local = if scalar == line.text.chars().count() {
        line.text.len()
    } else {
        line.text.char_indices().nth(scalar)?.0
    };
    Some(local as u64)
}

fn scalar_for_local_byte(line: &DesktopCodeLineViewModel, local_byte: u64) -> Option<usize> {
    let local = usize::try_from(local_byte).ok()?;
    if local > line.text.len() || !line.text.is_char_boundary(local) {
        return None;
    }
    Some(line.text[..local].chars().count())
}

fn code_line_galley_cache_key(
    buffer_id: Option<legion_protocol::BufferId>,
    snapshot_id: Option<legion_protocol::SnapshotId>,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
) -> CodeLineGalleyCacheKey {
    CodeLineGalleyCacheKey {
        buffer_id: buffer_id.map(|buffer_id| buffer_id.0).unwrap_or_default(),
        snapshot_id: snapshot_id
            .map(|snapshot_id| snapshot_id.0)
            .unwrap_or_default(),
        content_fingerprint: code_line_content_fingerprint(line),
        font_size_bucket: code_line_font_size_bucket(),
        width_bucket: code_line_width_bucket(wrap_width),
    }
}

fn code_line_galley_cache_id(_buffer_id: legion_protocol::BufferId) -> egui::Id {
    egui::Id::new("legion_desktop_line_galley_cache")
}

fn code_line_content_fingerprint(line: &DesktopCodeLineViewModel) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.text.hash(&mut hasher);
    for highlight in &line.highlights {
        highlight.start_col.hash(&mut hasher);
        highlight.end_col.hash(&mut hasher);
        highlight.kind.hash(&mut hasher);
    }
    hasher.finish()
}

fn code_line_font_size_bucket() -> u32 {
    (theme::tokens().typography.code as f32 * 100.0).round() as u32
}

fn code_line_width_bucket(width: f32) -> u32 {
    if !width.is_finite() || width <= 0.0 {
        0
    } else {
        (width / 4.0).floor() as u32
    }
}

fn paint_current_line_highlight(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    cursor: TextCoordinate,
) {
    if cursor.line != line.number.saturating_sub(1) {
        return;
    }
    ui.painter().rect_filled(
        response.rect.expand2(egui::vec2(3.0, 1.0)),
        0.0,
        theme::tokens().code_canvas.current_line,
    );
}

fn paint_code_selections(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    selections: &[ProtocolTextRange],
    galley: &egui::Galley,
) {
    for selection in selections {
        let Some((start_col, end_col, include_empty_line)) =
            editor_selection_columns_for_line(line, *selection)
        else {
            continue;
        };
        for selection_rect in editor_selection_rects_for_galley(
            line,
            galley,
            response.rect.min,
            start_col,
            end_col,
            include_empty_line,
        ) {
            ui.painter()
                .rect_filled(selection_rect, 0.0, theme::tokens().code_canvas.selection);
        }
    }
}

/// Return the visible scalar interval of a logical selection for one rendered line.
pub fn editor_selection_columns_for_line(
    line: &DesktopCodeLineViewModel,
    selection: ProtocolTextRange,
) -> Option<(usize, usize, bool)> {
    let selection = normalized_text_range(selection);
    let line_index = line.number.saturating_sub(1);
    if line_index < selection.start.line || line_index > selection.end.line {
        return None;
    }
    let line_len = line.text.chars().count();
    let start_col = if line_index == selection.start.line {
        clipped_scalar_for_coordinate(line, selection.start)?
    } else {
        0
    };
    let end_col = if line_index == selection.end.line {
        clipped_scalar_for_coordinate(line, selection.end)?
    } else {
        line_len
    };
    let include_empty_line = line.text.is_empty()
        && selection.start != selection.end
        && line_index >= selection.start.line
        && line_index < selection.end.line;
    if start_col > end_col || (start_col == end_col && !include_empty_line) {
        return None;
    }
    Some((start_col, end_col, include_empty_line))
}

fn normalized_text_range(range: ProtocolTextRange) -> ProtocolTextRange {
    if (range.end.line, range.end.character) < (range.start.line, range.start.character) {
        ProtocolTextRange {
            start: range.end,
            end: range.start,
        }
    } else {
        range
    }
}

fn clipped_scalar_for_coordinate(
    line: &DesktopCodeLineViewModel,
    coordinate: TextCoordinate,
) -> Option<usize> {
    let absolute_byte = if let Some(byte) = coordinate.byte_offset {
        byte
    } else if let Some(origin) = line.line_start_byte_offset {
        origin.checked_add(u64::from(coordinate.character))?
    } else if matches!(
        line.truncation_state,
        ViewportLineTruncationState::Leading | ViewportLineTruncationState::Both
    ) {
        return None;
    } else {
        line.byte_range
            .start
            .checked_add(u64::from(coordinate.character))?
    };
    let clipped_byte = absolute_byte.clamp(line.byte_range.start, line.byte_range.end);
    Some(byte_column_to_scalar_index(
        &line.text,
        (clipped_byte - line.byte_range.start) as u32,
    ))
}

fn paint_code_cursor(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    cursor: TextCoordinate,
    affinity: CaretAffinity,
    primary: bool,
    galley: &egui::Galley,
) {
    if cursor.line != line.number.saturating_sub(1) {
        return;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(530));
    let Some(scalar_index) = local_scalar_for_coordinate(line, cursor) else {
        return;
    };
    let cursor_rect = editor_cursor_rect_for_galley_with_affinity(
        line,
        galley,
        response.rect.min,
        scalar_index,
        affinity,
    );
    let to_global = ui
        .ctx()
        .layer_transform_to_global(ui.layer_id())
        .unwrap_or_default();
    if primary {
        ui.output_mut(|output| {
            output.ime = Some(egui::output::IMEOutput {
                rect: to_global * response.rect,
                cursor_rect: to_global * cursor_rect,
            });
        });
    }
    let blink_on = ui
        .ctx()
        .input(|input| ((input.time * 2.0) as i64).rem_euclid(2) == 0);
    if !blink_on {
        return;
    }
    ui.painter().line_segment(
        [cursor_rect.left_top(), cursor_rect.left_bottom()],
        egui::Stroke::new(1.0_f32, theme::tokens().code_canvas.cursor),
    );
}

/// Paint semi-transparent highlight rectangles for find matches on a code line.
///
/// All matches are painted in yellow; the current match is painted in orange.
fn byte_column_to_display_column(line: &str, byte_column: u32) -> u32 {
    let byte_column = (byte_column as usize).min(line.len());
    line.get(..byte_column)
        .map(|prefix| prefix.chars().count() as u32)
        .unwrap_or_else(|| line.chars().count() as u32)
}

fn byte_column_to_scalar_index(line: &str, byte_column: u32) -> usize {
    let mut boundary = (byte_column as usize).min(line.len());
    while boundary > 0 && !line.is_char_boundary(boundary) {
        boundary -= 1;
    }
    line[..boundary].chars().count()
}

fn scalar_index_to_byte_column(line: &str, scalar_index: usize) -> u32 {
    line.char_indices()
        .nth(scalar_index)
        .map(|(offset, _)| offset)
        .unwrap_or(line.len()) as u32
}

/// Shape an editor line with the same layout job used by the live code canvas.
///
/// This small seam is used by headless geometry tests; it does not own editor
/// state or change the renderer's text authority.
pub fn editor_galley_for_geometry(
    ctx: &egui::Context,
    line: &DesktopCodeLineViewModel,
    wrap_width: f32,
) -> Arc<egui::Galley> {
    let mut galley = None;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        galley = Some(ui.ctx().fonts_mut(|fonts| {
            let mut job = code_line_layout_job(line);
            job.wrap.max_width = wrap_width;
            fonts.layout_job(job)
        }));
    });
    galley.expect("geometry galley should be shaped during an egui frame")
}

/// Map a pointer in a rendered line label to the protocol's UTF-8 byte column.
pub fn editor_coordinate_from_galley_pointer(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    pointer: egui::Pos2,
    origin: egui::Pos2,
) -> Option<TextCoordinate> {
    editor_visual_coordinate_from_galley_pointer(line, galley, pointer, origin)
        .map(|(coordinate, _)| coordinate)
}

/// Map a pointer in a rendered line label to its protocol coordinate and wrap-side affinity.
pub fn editor_visual_coordinate_from_galley_pointer(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    pointer: egui::Pos2,
    origin: egui::Pos2,
) -> Option<(TextCoordinate, CaretAffinity)> {
    let cursor = galley.cursor_from_pos(pointer - origin);
    let scalar_index = cursor.index.min(line.text.chars().count());
    text_coordinate_for_line_scalar(line, scalar_index).map(|coordinate| {
        (
            coordinate,
            if cursor.prefer_next_row {
                CaretAffinity::Downstream
            } else {
                CaretAffinity::Upstream
            },
        )
    })
}

/// Map a drag anchor through the rendered galley, including wrapped-row Y.
pub fn drag_anchor_for_line_pointer_with_galley(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    pointer_x: f32,
    total_drag_delta: egui::Vec2,
    pointer_y: f32,
    origin: egui::Pos2,
) -> Option<TextCoordinate> {
    editor_coordinate_from_galley_pointer(
        line,
        galley,
        egui::pos2(
            pointer_x - total_drag_delta.x,
            pointer_y - total_drag_delta.y,
        ),
        origin,
    )
}

/// Map a drag anchor through the rendered galley, retaining wrap-side affinity.
pub fn drag_anchor_for_line_pointer_with_galley_visual(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    pointer_x: f32,
    total_drag_delta: egui::Vec2,
    pointer_y: f32,
    origin: egui::Pos2,
) -> Option<(TextCoordinate, CaretAffinity)> {
    editor_visual_coordinate_from_galley_pointer(
        line,
        galley,
        egui::pos2(
            pointer_x - total_drag_delta.x,
            pointer_y - total_drag_delta.y,
        ),
        origin,
    )
}

fn text_coordinate_for_line_scalar(
    line: &DesktopCodeLineViewModel,
    scalar_index: usize,
) -> Option<TextCoordinate> {
    let scalar_index = scalar_index.min(line.text.chars().count());
    let local_byte = scalar_index_to_byte_column(&line.text, scalar_index) as u64;
    let local_utf16 = line.text[..local_byte as usize].encode_utf16().count() as u64;
    let logical_base = line
        .line_start_byte_offset
        .map(|origin| line.byte_range.start.checked_sub(origin))
        .unwrap_or_else(|| {
            matches!(
                line.truncation_state,
                ViewportLineTruncationState::None | ViewportLineTruncationState::Trailing
            )
            .then_some(0)
        })?;
    let byte_offset = line.byte_range.start.checked_add(local_byte)?;
    let character = logical_base.checked_add(local_byte)?;
    let utf16_offset = line
        .line_start_utf16_offset
        .and_then(|origin| origin.checked_add(u64::from(line.utf16_range.start.character)))
        .and_then(|base| base.checked_add(local_utf16));
    let character = u32::try_from(character).ok()?;
    let mut coordinate = text_coordinate(line.number.saturating_sub(1), character);
    coordinate.byte_offset = Some(byte_offset);
    coordinate.utf16_offset = utf16_offset;
    Some(coordinate)
}

fn local_scalar_for_coordinate(
    line: &DesktopCodeLineViewModel,
    coordinate: TextCoordinate,
) -> Option<usize> {
    let local_byte = if let Some(absolute) = coordinate.byte_offset {
        if absolute < line.byte_range.start || absolute > line.byte_range.end {
            return None;
        }
        absolute - line.byte_range.start
    } else {
        let logical_base = line
            .line_start_byte_offset
            .map(|origin| line.byte_range.start.checked_sub(origin))
            .unwrap_or_else(|| {
                matches!(
                    line.truncation_state,
                    ViewportLineTruncationState::None | ViewportLineTruncationState::Trailing
                )
                .then_some(0)
            })?;
        u64::from(coordinate.character).checked_sub(logical_base)?
    };
    if local_byte > line.text.len() as u64 {
        return None;
    }
    Some(byte_column_to_scalar_index(&line.text, local_byte as u32))
}

/// Return a short caret rectangle at a scalar cursor position in a rendered line.
pub fn editor_cursor_rect_for_galley(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    origin: egui::Pos2,
    scalar_index: usize,
) -> egui::Rect {
    editor_cursor_rect_for_galley_with_affinity(
        line,
        galley,
        origin,
        scalar_index,
        CaretAffinity::Upstream,
    )
}

/// Return a caret rectangle using the projected wrap-side affinity.
pub fn editor_cursor_rect_for_galley_with_affinity(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    origin: egui::Pos2,
    scalar_index: usize,
    affinity: CaretAffinity,
) -> egui::Rect {
    let scalar_index = scalar_index.min(line.text.chars().count());
    let position = if galley.rows.is_empty()
        || galley
            .pos_from_cursor(egui::text::CCursor {
                index: scalar_index,
                prefer_next_row: affinity == CaretAffinity::Downstream,
            })
            .height()
            <= 0.0
    {
        egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(0.0, code_line_fallback_height()),
        )
    } else {
        galley.pos_from_cursor(egui::text::CCursor {
            index: scalar_index,
            prefer_next_row: affinity == CaretAffinity::Downstream,
        })
    };
    egui::Rect::from_min_max(
        origin + position.min.to_vec2(),
        origin + egui::vec2(position.min.x + 1.0, position.max.y),
    )
}

/// Split a selection into one rectangle per rendered galley row.
pub fn editor_selection_rects_for_galley(
    line: &DesktopCodeLineViewModel,
    galley: &egui::Galley,
    origin: egui::Pos2,
    start_scalar: usize,
    end_scalar: usize,
    include_empty_line: bool,
) -> Vec<egui::Rect> {
    let line_len = line.text.chars().count();
    let start_scalar = start_scalar.min(line_len);
    let end_scalar = end_scalar.min(line_len);
    if start_scalar == end_scalar && !(include_empty_line && line_len == 0) {
        return Vec::new();
    }
    if start_scalar > end_scalar {
        return Vec::new();
    }
    if galley.rows.is_empty() || galley.rows.iter().all(|row| row.rect().height() <= 0.0) {
        return vec![egui::Rect::from_min_size(
            origin,
            egui::vec2(1.0, code_line_fallback_height()),
        )];
    }
    let mut row_start = 0usize;
    let mut rectangles = Vec::new();
    for placed_row in &galley.rows {
        let row_end = row_start + placed_row.glyphs.len();
        let selection_start = start_scalar.max(row_start);
        let selection_end = end_scalar.min(row_end);
        if selection_start < selection_end {
            let start_column = selection_start - row_start;
            let end_column = selection_end - row_start;
            let left = placed_row.pos.x + placed_row.x_offset(start_column);
            let right = (placed_row.pos.x + placed_row.x_offset(end_column)).max(left + 1.0);
            let row_rect = placed_row.rect();
            rectangles.push(egui::Rect::from_min_max(
                origin + egui::vec2(left, row_rect.top()),
                origin + egui::vec2(right, row_rect.bottom()),
            ));
        }
        row_start = row_end + if placed_row.ends_with_newline { 1 } else { 0 };
    }
    rectangles
}

fn code_line_fallback_height() -> f32 {
    (theme::tokens().typography.code as f32 * 1.2).max(1.0)
}

fn paint_find_match_highlights(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    find_bar: &legion_ui::ui::FindBarProjection,
    char_width: f32,
) {
    if !find_bar.visible || find_bar.matches.is_empty() {
        return;
    }
    let line_zero = line.number.saturating_sub(1);
    let yellow = theme::tokens().search.match_highlight;
    let orange = theme::tokens().search.current_match;

    for (i, m) in find_bar.matches.iter().enumerate() {
        // Skip matches that don't overlap this line.
        if m.start.line > line_zero || m.end.line < line_zero {
            continue;
        }
        let start_char = if m.start.line == line_zero {
            byte_column_to_display_column(&line.text, m.start.character)
        } else {
            0
        };
        let end_char = if m.end.line == line_zero {
            byte_column_to_display_column(&line.text, m.end.character)
        } else {
            line.text.chars().count() as u32
        };
        if start_char >= end_char {
            continue;
        }
        let start_x = response.rect.left() + start_char as f32 * char_width;
        let end_x = response.rect.left() + end_char as f32 * char_width;
        let color = if i == find_bar.current_match_index {
            orange
        } else {
            yellow
        };
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(start_x, response.rect.top()),
                egui::pos2(end_x, response.rect.bottom()),
            ),
            egui::CornerRadius::ZERO,
            color,
        );
    }
}

fn paint_diagnostic_underlines(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    problems: &[LanguageProblemProjection],
    char_width: f32,
) {
    let line_zero = line.number.saturating_sub(1);
    let line_chars = line.text.chars().count() as u32;
    for problem in problems {
        let Some(range) = problem.range.as_ref() else {
            continue;
        };
        let Some((start_char, end_char)) =
            crate::diagnostic_underline::diagnostic_underline_span(line_zero, line_chars, range)
        else {
            continue;
        };
        let start_x = response.rect.left() + start_char as f32 * char_width;
        let end_x = response.rect.left() + end_char as f32 * char_width;
        let y = response.rect.bottom() - 1.0;
        let color = match problem.severity {
            ProtocolDiagnosticSeverity::Error => theme::tokens().diagnostic.error,
            ProtocolDiagnosticSeverity::Warning => theme::tokens().diagnostic.warning,
            ProtocolDiagnosticSeverity::Info => theme::tokens().diagnostic.info,
            ProtocolDiagnosticSeverity::Hint => theme::tokens().diagnostic.hint,
        };
        ui.painter().line_segment(
            [egui::pos2(start_x, y), egui::pos2(end_x, y)],
            egui::Stroke::new(1.5_f32, color),
        );
    }
}

fn show_diagnostic_tooltip(
    ui: &egui::Ui,
    response: &egui::Response,
    line: &DesktopCodeLineViewModel,
    problems: &[LanguageProblemProjection],
    char_width: f32,
) {
    if !response.hovered() {
        return;
    }
    let Some(hover_pos) = response.hover_pos() else {
        return;
    };
    let hover_col = ((hover_pos.x - response.rect.left()) / char_width).max(0.0) as u32;
    let line_zero = line.number.saturating_sub(1);
    let matching: Vec<&LanguageProblemProjection> = problems
        .iter()
        .filter(|p| {
            let Some(range) = p.range.as_ref() else {
                return false;
            };
            if range.start.line > line_zero || range.end.line < line_zero {
                return false;
            }
            let start_char = if range.start.line == line_zero {
                range.start.character
            } else {
                0
            };
            let end_char = if range.end.line == line_zero {
                range.end.character
            } else {
                line.text.chars().count() as u32
            };
            hover_col >= start_char && hover_col < end_char
        })
        .collect();
    if matching.is_empty() {
        return;
    }
    egui::Tooltip::always_open(
        ui.ctx().clone(),
        ui.layer_id(),
        ui.id().with("diag_tooltip"),
        egui::PopupAnchor::Pointer,
    )
    .show(|ui: &mut egui::Ui| {
        for (i, problem) in matching.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let color = match problem.severity {
                ProtocolDiagnosticSeverity::Error => theme::tokens().diagnostic.error,
                ProtocolDiagnosticSeverity::Warning => theme::tokens().diagnostic.warning,
                ProtocolDiagnosticSeverity::Info => theme::tokens().diagnostic.info,
                ProtocolDiagnosticSeverity::Hint => theme::tokens().diagnostic.hint,
            };
            ui.colored_label(color, format!("{:?}", problem.severity));
            ui.label(&problem.message);
            if let Some(source) = &problem.source_label {
                ui.label(theme::muted(source));
            }
            if let Some(code) = &problem.code_label {
                ui.label(theme::muted(code));
            }
        }
    });
}

/// Render inlay hints as semi-transparent ghost text at their positions.
fn paint_inlay_hints(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    inlay_hints: &[LanguageInlayHintProjection],
    char_width: f32,
) {
    let line_zero = line.number.saturating_sub(1);
    for hint in inlay_hints {
        if hint.position.line != line_zero {
            continue;
        }
        let mut x = response.rect.left() + hint.position.character as f32 * char_width;
        if hint.padding_left {
            x += char_width * 0.5;
        }
        let label = if hint.kind_label.contains("type") {
            format!(": {}", hint.label)
        } else {
            hint.label.clone()
        };
        ui.painter().text(
            egui::pos2(x, response.rect.top()),
            egui::Align2::LEFT_TOP,
            &label,
            egui::FontId::monospace(theme::tokens().typography.code as f32),
            theme::tokens().chrome.fold_indicator,
        );
    }
}

/// Show a picker popup when multiple definition locations are available.
fn render_definition_picker(
    ui: &mut egui::Ui,
    definitions: &[LanguageLocationProjection],
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_definition_picker".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(320.0, -100.0))
        .show(ui.ctx(), |ui| {
            ui.set_max_width(500.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.label(theme::body_strong("Go to Definition"));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .id_salt("definition_picker_scroll")
                        .show(ui, |ui| {
                            for (i, def) in definitions.iter().enumerate() {
                                let path_label = def
                                    .path
                                    .as_ref()
                                    .map(|p| p.0.as_str())
                                    .unwrap_or("<unknown>");
                                let row = egui::Frame::new()
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(theme::body(&def.label));
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(theme::code_muted(trim_middle(
                                                        path_label, 60,
                                                    )));
                                                },
                                            );
                                        });
                                    })
                                    .response;
                                if row.clicked() {
                                    actions.push(DesktopAction::NavigateToDefinition { index: i });
                                }
                            }
                        });
                });
        });
}

/// Show bounded, keyboard-focusable reference locations from the authoritative
/// language projection. Activation emits the normal path-opening action.
fn render_reference_panel(
    ui: &mut egui::Ui,
    references: &[LanguageLocationProjection],
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_reference_panel".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -100.0))
        .show(ui.ctx(), |ui| {
            ui.set_max_width(560.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.label(theme::body_strong("References"));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .id_salt("reference_panel_scroll")
                        .show(ui, |ui| {
                            for reference in references.iter().take(64) {
                                let path = reference
                                    .path
                                    .as_ref()
                                    .map(|path| path.0.as_str())
                                    .unwrap_or("<unavailable path>");
                                let (line, character) = reference
                                    .range
                                    .as_ref()
                                    .map(|range| (range.start.line, range.start.character))
                                    .unwrap_or((0, 0));
                                let label = format!(
                                    "{}  {}:{}:{}",
                                    reference.label,
                                    path,
                                    line + 1,
                                    character + 1
                                );
                                if ui
                                    .add(egui::Button::new(theme::body(&label)).wrap())
                                    .clicked()
                                    && reference.path.is_some()
                                    && reference.range.is_some()
                                {
                                    actions.push(DesktopAction::NavigateToReference {
                                        path: path.to_owned(),
                                        line,
                                        character,
                                    });
                                }
                            }
                        });
                });
        });
}

fn render_code_action_panel(
    ui: &mut egui::Ui,
    candidates: &[LanguageCodeActionProjection],
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    egui::Area::new("legion_desktop_code_action_panel".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
        .show(ui.ctx(), |ui| {
            ui.set_max_width(560.0);
            egui::Frame::new()
                .fill(tokens.bg.panel)
                .stroke(egui::Stroke::new(1.0_f32, tokens.border.default))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(theme::body_strong("Code actions"));
                        if ui.button("Request").clicked() {
                            actions.push(DesktopAction::RequestCodeActions);
                        }
                    });
                    if candidates.is_empty() {
                        ui.label(theme::muted("No code actions available"));
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .id_salt("code_action_panel_scroll")
                        .show(ui, |ui| {
                            for candidate in candidates.iter().take(64) {
                                let suffix = candidate
                                    .kind
                                    .as_deref()
                                    .map(|kind| format!(" ({kind})"))
                                    .unwrap_or_default();
                                let disabled = candidate.disabled_reason.is_some();
                                let text = if let Some(reason) = &candidate.disabled_reason {
                                    format!("{}{} — disabled: {}", candidate.title, suffix, reason)
                                } else {
                                    format!("{}{}", candidate.title, suffix)
                                };
                                if ui
                                    .add_enabled(
                                        !disabled,
                                        egui::Button::new(theme::body(&text)).wrap(),
                                    )
                                    .clicked()
                                {
                                    actions.push(DesktopAction::SelectCodeAction {
                                        response_id: candidate.response_id.clone(),
                                        action_id: candidate.action_id.clone(),
                                    });
                                }
                            }
                        });
                });
        });
}

fn paint_ime_composition(
    ui: &egui::Ui,
    line: &DesktopCodeLineViewModel,
    response: &egui::Response,
    cursor: TextCoordinate,
    affinity: CaretAffinity,
    source_galley: &egui::Galley,
    ime_composition: &ImeCompositionProjection,
) {
    if cursor.line != line.number.saturating_sub(1) || !ime_composition.active {
        return;
    }
    let preedit = ime_composition.preedit.as_str();
    if preedit.is_empty() {
        return;
    }

    let Some(scalar_index) = local_scalar_for_coordinate(line, cursor) else {
        return;
    };
    let cursor_rect = editor_cursor_rect_for_galley_with_affinity(
        line,
        source_galley,
        response.rect.min,
        scalar_index,
        affinity,
    );
    let font_id = egui::FontId::monospace(theme::tokens().typography.code as f32);
    let galley =
        ui.painter()
            .layout_no_wrap(preedit.to_string(), font_id, theme::tokens().accent.orange);
    let top_left = cursor_rect.left_top();
    let ime_rect = egui::Rect::from_min_size(top_left, galley.size());
    ui.painter().rect_filled(
        ime_rect.expand2(egui::vec2(2.0, 1.0)),
        2.0,
        theme::tokens().bg.input,
    );
    ui.painter()
        .galley(top_left, galley, theme::tokens().accent.orange);
    ui.painter().line_segment(
        [
            egui::pos2(ime_rect.left(), ime_rect.bottom() - 1.0),
            egui::pos2(ime_rect.right(), ime_rect.bottom() - 1.0),
        ],
        egui::Stroke::new(1.0_f32, theme::tokens().accent.orange),
    );
}

fn code_line_layout_job(line: &DesktopCodeLineViewModel) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut cursor_col = 0;
    let char_count = line.text.chars().count() as u32;
    let mut highlights = line.highlights.clone();
    highlights.sort_by_key(|span| (span.start_col, span.end_col));

    for span in highlights {
        let start_col = span.start_col.min(char_count);
        let end_col = span.end_col.min(char_count);
        if start_col >= end_col || start_col < cursor_col {
            continue;
        }
        append_code_segment(
            &mut job,
            char_slice(&line.text, cursor_col, start_col),
            theme::tokens().text.secondary,
        );
        append_code_segment(
            &mut job,
            char_slice(&line.text, start_col, end_col),
            semantic_token_color(span.kind),
        );
        cursor_col = end_col;
    }

    append_code_segment(
        &mut job,
        char_slice(&line.text, cursor_col, char_count),
        theme::tokens().text.secondary,
    );
    job
}

fn append_code_segment(job: &mut egui::text::LayoutJob, text: &str, color: egui::Color32) {
    if text.is_empty() {
        return;
    }
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::monospace(theme::tokens().typography.code as f32),
            color,
            ..Default::default()
        },
    );
}

fn char_slice(text: &str, start_col: u32, end_col: u32) -> &str {
    let start = char_col_to_byte_index(text, start_col).unwrap_or(text.len());
    let end = char_col_to_byte_index(text, end_col).unwrap_or(text.len());
    if start <= end { &text[start..end] } else { "" }
}

fn char_col_to_byte_index(text: &str, column: u32) -> Option<usize> {
    if column == 0 {
        return Some(0);
    }
    text.char_indices()
        .nth(column as usize)
        .map(|(index, _)| index)
        .or_else(|| (column as usize == text.chars().count()).then_some(text.len()))
}

fn render_assisted_suggestion_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    _model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    ui.add_space(8.0);
    theme::card_frame_tinted(
        theme::tokens().bg.card,
        theme::dim(theme::tokens().accent.cyan, 80),
    )
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::accent(
                "Inline prediction",
                theme::tokens().accent.cyan,
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if snapshot
                    .assist_inline_prediction_projection
                    .request_in_flight
                {
                    if soft_button(ui, "Cancel").clicked() {
                        actions.push(DesktopAction::CancelAssistInlinePrediction);
                    }
                } else if snapshot
                    .assist_inline_prediction_projection
                    .active_prediction
                    .is_none()
                    && soft_button(ui, "Predict").clicked()
                {
                    actions.push(DesktopAction::RequestAssistInlinePrediction {
                        position: projected_cursor(snapshot),
                    });
                }
            });
        });
        if let Some(prediction) = &snapshot
            .assist_inline_prediction_projection
            .active_prediction
        {
            theme::small_card_frame().show(ui, |ui| {
                ui.label(theme::code(&prediction.ghost_text_label));
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted(&prediction.provider_label));
                    ui.separator();
                    ui.label(theme::muted(&prediction.status_label));
                    if prediction.stale {
                        ui.separator();
                        ui.label(theme::accent("stale", theme::tokens().accent.orange));
                    }
                });
                if let Some(preview) = &prediction.replacement_preview_label {
                    ui.label(theme::code_muted(preview));
                }
            });
            ui.horizontal(|ui| {
                if primary_button(ui, "Accept", theme::tokens().accent.green).clicked() {
                    actions.push(DesktopAction::AcceptCurrentAssistInlinePrediction);
                }
                if soft_button(ui, "Dismiss").clicked() {
                    actions.push(DesktopAction::DismissCurrentAssistInlinePrediction);
                }
            });
        } else if !snapshot
            .assist_inline_prediction_projection
            .request_in_flight
            && snapshot.assist_inline_prediction_projection.rows.is_empty()
        {
            ui.label(theme::muted("No predictions yet"));
        }
        let active_id = snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .as_ref()
            .map(|prediction| prediction.prediction_id.as_str());
        let next_edits = snapshot
            .assist_inline_prediction_projection
            .rows
            .iter()
            .filter(|prediction| Some(prediction.prediction_id.as_str()) != active_id)
            .collect::<Vec<_>>();
        if !next_edits.is_empty() {
            section_label(
                ui,
                "Next-edit predictions",
                Some(theme::tokens().accent.orange),
            );
            for prediction in next_edits.into_iter().take(4) {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(&prediction.ghost_text_label));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::muted(&prediction.provider_label));
                        ui.separator();
                        ui.label(theme::muted(&prediction.status_label));
                        if let Some(latency_ms) = prediction.latency_ms {
                            ui.separator();
                            ui.label(theme::code_muted(format!("{latency_ms} ms")));
                        }
                    });
                });
            }
        }
        let attempts = snapshot
            .assist_inline_prediction_projection
            .after_edit_prediction_attempts;
        if attempts > 0 {
            let accepts = snapshot
                .assist_inline_prediction_projection
                .after_edit_prediction_accepts;
            let precision = (accepts.saturating_mul(100)) / attempts.max(1);
            ui.horizontal_wrapped(|ui| {
                pill(
                    ui,
                    &format!("next-edit precision: {accepts}/{attempts} ({precision}%)"),
                    theme::tokens().accent.orange,
                    true,
                );
            });
        }
        section_label(ui, "Context", Some(theme::tokens().accent.violet));
        theme::small_card_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                pill(
                    ui,
                    &format!("file: {}", trim_middle(current_path(snapshot), 18)),
                    theme::tokens().accent.blue,
                    true,
                );
                if snapshot.active_buffer_projection.workspace_id.is_some() {
                    pill(ui, "workspace: current", theme::tokens().accent.cyan, true);
                }
                let manifest = &snapshot.context_manifest_projection.manifest;
                pill(
                    ui,
                    &format!("manifest: {} items", manifest.items.len()),
                    theme::tokens().accent.green,
                    !manifest.items.is_empty(),
                );
                if let Some(selected_item_id) = snapshot
                    .context_manifest_projection
                    .selected_item_id
                    .as_ref()
                {
                    pill(
                        ui,
                        &format!("selected: {}", trim_middle(selected_item_id, 28)),
                        theme::tokens().accent.amber,
                        true,
                    );
                }
            });
        });
    });
}

fn render_delegated_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    if let Some(proposal_id) = next_delegate_owned_proposal_id(snapshot) {
        let lifecycle = delegate_owned_proposal_lifecycle(snapshot, proposal_id);
        let awaiting_decision = lifecycle.is_some_and(|state| {
            matches!(
                state,
                ProposalLifecycleState::Created
                    | ProposalLifecycleState::Validated
                    | ProposalLifecycleState::Previewed
            )
        });
        let approved = lifecycle == Some(ProposalLifecycleState::Approved);
        theme::pane_frame(theme::tokens().bg.code).show(ui, |ui| {
            ui.set_height(220.0);
            ui.horizontal(|ui| {
                section_label(
                    ui,
                    "Delegated Diff Review",
                    Some(theme::tokens().accent.violet),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if approved
                        && primary_button(
                            ui,
                            "Apply approved changes",
                            theme::tokens().accent.green,
                        )
                        .clicked()
                    {
                        actions.push(DesktopAction::ApplyProposal { proposal_id });
                    } else if awaiting_decision
                        && primary_button(ui, "Approve", theme::tokens().accent.blue).clicked()
                    {
                        actions.push(DesktopAction::ApproveProposal { proposal_id });
                    }
                    if awaiting_decision && soft_button(ui, "Request Changes").clicked() {
                        actions.push(DesktopAction::RejectProposal {
                            proposal_id,
                            reason: ProposalRejectionReason::UserRejected,
                        });
                    }
                });
            });
            render_compact_rows(
                ui,
                &delegated_proposal_rows(snapshot),
                "No delegated proposal selected",
                1,
            );
            render_delegated_hunk_review_controls(ui, snapshot, actions);
        });
        ui.separator();
    }
    render_delegate_task_board(ui, snapshot, model, actions);
}

fn render_fleet_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    _model: &DesktopProjectionViewModel,
    _actions: &mut Vec<DesktopAction>,
) {
    section_label(ui, "Workflow board", Some(theme::tokens().accent.purple));
    fleet_board::render_fleet_board(ui, &snapshot.legion_workflow_board_columns);
}

fn render_delegate_task_board(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    // This board is nested in the bounded advanced-workbench viewport. Keep
    // its own non-shrinking horizontal scroll finite so it cannot claim the
    // editor allocation below the outer vertical scroll region.
    let board_height = 180.0_f32;
    egui::ScrollArea::horizontal()
        .id_salt("legion_desktop_task_board")
        .auto_shrink([false, false])
        .max_height(board_height)
        .show(ui, |ui| {
            ui.set_min_height(board_height);
            ui.horizontal_top(|ui| {
                delegate_task_column(
                    ui,
                    "ASSIGNED",
                    theme::tokens().text.muted,
                    delegated_plan_rows(snapshot, model, 0),
                    actions,
                );
                delegate_task_column(
                    ui,
                    "IN PROGRESS",
                    theme::tokens().accent.blue,
                    delegated_step_rows(snapshot, model),
                    actions,
                );
                delegate_task_column(
                    ui,
                    "WAITING ON HUMAN",
                    theme::tokens().accent.orange,
                    proposal_board_rows(snapshot, model),
                    actions,
                );
                delegate_task_column(
                    ui,
                    "TESTING",
                    theme::tokens().accent.violet,
                    delegated_testing_rows(snapshot),
                    actions,
                );
                delegate_task_column(
                    ui,
                    "DONE",
                    theme::tokens().accent.green,
                    delegated_done_rows(snapshot),
                    actions,
                );
            });
        });
}

fn delegate_task_column(
    ui: &mut egui::Ui,
    title: &str,
    color: egui::Color32,
    rows: Vec<String>,
    _actions: &mut Vec<DesktopAction>,
) {
    theme::card_frame_tinted(theme::tokens().bg.canvas, theme::tokens().border.subtle).show(
        ui,
        |ui| {
            ui.set_width(260.0);
            ui.horizontal(|ui| {
                status_dot(ui, color);
                ui.label(theme::accent(title, theme::tokens().text.secondary));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    pill(ui, &rows.len().to_string(), color, false);
                });
            });
            ui.separator();
            if rows.is_empty() {
                ui.label(theme::muted("No tasks"));
            }
            for (index, row) in rows.iter().take(5).enumerate() {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(trim_middle(row, 54)));
                    ui.horizontal(|ui| {
                        let label = format!("{}", index + 1);
                        avatar(ui, &label, color);
                        ui.label(theme::muted("Details"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(theme::accent("review", color));
                        });
                    });
                });
            }
        },
    );
}

/// Height the runnables list may occupy before it scrolls.
///
/// The activity sidebar is not itself scrollable, so an unbounded list would
/// push the test rows below it off the panel.
const RUNNABLES_MAX_HEIGHT: f32 = 96.0;

fn render_test_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    // Runnable code lenses, as buttons. rust-analyzer reports these for every
    // `#[test]` and every binary target, the app turns one into a terminal
    // launch, and until now they rendered only inside a diagnostic string —
    // so the "run this test" affordance existed everywhere except on screen.
    let runnables: Vec<_> = snapshot
        .language_tooling_projection
        .code_lenses
        .iter()
        .filter(|lens| lens.kind_label.contains("runnable"))
        .collect();
    if !runnables.is_empty()
        && let Some(buffer_id) = snapshot.active_buffer_projection.buffer_id
    {
        ui.label(theme::label("Runnables"));
        // Every runnable, never a capped subset: a list that hides entries is
        // a capability nobody can reach, however honestly it counts them.
        egui::ScrollArea::vertical()
            .id_salt("legion_desktop_runnables")
            .max_height(RUNNABLES_MAX_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for lens in &runnables {
                        if soft_button(ui, &lens.title)
                            .on_hover_text(&lens.command_label)
                            .clicked()
                        {
                            actions.push(DesktopAction::ActivateLanguageCodeLens {
                                buffer_id,
                                lens_id: lens.lens_id.clone(),
                            });
                        }
                    }
                });
            });
    }
    ui.horizontal_wrapped(|ui| {
        if soft_button(ui, "Refresh tests").clicked() {
            actions.push(DesktopAction::RefreshTestExplorer);
        }
        if let Some(item_id) = snapshot
            .test_explorer_projection
            .items
            .first()
            .map(|item| item.item_id.clone())
            && soft_button(ui, "Run first listed test").clicked()
        {
            actions.push(DesktopAction::RunTestExplorerItem { item_id });
        }
        if let Some(parent) = snapshot
            .test_explorer_projection
            .items
            .first()
            .and_then(|item| item.parent_label.clone())
            && soft_button(ui, "Run first group").clicked()
        {
            actions.push(DesktopAction::RunTestExplorerGroup {
                parent_label: parent,
            });
        }
        if soft_button(ui, "Run cargo test").clicked() {
            // Two actions, because launching is not running. `TerminalLaunch`
            // spawns the shell and uses `command_label` for the status line and
            // the audit record only — it never writes the command to the PTY.
            // This button therefore opened a shell, reported "Terminal running:
            // cargo test", and ran nothing.
            //
            // Sending the command as input is what runs it, and it goes through
            // the `terminal.input` capability gate rather than inventing a new
            // authority that writes to a PTY without one.
            actions.push(DesktopAction::TerminalLaunch {
                command_label: RUN_TESTS_COMMAND.to_string(),
            });
            actions.push(DesktopAction::TerminalInput {
                payload: format!("{RUN_TESTS_COMMAND}\r"),
            });
        }
    });
    render_test_explorer_tree(ui, snapshot, actions);
}

/// The command the Tests surface offers to run.
///
/// Named so the button label, the status line and the bytes sent to the PTY
/// cannot drift apart -- which is exactly how the button came to claim it ran
/// something it never sent.
const RUN_TESTS_COMMAND: &str = "cargo test";

fn render_utility_overlay(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    let Some(surface @ (UtilitySurface::Settings | UtilitySurface::Setup | UtilitySurface::About)) =
        view.utility_surface
    else {
        return;
    };
    let title = match surface {
        UtilitySurface::Settings => "Settings",
        UtilitySurface::Setup => "Welcome to Legion",
        UtilitySurface::About => "About Legion",
        UtilitySurface::Diagnostics => unreachable!(),
    };
    let content_rect = ctx.content_rect();
    let overlay_width = (content_rect.width() - 48.0).clamp(360.0, 920.0);
    let overlay_height = (content_rect.height() - 72.0).clamp(320.0, 720.0);
    let offset = egui::vec2(
        (content_rect.width() - overlay_width) * 0.5,
        (content_rect.height() - overlay_height) * 0.5,
    );
    let modal_id = egui::Id::new(("legion_desktop_utility_overlay", title));
    let mut close = false;
    let focused_before_tab = ctx.memory(|memory| memory.focused());
    let (wrap_forward, wrap_backward) = view
        .utility_overlay_focus_bounds
        .map(|(first, last)| {
            let forward = focused_before_tab == Some(last)
                && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
            let backward = focused_before_tab == Some(first)
                && ctx.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
            (forward, backward)
        })
        .unwrap_or((false, false));
    let mut first_focus = None;
    let mut last_focus = None;
    let modal = egui::Modal::new(modal_id)
        .area(
            egui::Modal::default_area(modal_id)
                .anchor(egui::Align2::LEFT_TOP, offset)
                .default_size([overlay_width, overlay_height]),
        )
        .frame(theme::pane_frame(theme::tokens().bg.panel))
        .show(ctx, |ui| {
            ui.set_min_width(overlay_width);
            ui.set_max_width(overlay_width);
            ui.set_min_height(overlay_height);
            ctx.accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Dialog);
                node.set_label(title);
                node.set_modal();
            });
            ui.horizontal(|ui| {
                ui.label(theme::title(title));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_label = match surface {
                        UtilitySurface::Settings => "Close Settings",
                        UtilitySurface::Setup => "Close Setup",
                        UtilitySurface::About => "Close About",
                        UtilitySurface::Diagnostics => unreachable!(),
                    };
                    let close_response = soft_button(ui, close_label);
                    first_focus = Some(close_response.id);
                    if close_response.clicked() {
                        close = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(("legion_desktop_utility_overlay_scroll", title))
                .auto_shrink([false, false])
                .show(ui, |ui| match surface {
                    UtilitySurface::Settings => {
                        last_focus =
                            Some(render_settings_panel(ui, snapshot, model, view, actions));
                    }
                    UtilitySurface::Setup => {
                        last_focus = Some(render_setup_panel(
                            ui, snapshot, model, view, actions, &mut close,
                        ));
                    }
                    UtilitySurface::About => {
                        last_focus = Some(about::render_about_panel(
                            ui, snapshot, model, view, actions,
                        ));
                    }
                    UtilitySurface::Diagnostics => unreachable!(),
                });
        });
    if let (Some(first), Some(last)) = (first_focus, last_focus) {
        view.utility_overlay_focus_bounds = Some((first, last));
        if wrap_forward {
            ctx.memory_mut(|memory| memory.request_focus(first));
        } else if wrap_backward {
            ctx.memory_mut(|memory| memory.request_focus(last));
        }
    }
    if close || modal.should_close() {
        view.close_utility_overlay(true);
    }
}

fn render_setup_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
    close: &mut bool,
) -> egui::Id {
    let checklist = DesktopSetupChecklistViewModel::from_snapshot(snapshot, &model.settings);
    ui.label(theme::body(
        "Use this checklist to finish the essentials. You can reopen Setup at any time.",
    ));
    section_label(ui, "Setup checklist", Some(theme::tokens().accent.green));
    theme::small_card_frame().show(ui, |ui| {
        for (index, item) in checklist.items.iter().enumerate() {
            if index > 0 {
                ui.add_space(8.0);
            }
            ui.label(theme::body_strong(item.title));
            ui.label(theme::muted(&item.detail));
        }
    });
    ui.add_space(12.0);
    let focus_finish = view.utility_overlay_needs_focus;
    if focus_finish {
        view.utility_overlay_needs_focus = false;
    }
    ui.horizontal(|ui| {
        let review = soft_button(ui, "Review Settings");
        if review.clicked() {
            actions.push(DesktopAction::OpenSettings);
            view.utility_surface = Some(UtilitySurface::Settings);
            view.settings_section = SettingsSection::Privacy;
            view.utility_overlay_needs_focus = true;
            view.utility_overlay_focus_bounds = None;
        }
        let finish = primary_button(ui, "Finish setup", theme::tokens().accent.blue);
        if focus_finish {
            finish.request_focus();
        }
        if finish.clicked() {
            actions.push(DesktopAction::DismissOnboarding);
            *close = true;
        }
        finish.id
    })
    .inner
}

fn bound_typescript_toolchain_path(value: &mut String) {
    if value.chars().count() > TYPESCRIPT_TOOLCHAIN_PATH_MAX_CHARS {
        *value = value
            .chars()
            .take(TYPESCRIPT_TOOLCHAIN_PATH_MAX_CHARS)
            .collect();
    }
}

fn bounded_typescript_toolchain_path(mut value: String) -> String {
    bound_typescript_toolchain_path(&mut value);
    value
}

fn active_file_supports_typescript(snapshot: &ShellProjectionSnapshot) -> bool {
    snapshot
        .active_buffer_projection
        .file_path
        .as_ref()
        .and_then(|path| path.0.rsplit_once('.').map(|(_, extension)| extension))
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
            )
        })
}

fn render_settings_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) -> egui::Id {
    let mut last_focus = None;
    ui.horizontal_wrapped(|ui| {
        for section in SettingsSection::ALL {
            let selected = section == view.settings_section;
            let response = selectable_pill_button(
                ui,
                section.label(),
                if selected {
                    theme::tokens().accent.blue
                } else {
                    theme::tokens().text.muted
                },
                selected,
            );
            if view.utility_overlay_needs_focus && section == view.settings_section {
                response.request_focus();
                view.utility_overlay_needs_focus = false;
            }
            if response.clicked() {
                view.settings_section = section;
            }
        }
    });
    section_label(
        ui,
        view.settings_section.label(),
        Some(theme::tokens().accent.blue),
    );
    theme::small_card_frame().show(ui, |ui| {
        if view.settings_section == SettingsSection::Appearance {
            ui.horizontal(|ui| {
                ui.label(theme::label("Theme"));
                for preference in [
                    ThemePreferenceProjection::Dark,
                    ThemePreferenceProjection::Light,
                    ThemePreferenceProjection::System,
                ] {
                    let selected = model.settings.theme_preference == preference;
                    let response = selectable_pill_button(
                        ui,
                        preference.label(),
                        if selected {
                            theme::tokens().accent.blue
                        } else {
                            theme::tokens().text.muted
                        },
                        selected,
                    );
                    if response.clicked() && !selected {
                        actions.push(DesktopAction::SetThemePreference { preference });
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(theme::label("Zoom"));
                if soft_button(ui, "-").clicked() {
                    actions.push(DesktopAction::SetZoomPercent {
                        zoom_percent: model.settings.zoom_percent.saturating_sub(10),
                    });
                }
                ui.label(theme::code(format!("{}%", model.settings.zoom_percent)));
                if soft_button(ui, "+").clicked() {
                    actions.push(DesktopAction::SetZoomPercent {
                        zoom_percent: model.settings.zoom_percent.saturating_add(10),
                    });
                }
                if soft_button(ui, "Reset").clicked() {
                    actions.push(DesktopAction::SetZoomPercent { zoom_percent: 100 });
                }
            });
        }
        if view.settings_section == SettingsSection::Extensions {
            extensions_panel::render_extensions_panel(ui, &model.extensions_panel, actions);
        }
        if view.settings_section == SettingsSection::Editor {
            ui.horizontal(|ui| {
                ui.label(theme::label("Editor font"));
                if soft_button(ui, "-").clicked() {
                    actions.push(DesktopAction::SetEditorFontSize {
                        font_size_pt: model.settings.editor_font_size_pt.saturating_sub(1),
                    });
                }
                ui.label(theme::code(format!(
                    "{} / {} pt",
                    model.settings.editor_font_family, model.settings.editor_font_size_pt
                )));
                if soft_button(ui, "+").clicked() {
                    actions.push(DesktopAction::SetEditorFontSize {
                        font_size_pt: model.settings.editor_font_size_pt.saturating_add(1),
                    });
                }
            });
        }
        if view.settings_section == SettingsSection::Editor {
            // Line wrapping has a projected setting, an intent, and app
            // handling, but no control, so the only value reachable in the
            // product was the default.
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::label("Line wrapping"));
                for (policy, label) in [
                    (LineWrappingPolicy::Off, "Off"),
                    (LineWrappingPolicy::Viewport, "Viewport"),
                    (LineWrappingPolicy::FixedColumn, "Column"),
                ] {
                    let selected = model.settings.line_wrapping_policy == policy;
                    let response =
                        selectable_pill_button(ui, label, theme::tokens().accent.cyan, selected);
                    if response.clicked() {
                        actions.push(DesktopAction::SetLineWrappingPolicy {
                            policy,
                            wrap_column: model.settings.wrap_column,
                        });
                    }
                }
            });
        }
        if view.settings_section == SettingsSection::Notifications {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::label("Toasts"));
                for verbosity in [
                    ToastVerbosityProjection::ErrorsOnly,
                    ToastVerbosityProjection::WarningsAndErrors,
                    ToastVerbosityProjection::All,
                ] {
                    let selected = model.settings.toast_verbosity == verbosity;
                    let response = selectable_pill_button(
                        ui,
                        verbosity.label(),
                        if selected {
                            theme::tokens().accent.orange
                        } else {
                            theme::tokens().text.muted
                        },
                        selected,
                    );
                    if response.clicked() && !selected {
                        actions.push(DesktopAction::SetToastVerbosity { verbosity });
                    }
                }
            });
        }
        if view.settings_section == SettingsSection::LanguageTools {
            let toolchain = &snapshot
                .language_tooling_projection
                .typescript_toolchain;
            let status = match toolchain.status {
                LanguageToolchainConfigurationStatus::Unconfigured => "Unconfigured",
                LanguageToolchainConfigurationStatus::Draft => "Draft",
                LanguageToolchainConfigurationStatus::Configured => "Configured",
            };
            ui.horizontal(|ui| {
                ui.label(theme::label("TypeScript / JavaScript"));
                ui.label(theme::muted(format!("Status: {status}")));
            });
            ui.label(theme::muted(
                "Choose metadata paths; app authority validates them when starting the server.",
            ));
            ui.horizontal(|ui| {
                ui.label(theme::muted("Language server archive"));
                let response = ui.add(interactive_fields::settings_path_text_edit(
                    &mut view.typescript_toolchain_draft.server_archive,
                    "server archive path",
                ));
                if response.changed() {
                    bound_typescript_toolchain_path(
                        &mut view.typescript_toolchain_draft.server_archive,
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label(theme::muted("Compiler archive"));
                let response = ui.add(interactive_fields::settings_path_text_edit(
                    &mut view.typescript_toolchain_draft.compiler_archive,
                    "compiler archive path",
                ));
                if response.changed() {
                    bound_typescript_toolchain_path(
                        &mut view.typescript_toolchain_draft.compiler_archive,
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label(theme::muted("Node executable"));
                let response = ui.add(interactive_fields::settings_path_text_edit(
                    &mut view.typescript_toolchain_draft.node_executable,
                    "node executable path",
                ));
                if response.changed() {
                    bound_typescript_toolchain_path(
                        &mut view.typescript_toolchain_draft.node_executable,
                    );
                }
            });
            let ready = !view.typescript_toolchain_draft.server_archive.is_empty()
                && !view.typescript_toolchain_draft.compiler_archive.is_empty()
                && !view.typescript_toolchain_draft.node_executable.is_empty();
            ui.horizontal(|ui| {
                if primary_button_enabled(
                    ui,
                    "Configure toolchain",
                    theme::tokens().accent.blue,
                    ready,
                )
                .clicked()
                    && ready
                {
                    actions.push(DesktopAction::ConfigureTypeScriptToolchain {
                        server_archive: view.typescript_toolchain_draft.server_archive.clone(),
                        compiler_archive: view.typescript_toolchain_draft.compiler_archive.clone(),
                        node_executable: view.typescript_toolchain_draft.node_executable.clone(),
                    });
                }
                if soft_button(ui, "Clear toolchain").clicked() {
                    actions.push(DesktopAction::ClearTypeScriptToolchain);
                }
            });
            if active_file_supports_typescript(snapshot) {
                ui.horizontal(|ui| {
                    if soft_button(ui, "Start language server").clicked() {
                        actions.push(DesktopAction::StartLspSession);
                    }
                    if soft_button(ui, "Restart language server").clicked() {
                        actions.push(DesktopAction::RestartLspSession);
                    }
                });
            } else {
                ui.label(theme::muted(
                    "Start and restart are available when a TypeScript or JavaScript file is active.",
                ));
            }
        }
        if view.settings_section == SettingsSection::AiProviders {
            if snapshot.assisted_ai_projection.providers.is_empty() {
                ui.label(theme::body_strong("No AI provider configured"));
                ui.label(theme::muted(
                    "Choose an AI provider available on this computer or add an Anthropic API key.",
                ));
            } else {
                for provider in snapshot.assisted_ai_projection.providers.iter().take(6) {
                    let display = crate::cut_lines::provider_display_label(
                        &provider.provider_id,
                        &provider.provider_label,
                    );
                    let availability = match provider.availability {
                        AssistedAiProviderAvailabilityState::Available => "Ready",
                        AssistedAiProviderAvailabilityState::Disabled => "Disabled",
                        AssistedAiProviderAvailabilityState::Refused => "Blocked by policy",
                        AssistedAiProviderAvailabilityState::Unavailable => "Unavailable",
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::body_strong(display));
                        ui.separator();
                        ui.label(theme::muted(availability));
                    });
                }
            }
            interactive_fields::render_preferred_provider_picker(
                ui,
                &model.preferred_ai_provider,
                actions,
            );
            interactive_fields::render_anthropic_byok_form(ui, actions);
        }
        if view.settings_section == SettingsSection::Editor {
            let mut line_numbers_visible = model.settings.line_numbers_visible;
            if ui
                .checkbox(&mut line_numbers_visible, "Line numbers")
                .changed()
            {
                actions.push(DesktopAction::SetLineNumbersVisible {
                    visible: line_numbers_visible,
                });
            }
            let mut current_line_highlight = model.settings.current_line_highlight;
            if ui
                .checkbox(&mut current_line_highlight, "Current line highlight")
                .changed()
            {
                actions.push(DesktopAction::SetCurrentLineHighlight {
                    enabled: current_line_highlight,
                });
            }
            let mut sticky_headers_visible = model.settings.sticky_headers_visible;
            if ui
                .checkbox(&mut sticky_headers_visible, "Sticky headers")
                .changed()
            {
                actions.push(DesktopAction::SetStickyHeadersVisible {
                    visible: sticky_headers_visible,
                });
            }
            let mut code_folding_visible = model.settings.code_folding_visible;
            if ui
                .checkbox(&mut code_folding_visible, "Code folding")
                .changed()
            {
                actions.push(DesktopAction::SetCodeFoldingVisible {
                    visible: code_folding_visible,
                });
            }
            let mut minimap_visible = model.settings.minimap_visible;
            if ui.checkbox(&mut minimap_visible, "Minimap").changed() {
                actions.push(DesktopAction::SetMinimapVisible {
                    visible: minimap_visible,
                });
            }
            let mut whitespace_guides_visible = model.settings.whitespace_guides_visible;
            if ui
                .checkbox(&mut whitespace_guides_visible, "Whitespace guides")
                .changed()
            {
                actions.push(DesktopAction::SetWhitespaceGuidesVisible {
                    visible: whitespace_guides_visible,
                });
            }
            let mut indent_guides_visible = model.settings.indent_guides_visible;
            if ui
                .checkbox(&mut indent_guides_visible, "Indent guides")
                .changed()
            {
                actions.push(DesktopAction::SetIndentGuidesVisible {
                    visible: indent_guides_visible,
                });
            }
            let mut smooth_scrolling_enabled = model.settings.smooth_scrolling_enabled;
            if ui
                .checkbox(&mut smooth_scrolling_enabled, "Smooth scrolling")
                .changed()
            {
                actions.push(DesktopAction::SetSmoothScrollingEnabled {
                    enabled: smooth_scrolling_enabled,
                });
            }
            let mut next_edit_prediction_enabled = model.settings.next_edit_prediction_enabled;
            if ui
                .checkbox(&mut next_edit_prediction_enabled, "Next-edit prediction")
                .changed()
            {
                actions.push(DesktopAction::SetNextEditPredictionEnabled {
                    enabled: next_edit_prediction_enabled,
                });
            }
        }
        if view.settings_section == SettingsSection::Privacy {
            let mut crash_reports_enabled = model.settings.crash_reports_enabled;
            if ui
                .checkbox(&mut crash_reports_enabled, "Crash reports")
                .changed()
            {
                actions.push(DesktopAction::SetCrashReportsEnabled {
                    enabled: crash_reports_enabled,
                });
            }
            ui.label(theme::muted(format!(
                "Data sharing: {}",
                model.settings.telemetry_label
            )));
            ui.add_space(8.0);
            if soft_button(ui, "Export support bundle").clicked() {
                actions.push(DesktopAction::ExportSupportBundle);
            }
            ui.label(theme::muted(
                "Writes .legion/support-bundle.md. Metadata only: no editor text, secrets, or raw crash bodies.",
            ));
        }
        if view.settings_section == SettingsSection::Advanced {
            let mut indexed_workspace_search_enabled =
                model.settings.indexed_workspace_search_enabled;
            if ui
                .checkbox(
                    &mut indexed_workspace_search_enabled,
                    "Indexed workspace search",
                )
                .changed()
            {
                actions.push(DesktopAction::SetIndexedWorkspaceSearchEnabled {
                    enabled: indexed_workspace_search_enabled,
                });
            }
            ui.label(theme::muted(
                "Use the workspace index to speed up searches in larger projects.",
            ));
        }
        ui.horizontal(|ui| {
            ui.label(theme::muted(format!(
                "{} theme · {} notifications",
                model.settings.theme_label, model.settings.toast_verbosity_label
            )));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let defaults = soft_button(ui, "Defaults");
                last_focus = Some(defaults.id);
                if defaults.clicked() {
                    actions.push(DesktopAction::ResetSettings);
                }
            });
        });
    });
    last_focus.expect("Settings always renders the Defaults action")
}

fn render_assist_rail(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Assist", DesktopProductMode::Assist);
    // Only the missing-buffer block has a resolution this screen can perform;
    // every other resolution reads as text, the way the Delegate rail already
    // presents one nobody can press.
    if let SurfaceAvailability::Blocked { reason, resolution } = &model.mode_surface.inspector {
        let resolvable_here = snapshot.active_buffer_projection.buffer_id.is_none();
        surface_card(ui, |ui| {
            ui.label(theme::body_strong(reason));
            if resolvable_here {
                if soft_button(ui, resolution).clicked() {
                    actions.push(DesktopAction::OpenPalette {
                        mode: PaletteMode::File,
                        query: String::new(),
                        scope: SearchScopeProjection::Workspace,
                    });
                }
            } else {
                ui.label(theme::muted(resolution));
            }
        });
        return;
    }
    assist_rail_commands::render_assist_rail_commands(
        ui,
        snapshot,
        model.product_ai_stream_in_flight,
        actions,
    );
    // A proposal created here has to be reviewable here. `render_proposal_cards`
    // is a complete Approve/Reject/Cancel surface and was called only from the
    // Workflows rail, so an Assist user could start a proposal and had nowhere
    // to act on it -- the checklist row asks for a proposal to *appear*, not
    // merely to exist in a ledger.
    if !snapshot.proposal_ledger_projection.rows.is_empty() {
        components::section_header(ui, "Proposals", Some(theme::tokens().accent.green));
        render_proposal_cards(ui, snapshot, actions);
    }
    render_assisted_suggestion_panel(ui, snapshot, model, actions);
    ui.add_space(6.0);
    ui.label(theme::muted(
        "Assist never writes to the workspace until you accept a suggestion.",
    ));
    // Route discoverability, without blocking on it. A remote provider is an
    // upgrade, not a prerequisite, and this line is the only place Assist says
    // so. It names the fallback rather than claiming the local route always
    // answers: with a reachable Ollama the preferred route answers, and a
    // sentence saying otherwise would contradict the route named beside it.
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted(format!(
            "Route: {}. Falls back to a local deterministic route if that is unavailable.",
            model.preferred_ai_provider
        )));
        if soft_button(ui, "AI provider settings").clicked() {
            actions.push(DesktopAction::OpenSettings);
            view.utility_surface = Some(UtilitySurface::Settings);
            view.settings_section = SettingsSection::AiProviders;
            view.utility_overlay_needs_focus = true;
            view.utility_overlay_focus_bounds = None;
        }
    });
}

fn render_delegate_prerequisite_rail(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    inspector_header(ui, "Delegate", DesktopProductMode::Delegate);
    if let SurfaceAvailability::Blocked { reason, resolution } = &model.mode_surface.inspector {
        surface_card(ui, |ui| {
            ui.label(theme::body_strong(reason));
            ui.label(theme::muted(resolution));
        });
    }
}

fn render_delegate_draft_rail(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Delegate", DesktopProductMode::Delegate);
    ui.label(theme::muted(
        "Describe a bounded task. Delegate plans and edits in an isolated scope, then stages proposals for review.",
    ));
    ui.add_space(6.0);
    let scope = desktop_default_delegated_scope(state);
    if let Some(task_draft) = interactive_fields::render_delegate_task_draft(ui, scope.is_some())
        && let Some(action) = desktop_delegated_task_action(state, &task_draft)
    {
        actions.push(action);
    }

    render_delegate_chat_section(ui, snapshot, model, actions);

    section_label(ui, "Readiness", Some(theme::tokens().accent.green));
    match &model.mode_surface.inspector {
        SurfaceAvailability::Ready => prerequisite_card(
            ui,
            "Ready to delegate",
            "Task scope and proposal-safe tools are available.",
            true,
        ),
        SurfaceAvailability::Blocked { reason, resolution } => {
            surface_card(ui, |ui| {
                ui.label(theme::body_strong(reason));
                ui.label(theme::muted(resolution));
            });
        }
        SurfaceAvailability::Hidden => {}
    };

    section_label(ui, "Scope", Some(theme::tokens().accent.violet));
    theme::small_card_frame().show(ui, |ui| {
        if let Some(scope) = &scope {
            ui.label(theme::code(trim_middle(&scope.workspace_root.0, 52)));
            ui.label(theme::muted(format!(
                "repository scope · {:?} risk tolerance",
                scope.risk_tolerance
            )));
            ui.label(theme::muted(format!(
                "{} proposal-safe tools",
                scope.allowed_tools.len()
            )));
        } else {
            ui.label(theme::muted("Workspace scope is not available."));
        }
    });

    section_label(ui, "Permission budget", Some(theme::tokens().accent.orange));
    render_delegate_permission_budget(ui, snapshot);

    section_label(ui, "Sandbox", Some(theme::tokens().accent.blue));
    ui.label(theme::muted("Sandbox starts after the task is submitted."));
}

/// Delegate's chat transcript and composer.
///
/// `send_delegate_chat` — retrieval-backed, citation-carrying, and able to
/// stream a live reply — has been implemented in the app since Phase 5 and had
/// no rendered control: `DesktopAction::SendDelegateChat` was pushed by exactly
/// nothing, so the only way to reach it was the `:delegate-chat` shell verb,
/// which the desktop command palette does not offer either. The transcript was
/// equally invisible; chat messages reached the projection and were rendered
/// only as a debug row. Checklist row 7 ("Delegate chat: Streaming… then
/// reply") could not be exercised because there was nowhere to type.
///
/// The composer is shown by both Delegate rails, because chat is useful before
/// a task is submitted as well as during one.
fn render_delegate_chat_section(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    section_label(ui, "Chat", Some(theme::tokens().accent.cyan));
    let messages = &snapshot.delegated_task_projection.chat_messages;
    if messages.is_empty() {
        ui.label(theme::muted("No chat turns yet."));
    } else {
        theme::small_card_frame().show(ui, |ui| {
            let skip = messages.len().saturating_sub(DELEGATE_CHAT_VISIBLE_TURNS);
            for message in messages.iter().skip(skip) {
                let (who, color) = match message.role {
                    legion_protocol::DelegatedTaskChatRole::User => {
                        ("You", theme::tokens().accent.blue)
                    }
                    legion_protocol::DelegatedTaskChatRole::Assistant => {
                        ("Delegate", theme::tokens().accent.cyan)
                    }
                    legion_protocol::DelegatedTaskChatRole::System => {
                        ("System", theme::tokens().accent.violet)
                    }
                };
                ui.label(theme::accent(who, color));
                ui.label(theme::muted(&message.content_label));
                ui.add_space(4.0);
            }
            if skip > 0 {
                ui.label(theme::muted(format!("+{skip} earlier turns")));
            }
        });
        // Where the last turn's request went.
        //
        // The transcript is what somebody reads to decide whether to trust a
        // reply, and it said nothing about the destination -- a Delegate turn
        // could upload a buffer excerpt and leave no reviewer-visible evidence
        // of where. Assist carries that in its proposal; this is the same
        // evidence for the path that has no proposal.
        if let Some(route) = snapshot.delegated_task_projection.provider_routes.last() {
            ui.add_space(4.0);
            ui.label(theme::muted(format!(
                "Route: {} ({}) · {} · {} · {:?}",
                route.provider_id,
                route.model_label,
                route.destination_label,
                route.egress_label,
                route.invocation_state
            )));
        }
    }
    // The app requires an active buffer to build chat context, so say that
    // instead of offering a Send that would only return an error.
    let has_buffer = snapshot.active_buffer_projection.buffer_id.is_some();
    if model.product_ai_stream_in_flight {
        ui.label(theme::accent("Streaming…", theme::tokens().accent.amber));
    }
    if let Some(prompt) = interactive_fields::render_delegate_chat_draft(
        ui,
        has_buffer && !model.product_ai_stream_in_flight,
    ) {
        actions.push(DesktopAction::SendDelegateChat {
            prompt_label: prompt,
        });
    }
    if !has_buffer {
        ui.label(theme::muted(
            "Open a file to give Delegate something to talk about.",
        ));
    }
}

fn render_delegation_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Delegate", DesktopProductMode::Delegate);
    if let SurfaceAvailability::Blocked { reason, resolution } = &model.mode_surface.inspector {
        surface_card(ui, |ui| {
            ui.label(theme::body_strong(reason));
            ui.label(theme::muted(resolution));
        });
        // Chat survives the block. `send_delegate_chat` needs Delegate mode and
        // an open buffer, neither of which a blocked *task* affects -- so the
        // early return took away the one surface that still worked, exactly
        // when a user asking "why is this blocked?" would reach for it.
        render_delegate_chat_section(ui, snapshot, model, actions);
        return;
    }
    let lifecycle = model
        .mode_surface
        .delegate_lifecycle
        .unwrap_or(DelegateLifecycle::Draft);
    section_label(ui, "Task intent", Some(theme::tokens().accent.blue));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(current_objective(snapshot)));
    });
    render_delegate_chat_section(ui, snapshot, model, actions);
    section_label(ui, "Readiness", Some(theme::tokens().accent.green));
    let (readiness, detail, ready) = match lifecycle {
        DelegateLifecycle::Draft => (
            "Task draft projected",
            "Review the projected task metadata before runtime work begins.",
            false,
        ),
        DelegateLifecycle::Running => (
            "Task is active",
            "Changes remain proposals until you approve them.",
            true,
        ),
        DelegateLifecycle::Waiting => (
            "Task is waiting",
            "Review the requested approval or prerequisite to continue.",
            false,
        ),
        DelegateLifecycle::Terminal => {
            ("Task ended", "No further delegated work is running.", false)
        }
    };
    prerequisite_card(ui, readiness, detail, ready);
    section_label(ui, "Phase", Some(theme::tokens().accent.violet));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(delegated_runtime_label(
            snapshot.delegated_task_projection.runtime_activation,
        )));
        ui.label(theme::muted(format!(
            "{} plans · {} blocked · {} refused",
            snapshot.delegated_task_projection.plan_count,
            snapshot.delegated_task_projection.blocked_plan_count,
            snapshot.delegated_task_projection.refused_plan_count
        )));
        if delegated_runtime_is_cancellable(snapshot.delegated_task_projection.runtime_activation)
            && primary_button(ui, "Cancel task", theme::tokens().accent.red).clicked()
        {
            actions.push(DesktopAction::CancelDelegatedTask);
        }
    });

    if !snapshot
        .delegated_task_projection
        .tool_permission_requests
        .is_empty()
    {
        section_label(ui, "Permissions", Some(theme::tokens().accent.red));
        render_delegated_tool_permission_controls(ui, snapshot, actions);
    }

    section_label(
        ui,
        "Task graph and evidence",
        Some(theme::tokens().accent.blue),
    );
    let panel_vm = worker_panel::DesktopWorkerPanelViewModel::from_snapshot(snapshot);
    let evidence = proposal_review::DesktopProposalEvidencePanelViewModel::default();
    worker_panel::render_worker_panel(ui, &panel_vm, &evidence, actions);

    section_label(ui, "Scope", Some(theme::tokens().accent.violet));
    if let Some(scope) = desktop_default_delegated_scope(state) {
        ui.label(theme::code(trim_middle(&scope.workspace_root.0, 52)));
    } else {
        ui.label(theme::muted("Scope is locked by the active task."));
    }
    section_label(ui, "Permission budget", Some(theme::tokens().accent.orange));
    render_delegate_permission_budget(ui, snapshot);
    section_label(ui, "Sandbox", Some(theme::tokens().accent.blue));
    render_compact_rows(
        ui,
        &model.sandbox_rows,
        "Sandbox is preparing",
        sandbox_panel::PANEL_VISIBLE_ROW_LIMIT,
    );
}

fn render_delegate_permission_budget(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let budget = &snapshot.permission_budget_projection;
    if budget.budgets.is_empty() && budget.evaluations.is_empty() {
        ui.label(theme::muted("No extra permissions requested."));
        return;
    }
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(format!(
            "{} permission limit{}",
            budget.budgets.len(),
            if budget.budgets.len() == 1 { "" } else { "s" }
        )));
        ui.label(theme::muted(format!(
            "{} checks · {} denied · {} depleted",
            budget.evaluations.len(),
            budget.denied_budget_count,
            budget.depleted_budget_count
        )));
    });
}

fn delegated_runtime_label(
    activation: legion_protocol::DelegatedTaskRuntimeActivationState,
) -> &'static str {
    use legion_protocol::DelegatedTaskRuntimeActivationState as State;
    match activation {
        State::NotEncoded | State::Planned => "Preparing task",
        State::SandboxAllocated => "Sandbox ready",
        State::Executing => "Running",
        State::Verifying => "Verifying",
        State::WaitingForApproval => "Waiting for approval",
        State::Blocked => "Blocked",
        State::Completed => "Completed",
        State::Cancelled => "Cancelled",
        State::Failed => "Failed",
    }
}

fn legion_workflow_lifecycle_label(state: legion_protocol::LegionWorkflowState) -> &'static str {
    use legion_protocol::LegionWorkflowState as State;
    match state {
        State::Draft => "Draft",
        State::Planning => "Planning",
        State::Executing => "Running",
        State::Verifying => "Verifying",
        State::WaitingForApproval => "Waiting for approval",
        State::WaitingOnHuman => "Waiting for input",
        State::Blocked => "Blocked",
        State::Completed => "Completed",
        State::Failed => "Failed",
        State::Cancelled => "Cancelled",
    }
}

fn legion_workflow_merge_label(
    state: legion_protocol::LegionWorkflowMergeReadinessState,
) -> &'static str {
    use legion_protocol::LegionWorkflowMergeReadinessState as State;
    match state {
        State::WaitingForApproval => "Waiting for approval",
        State::Ready => "Ready for review",
        State::Blocked => "Blocked",
    }
}

fn legion_workflow_risk_state_label(
    state: legion_protocol::LegionWorkflowRiskMonitorState,
) -> &'static str {
    use legion_protocol::LegionWorkflowRiskMonitorState as State;
    match state {
        State::Nominal => "Within limits",
        State::Warning => "Approaching limit",
        State::Halted => "Work stopped",
    }
}

fn proposal_risk_label(risk: ProposalRiskLabel) -> &'static str {
    match risk {
        ProposalRiskLabel::Informational => "Informational",
        ProposalRiskLabel::Low => "Low",
        ProposalRiskLabel::Medium => "Medium",
        ProposalRiskLabel::High => "High",
        ProposalRiskLabel::Unknown => "Unknown",
    }
}

fn render_fleet_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Legion Workflows", DesktopProductMode::LegionWorkflows);
    // Cloud Lane rides here rather than in its own dock panel: cloud
    // submission requires Automate mode in app authority, and this rail is
    // only reached in that mode. Manual renders nothing in the right dock at
    // all, which is the ban ADR-0046 and the Manual capability suite both want.
    if model.cloud_lane.runtime_enabled || !model.cloud_lane.is_empty() {
        components::section_header(ui, "Cloud Lane", Some(theme::tokens().accent.orange));
        cloud_lane::render_cloud_lane_panel(ui, &model.cloud_lane, actions);
        ui.separator();
    }
    let workflows = &snapshot.legion_workflow_projection;
    if workflows.rows.is_empty() {
        ui.add_space(32.0);
        empty_state(
            ui,
            "No workflow sessions yet",
            "Start a workflow to see its progress here.",
        );
        return;
    } else {
        section_label(ui, "Workflow sessions", Some(theme::tokens().accent.purple));
        for (index, row) in workflows.rows.iter().take(4).enumerate() {
            theme::small_card_frame().show(ui, |ui| {
                ui.label(theme::body_strong(format!(
                    "Workflow session {}",
                    index + 1
                )));
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::accent(
                        legion_workflow_lifecycle_label(row.lifecycle_state),
                        theme::tokens().accent.blue,
                    ));
                    ui.separator();
                    ui.label(theme::muted(format!("{} workers", row.worker_count)));
                    ui.separator();
                    ui.label(theme::muted(format!(
                        "verification {}/{}",
                        row.passed_verification_count, row.verification_gate_count
                    )));
                });
                ui.label(theme::muted(format!(
                    "Sign-off {} of {} · Conflicts {} · Merge: {}",
                    row.signed_off_count,
                    row.sign_off_count,
                    row.unresolved_conflict_count,
                    legion_workflow_merge_label(row.merge_readiness.state)
                )));
            });
        }

        section_label(ui, "Selected task", Some(theme::tokens().accent.blue));
        fleet_card::render_fleet_cards(ui, &snapshot.legion_workflow_fleet_card_projections);

        if !snapshot.proposal_ledger_projection.rows.is_empty() {
            section_label(ui, "Approvals", Some(theme::tokens().accent.orange));
            render_proposal_cards(ui, snapshot, actions);
        }
        if !workflows.tool_permission_requests.is_empty() {
            section_label(ui, "Permissions", Some(theme::tokens().accent.orange));
            render_legion_workflow_tool_permission_controls(ui, snapshot, actions);
        }
        let stoppable_workflows = visible_stoppable_legion_workflows(snapshot);
        if !stoppable_workflows.is_empty() {
            section_label(ui, "Stop controls", Some(theme::tokens().accent.red));
            render_legion_workflow_kill_switch_controls(ui, &stoppable_workflows, actions);
        }

        let worker_panel = worker_panel::DesktopWorkerPanelViewModel::from_snapshot(snapshot);
        if !worker_panel.recovery_actions.is_empty() {
            section_label(ui, "Gated recovery", Some(theme::tokens().accent.orange));
            worker_panel::render_worker_recovery_actions(ui, &worker_panel, actions);
        }
    }

    if !snapshot.legion_workflow_budget_rows.is_empty() {
        section_label(ui, "Resource budgets", Some(theme::tokens().accent.green));
        render_legion_workflow_budget_rows(ui, &snapshot.legion_workflow_budget_rows);
    }
    if !workflows.risk_monitors.is_empty() {
        section_label(ui, "Risk gate", Some(theme::tokens().accent.red));
        for monitor in workflows.risk_monitors.iter().take(3) {
            theme::small_card_frame().show(ui, |ui| {
                let workflow_label = legion_workflow_ordinal(snapshot, &monitor.session_id)
                    .map_or_else(
                        || "Unlisted workflow".to_string(),
                        |ordinal| format!("Workflow {ordinal}"),
                    );
                ui.label(theme::body_strong(format!("{workflow_label} risk")));
                ui.label(theme::accent(
                    format!(
                        "{} · Risk score {} of {} · High-risk actions {} · Denied tools {}",
                        legion_workflow_risk_state_label(monitor.state),
                        monitor.risk_score,
                        monitor.halt_threshold,
                        monitor.high_risk_action_count,
                        monitor.denied_tool_count
                    ),
                    if monitor.state == legion_protocol::LegionWorkflowRiskMonitorState::Halted {
                        theme::tokens().accent.red
                    } else {
                        theme::tokens().accent.green
                    },
                ));
            });
        }
    }

    if !workflows.decision_feed.is_empty() {
        section_label(ui, "Decision feed", None);
        for decision in workflows.decision_feed.iter().take(6) {
            ui.label(theme::muted(&decision.summary_label));
        }
    }
    if !snapshot.legion_workflow_comm_rows.is_empty() {
        section_label(ui, "Agent communication", Some(theme::tokens().accent.cyan));
        agent_comm::render_agent_comm_rows(
            ui,
            &snapshot.legion_workflow_comm_rows,
            "No agent communication yet",
        );
    }
}

fn render_legion_workflow_budget_rows(
    ui: &mut egui::Ui,
    rows: &[legion_ui::LegionWorkflowBudgetUsageRowProjection],
) {
    if rows.is_empty() {
        return;
    }
    for (index, row) in rows.iter().take(6).enumerate() {
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(format!("Worker budget {}", index + 1)));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(user_facing_protocol_label(&row.budget_label)));
                ui.separator();
                ui.label(theme::accent(
                    user_facing_protocol_label(&row.status_label),
                    if row.status_label == "within-budget" {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                ));
            });
            ui.label(theme::muted(workflow_budget_usage_label(
                &row.model_turns_label,
                "model_turns=",
                "Model turns",
                "",
            )));
            ui.label(theme::muted(workflow_budget_usage_label(
                &row.tool_calls_label,
                "tool_calls=",
                "Tool calls",
                "",
            )));
            ui.label(theme::muted(workflow_budget_usage_label(
                &row.retry_label,
                "retries=",
                "Retries",
                "",
            )));
            ui.label(theme::muted(workflow_budget_usage_label(
                &row.output_bytes_label,
                "output_bytes=",
                "Output",
                " bytes",
            )));
            ui.label(theme::muted(workflow_budget_usage_label(
                &row.wall_clock_label,
                "wall_clock=",
                "Time",
                " ms",
            )));
        });
    }
}

fn workflow_budget_usage_label(raw: &str, prefix: &str, title: &str, unit: &str) -> String {
    let parsed = raw
        .strip_prefix(prefix)
        .and_then(|usage| usage.split_once('/'))
        .map(|(used, limit)| {
            let limit = if unit == " ms" {
                limit.strip_suffix("ms").unwrap_or(limit)
            } else {
                limit
            };
            (used, limit)
        })
        .filter(|(used, limit)| !used.is_empty() && !limit.is_empty());
    match parsed {
        Some((used, limit)) => format!("{title} {used} of {limit}{unit}"),
        None => user_facing_protocol_label(raw),
    }
}

fn user_facing_protocol_label(raw: &str) -> String {
    let mut label = raw.replace(['_', '-'], " ").replace('=', ":");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}

fn render_delegated_hunk_review_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let owned = delegate_owned_proposal_ids(snapshot);
    let reviews = snapshot
        .delegated_task_projection
        .proposal_reviews
        .iter()
        .filter(|review| owned.contains(&review.proposal_id))
        .collect::<Vec<_>>();
    if reviews.is_empty() {
        return;
    }
    section_label(ui, "Hunk Review", Some(theme::tokens().accent.violet));
    // Make every review and hunk reachable via scrolling rather than silently
    // truncating, so hidden reviews/hunks can still be accepted/rejected/pending.
    egui::ScrollArea::vertical()
        .id_salt("delegated_hunk_review_scroll")
        .max_height(280.0)
        .show(ui, |ui| {
            for review in reviews {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(format!(
                        "proposal {} accepted={} rejected={} pending={}",
                        review.proposal_id.0,
                        review.accepted_hunk_count,
                        review.rejected_hunk_count,
                        review.pending_hunk_count
                    )));
                    for hunk in review.hunks.iter() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(theme::code_muted(trim_middle(&hunk.hunk_id, 36)));
                            ui.label(theme::muted(format!("{:?}", hunk.disposition)));
                            if soft_button(ui, "Accept").clicked() {
                                actions.push(DesktopAction::ReviewDelegateProposalHunk {
                                    proposal_id: review.proposal_id,
                                    hunk_id: hunk.hunk_id.clone(),
                                    disposition: DelegatedTaskProposalHunkDisposition::Accepted,
                                });
                            }
                            if soft_button(ui, "Reject").clicked() {
                                actions.push(DesktopAction::ReviewDelegateProposalHunk {
                                    proposal_id: review.proposal_id,
                                    hunk_id: hunk.hunk_id.clone(),
                                    disposition: DelegatedTaskProposalHunkDisposition::Rejected,
                                });
                            }
                            if soft_button(ui, "Pending").clicked() {
                                actions.push(DesktopAction::ReviewDelegateProposalHunk {
                                    proposal_id: review.proposal_id,
                                    hunk_id: hunk.hunk_id.clone(),
                                    disposition: DelegatedTaskProposalHunkDisposition::Pending,
                                });
                            }
                        });
                    }
                });
            }
        });
}

fn render_delegated_tool_permission_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let requests = &snapshot.delegated_task_projection.tool_permission_requests;
    if requests.is_empty() {
        ui.label(theme::muted("No Delegate tool permissions"));
        return;
    }
    for request in requests.iter().take(6) {
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(trim_middle(&request.request_id, 56)));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!("{:?}", request.profile)));
                ui.separator();
                ui.label(theme::muted(format!("{:?}", request.action_class)));
                ui.separator();
                ui.label(theme::accent(
                    format!("{:?}", request.disposition),
                    if request.deny_overrides {
                        theme::tokens().accent.red
                    } else if request.runtime_allowed {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                ));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, decision) in [
                    ("Confirm", DelegatedTaskToolPermissionDecision::Confirm),
                    ("Allow", DelegatedTaskToolPermissionDecision::Allow),
                    ("Deny", DelegatedTaskToolPermissionDecision::Deny),
                    ("Always", DelegatedTaskToolPermissionDecision::Always),
                ] {
                    if soft_button(ui, label).clicked() {
                        actions.push(DesktopAction::RecordDelegateToolPermission {
                            request_id: request.request_id.clone(),
                            decision,
                        });
                    }
                }
            });
        });
    }
}

fn render_legion_workflow_tool_permission_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let requests = &snapshot.legion_workflow_projection.tool_permission_requests;
    if requests.is_empty() {
        ui.label(theme::muted("No Legion Workflows MCP tool permissions"));
        return;
    }
    for (index, request) in requests.iter().take(6).enumerate() {
        let Some((server_id, tool_name)) = parse_automate_tool_target(request.target_id.as_deref())
        else {
            continue;
        };
        let Some(session_id) = parse_automate_permission_session(request) else {
            continue;
        };
        theme::small_card_frame().show(ui, |ui| {
            let workflow_label = legion_workflow_ordinal(snapshot, &session_id).map_or_else(
                || "Unlisted workflow".to_string(),
                |ordinal| format!("Workflow {ordinal}"),
            );
            ui.label(theme::body_strong(format!(
                "{workflow_label} · Tool permission request {}",
                index + 1
            )));
            ui.label(theme::body(format!(
                "Target: {} · {}",
                workflow_permission_server_label(snapshot, &server_id),
                tool_name.0
            )));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(workflow_permission_profile_label(
                    request.profile,
                )));
                ui.separator();
                ui.label(theme::muted(workflow_permission_action_label(
                    request.action_class,
                )));
                ui.separator();
                ui.label(theme::accent(
                    workflow_permission_disposition_label(request.disposition),
                    if request.deny_overrides {
                        theme::tokens().accent.red
                    } else if request.runtime_allowed {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                ));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, decision) in [
                    ("Confirm", DelegatedTaskToolPermissionDecision::Confirm),
                    ("Allow", DelegatedTaskToolPermissionDecision::Allow),
                    ("Deny", DelegatedTaskToolPermissionDecision::Deny),
                    ("Always", DelegatedTaskToolPermissionDecision::Always),
                ] {
                    if soft_button(ui, label).clicked() {
                        actions.push(DesktopAction::RecordLegionWorkflowToolPermission {
                            session_id: session_id.clone(),
                            server_id: server_id.clone(),
                            tool_name: tool_name.clone(),
                            decision,
                        });
                    }
                }
            });
        });
    }
}

fn workflow_permission_profile_label(
    profile: legion_protocol::DelegatedTaskToolPermissionProfile,
) -> &'static str {
    use legion_protocol::DelegatedTaskToolPermissionProfile as Profile;
    match profile {
        Profile::Ask => "Ask each time",
        Profile::Write => "Changes workspace",
    }
}

fn workflow_permission_action_label(
    action: legion_protocol::PermissionBudgetActionClass,
) -> &'static str {
    use legion_protocol::PermissionBudgetActionClass as Action;
    match action {
        Action::ReadContext => "Reads task context",
        Action::ReadSemanticMetadata => "Reads code metadata",
        Action::InvokeLocalTool => "Uses a local tool",
        Action::InvokeProvider => "Uses an AI provider",
        Action::ProposeEdits => "Proposes edits",
        Action::ApplyApprovedProposal => "Applies an approved proposal",
        Action::AccessNetwork => "Uses the network",
        Action::AccessTerminal => "Uses the terminal",
        Action::AccessWorkspaceFiles => "Accesses workspace files",
        Action::RetainMemory => "Retains workspace memory",
    }
}

fn workflow_permission_disposition_label(
    disposition: legion_protocol::DelegatedTaskToolPermissionDisposition,
) -> &'static str {
    use legion_protocol::DelegatedTaskToolPermissionDisposition as Disposition;
    match disposition {
        Disposition::WaitingForConfirmation => "Waiting for confirmation",
        Disposition::AllowedOnce => "Allowed once",
        Disposition::AlwaysAllowed => "Always allowed",
        Disposition::Denied => "Denied",
    }
}

fn parse_automate_permission_session(
    request: &legion_protocol::DelegatedTaskToolPermissionRequest,
) -> Option<legion_protocol::LegionWorkflowSessionId> {
    request.labels.iter().find_map(|label| {
        label
            .strip_prefix("legion.session:")
            .filter(|session_id| !session_id.trim().is_empty())
            .map(|session_id| legion_protocol::LegionWorkflowSessionId(session_id.to_string()))
    })
}

fn legion_workflow_ordinal(
    snapshot: &ShellProjectionSnapshot,
    session_id: &legion_protocol::LegionWorkflowSessionId,
) -> Option<usize> {
    snapshot
        .legion_workflow_projection
        .rows
        .iter()
        .position(|row| &row.session_id == session_id)
        .map(|index| index + 1)
}

fn workflow_permission_server_label(
    snapshot: &ShellProjectionSnapshot,
    server_id: &legion_protocol::McpServerId,
) -> String {
    snapshot
        .legion_workflow_projection
        .mcp_registries
        .iter()
        .find(|registry| &registry.server.server_id == server_id)
        .map(|registry| registry.server.display_label.trim())
        .filter(|label| !label.is_empty())
        .unwrap_or("Unregistered MCP server")
        .to_string()
}

fn render_legion_workflow_kill_switch_controls(
    ui: &mut egui::Ui,
    rows: &[(usize, &legion_protocol::LegionWorkflowProjectionRow)],
    actions: &mut Vec<DesktopAction>,
) {
    for (row_index, row) in rows {
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(format!("Workflow {} stop", row_index + 1)));
            let stop = soft_button(ui, "Kill");
            ui.ctx().accesskit_node_builder(stop.id, |node| {
                node.set_label(format!("Stop workflow session {}", row_index + 1));
            });
            if stop.clicked() {
                actions.push(DesktopAction::TriggerLegionWorkflowKillSwitch {
                    session_id: row.session_id.clone(),
                    reason_label: "user requested hard stop".to_string(),
                });
            }
        });
    }
}

fn visible_stoppable_legion_workflows(
    snapshot: &ShellProjectionSnapshot,
) -> Vec<(usize, &legion_protocol::LegionWorkflowProjectionRow)> {
    snapshot
        .legion_workflow_projection
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            legion_workflow_is_cancellable(row.lifecycle_state)
                && snapshot
                    .legion_workflow_projection
                    .kill_switches
                    .iter()
                    .any(|switch| {
                        switch.session_id == row.session_id
                            && switch.state == legion_protocol::LegionWorkflowKillSwitchState::Armed
                    })
        })
        .take(3)
        .collect()
}

fn legion_workflow_is_cancellable(state: legion_protocol::LegionWorkflowState) -> bool {
    !matches!(
        state,
        legion_protocol::LegionWorkflowState::Completed
            | legion_protocol::LegionWorkflowState::Failed
            | legion_protocol::LegionWorkflowState::Cancelled
    )
}

fn parse_automate_tool_target(
    target_id: Option<&str>,
) -> Option<(legion_protocol::McpServerId, legion_protocol::McpToolName)> {
    let target_id = target_id?;
    let rest = target_id.strip_prefix("mcp-tool:")?;
    let (server_id, tool_name) = rest.split_once('|')?;
    if server_id.trim().is_empty() || tool_name.trim().is_empty() {
        return None;
    }
    Some((
        legion_protocol::McpServerId(server_id.to_string()),
        legion_protocol::McpToolName(tool_name.to_string()),
    ))
}

/// Builds the repository-scoped default used by the Delegate task control.
///
/// The root comes only from the runtime-owned projection state. The renderer
/// deliberately performs no filesystem probing or nearest-manifest inference.
pub fn desktop_default_delegated_scope(
    state: &DesktopProjectionViewState,
) -> Option<DelegatedTaskScope> {
    let root = state.canonical_workspace_root.clone()?;
    Some(DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: root,
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
        ],
        forbidden_paths: vec![],
        schema_version: 1,
    })
}

/// Converts a non-empty renderer-local Delegate draft into the existing
/// proposal-mediated delegated-task action using scope derived from the current
/// projection. Empty drafts emit nothing.
pub fn desktop_delegated_task_action(
    state: &DesktopProjectionViewState,
    task_draft: &str,
) -> Option<DesktopAction> {
    let task_description = interactive_fields::bounded_delegate_task_draft(task_draft);
    let task_description = task_description.trim();
    if task_description.is_empty() {
        return None;
    }
    let scope = desktop_default_delegated_scope(state)?;
    Some(DesktopAction::StartDelegatedTask {
        task_description: task_description.to_string(),
        scope,
    })
}

fn render_terminal_stream(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    let terminal = &snapshot.terminal_panel_projection;
    let render_model = terminal_panel::TerminalPanelRenderModel::from_projection(terminal, 100);
    section_label(ui, "Terminal / Runtime", Some(theme::tokens().accent.cyan));
    theme::code_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                // The render model's `key=value` labels stay as they are —
                // evidence tests assert them exactly — but the panel shows the
                // readable form. `status=disabled visible=0 omitted=0
                // matches=0` across the top of an idle terminal was four facts
                // nobody asked for, three of which were zero.
                ui.label(theme::body(terminal.status.kind.display_label()));
                if let Some(summary) = terminal_panel::scrollback_summary(terminal) {
                    ui.label(theme::muted(summary));
                }
                if render_model.scrollback_truncated {
                    ui.label(theme::code_muted("scrollback truncated"));
                }
                if render_model.search_truncated {
                    ui.label(theme::code_muted("search truncated"));
                }
            });
            ui.label(theme::body(trim_middle(&terminal.status.message, 140)));
            if let Some(policy) = &terminal.policy {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::code_muted(format!(
                        "policy capability={} trust={:?} granted={} timeout={}s",
                        policy.capability_id.0,
                        policy.workspace_trust_state,
                        policy.granted,
                        policy.timeout_seconds
                    )));
                    if let Some(decision_id) = policy.decision_id {
                        ui.label(theme::code_muted(format!("decision={}", decision_id.0)));
                    }
                });
                ui.label(theme::code_muted(trim_middle(&policy.reason, 140)));
            }
            if let Some(denial) = &terminal.last_denial {
                ui.label(theme::accent(
                    trim_middle(format!("denial: {denial}").as_str(), 140),
                    theme::tokens().accent.orange,
                ));
            }
            if let Some(error) = &terminal.last_error {
                ui.label(theme::accent(
                    trim_middle(format!("error: {error}").as_str(), 140),
                    theme::tokens().accent.red,
                ));
            }
            ui.add_space(theme::tokens().spacing.sm as f32);
            if terminal.active_session_id.is_none() && soft_button(ui, "Open terminal").clicked() {
                // Launch the shell only. The command label is audit/status metadata;
                // no task command is injected into the newly opened PTY.
                actions.push(DesktopAction::TerminalLaunch {
                    command_label: "interactive shell".to_string(),
                });
            }
            // Tier 1 A8: interactive input line — sends TerminalInput on Enter.
            if terminal.active_session_id.is_some() {
                interactive_fields::render_terminal_input_line(
                    ui,
                    actions,
                    terminal.application_cursor_keys.unwrap_or(false),
                );
                ui.horizontal(|ui| {
                    if soft_button(ui, "Poll").clicked() {
                        actions.push(DesktopAction::TerminalOutputPoll);
                    }
                    if soft_button(ui, "Kill").clicked() {
                        actions.push(DesktopAction::TerminalKill);
                    }
                    if soft_button(ui, "Close").clicked() {
                        actions.push(DesktopAction::TerminalClose);
                    }
                });
                ui.add_space(theme::tokens().spacing.sm as f32);
            }
            if terminal.output_rows.is_empty() {
                if projected_product_mode(snapshot) == DesktopProductMode::Manual {
                    ui.label(theme::muted("No terminal activity"));
                } else {
                    render_compact_rows(ui, &model.bottom_console_rows, "No terminal activity", 4);
                }
                return;
            }

            // Cell grid path: when the VT100 emulator has produced a cell grid,
            // render with per-cell colors. Otherwise fall back to text rows.
            if let Some(cell_grid) = &render_model.grid.cell_grid {
                let cell_scrollback = render_model.grid.cell_scrollback.as_deref().unwrap_or(&[]);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(280.0)
                    .show(ui, |ui| {
                        if !cell_scrollback.is_empty() {
                            render_terminal_cell_grid(ui, cell_scrollback, None, None, Some(false));
                            ui.separator();
                        }
                        render_terminal_cell_grid(
                            ui,
                            cell_grid,
                            render_model.grid.cursor_row,
                            render_model.grid.cursor_col,
                            render_model.grid.cursor_visible,
                        );
                    });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(280.0)
                    .show(ui, |ui| {
                        egui::Grid::new("terminal-output-grid")
                            .num_columns(4)
                            .striped(true)
                            .spacing([theme::tokens().spacing.sm as f32, 2.0])
                            .show(ui, |ui| {
                                for row in render_model.grid.rows.iter() {
                                    ui.label(theme::code_muted(row.sequence_label.clone()));
                                    ui.label(theme::code_muted(row.stream_label.clone()));
                                    ui.horizontal_wrapped(|ui| {
                                        render_terminal_payload(ui, &row.payload);
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        for badge in &row.badges {
                                            ui.label(theme::code_muted(badge.clone()));
                                        }
                                        if ui.small_button("Copy").clicked()
                                            && let Some(payload) =
                                                render_model.copy_row(row.sequence)
                                        {
                                            ui.ctx().copy_text(payload);
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
            }
        });
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalTextSegment {
    Text(String),
    Url(String),
}

fn terminal_text_segments(text: &str) -> Vec<TerminalTextSegment> {
    const URL_PREFIXES: [&str; 2] = ["https://", "http://"];
    let mut segments = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let remaining = &text[cursor..];
        let next_url = URL_PREFIXES
            .iter()
            .filter_map(|prefix| remaining.find(prefix).map(|offset| cursor + offset))
            .min();

        let Some(url_start) = next_url else {
            if cursor < text.len() {
                segments.push(TerminalTextSegment::Text(text[cursor..].to_string()));
            }
            break;
        };

        if url_start > cursor {
            segments.push(TerminalTextSegment::Text(
                text[cursor..url_start].to_string(),
            ));
        }

        let mut url_end = url_start;
        while url_end < text.len() {
            let Some(ch) = text[url_end..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            url_end += ch.len_utf8();
        }

        while url_end > url_start {
            let Some(ch) = text[url_start..url_end].chars().next_back() else {
                break;
            };
            if matches!(
                ch,
                '.' | ',' | ';' | ':' | '!' | ')' | ']' | '}' | '>' | '"' | '\''
            ) {
                url_end -= ch.len_utf8();
            } else {
                break;
            }
        }

        if url_end > url_start {
            segments.push(TerminalTextSegment::Url(
                text[url_start..url_end].to_string(),
            ));
        }
        cursor = url_end.max(url_start + 1);
    }

    segments
}

fn render_terminal_payload(ui: &mut egui::Ui, payload: &str) {
    let segments = terminal_text_segments(payload);
    if segments.is_empty() {
        ui.label(theme::code(payload));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for segment in segments {
            match segment {
                TerminalTextSegment::Text(text) => {
                    if !text.is_empty() {
                        ui.label(theme::code(text));
                    }
                }
                TerminalTextSegment::Url(url) => {
                    ui.hyperlink_to(theme::code(url.clone()), url);
                }
            }
        }
    });
}

/// Standard 16-color ANSI palette (indices 0-15).
const ANSI_16_COLORS: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0: Black
    (205, 0, 0),     // 1: Red
    (0, 205, 0),     // 2: Green
    (205, 205, 0),   // 3: Yellow
    (0, 0, 238),     // 4: Blue
    (205, 0, 205),   // 5: Magenta
    (0, 205, 205),   // 6: Cyan
    (229, 229, 229), // 7: White
    (128, 128, 128), // 8: Bright Black
    (255, 0, 0),     // 9: Bright Red
    (0, 255, 0),     // 10: Bright Green
    (255, 255, 0),   // 11: Bright Yellow
    (92, 92, 255),   // 12: Bright Blue
    (255, 0, 255),   // 13: Bright Magenta
    (0, 255, 255),   // 14: Bright Cyan
    (255, 255, 255), // 15: Bright White
];

/// Resolve a protocol `TerminalColor` to an egui `Color32`.
fn resolve_terminal_color(
    color: &legion_protocol::TerminalColor,
    is_foreground: bool,
) -> egui::Color32 {
    match color {
        legion_protocol::TerminalColor::Default => {
            if is_foreground {
                theme::tokens().text.secondary
            } else {
                egui::Color32::TRANSPARENT
            }
        }
        legion_protocol::TerminalColor::Indexed(n) => {
            let n = *n;
            if n < 16 {
                let (r, g, b) = ANSI_16_COLORS[n as usize];
                egui::Color32::from_rgb(r, g, b)
            } else if n < 232 {
                // 6x6x6 color cube: indices 16-231
                let idx = n - 16;
                let r_idx = idx / 36;
                let g_idx = (idx % 36) / 6;
                let b_idx = idx % 6;
                let r = if r_idx == 0 { 0 } else { 55 + 40 * r_idx };
                let g = if g_idx == 0 { 0 } else { 55 + 40 * g_idx };
                let b = if b_idx == 0 { 0 } else { 55 + 40 * b_idx };
                egui::Color32::from_rgb(r, g, b)
            } else {
                // Grayscale: indices 232-255
                let gray = 8 + 10 * (n - 232);
                egui::Color32::from_rgb(gray, gray, gray)
            }
        }
        legion_protocol::TerminalColor::Rgb(r, g, b) => egui::Color32::from_rgb(*r, *g, *b),
    }
}

/// Render a VT100 cell grid with per-cell colors using egui LayoutJob.
///
/// Falls back to existing text-row rendering when no cell grid is available.
fn render_terminal_cell_grid(
    ui: &mut egui::Ui,
    cell_grid: &[legion_protocol::TerminalCellRow],
    cursor_row: Option<usize>,
    cursor_col: Option<usize>,
    cursor_visible: Option<bool>,
) {
    let font_size = theme::tokens().typography.code as f32;
    let show_cursor = cursor_visible.unwrap_or(true);
    let cursor_pos = if show_cursor {
        cursor_row.zip(cursor_col)
    } else {
        None
    };

    for (row_idx, cell_row) in cell_grid.iter().enumerate() {
        let mut job = egui::text::LayoutJob::default();
        for (col_idx, cell) in cell_row.cells.iter().enumerate() {
            let is_cursor = cursor_pos == Some((row_idx, col_idx));
            let mut fg = resolve_terminal_color(&cell.attrs.fg, true);
            let mut bg = resolve_terminal_color(&cell.attrs.bg, false);
            if cell.attrs.inverse {
                std::mem::swap(&mut fg, &mut bg);
                if bg == egui::Color32::TRANSPARENT {
                    bg = theme::tokens().text.secondary;
                }
            }

            let mut text_format = egui::TextFormat {
                font_id: egui::FontId::monospace(font_size),
                color: fg,
                background: bg,
                ..Default::default()
            };
            if cell.attrs.bold {
                text_format.font_id = egui::FontId::new(font_size, egui::FontFamily::Monospace);
            }
            if cell.attrs.italic {
                text_format.italics = true;
            }
            if cell.attrs.underline {
                text_format.underline = egui::Stroke::new(1.0_f32, fg);
            }
            if cell.attrs.strikethrough {
                text_format.strikethrough = egui::Stroke::new(1.0_f32, fg);
            }
            if cell.attrs.hidden {
                text_format.color = egui::Color32::TRANSPARENT;
            }
            if is_cursor {
                let cursor_bg = if fg == egui::Color32::TRANSPARENT {
                    theme::tokens().text.primary
                } else {
                    fg
                };
                let cursor_fg = if bg == egui::Color32::TRANSPARENT {
                    theme::tokens().bg.code
                } else {
                    bg
                };
                text_format.background = cursor_bg;
                text_format.color = cursor_fg;
            }

            if cell.continuation && !is_cursor {
                continue;
            }
            let text = if cell.attrs.hidden || cell.continuation {
                " ".to_string()
            } else if cell.combining.is_empty() {
                cell.ch.to_string()
            } else {
                format!("{}{}", cell.ch, cell.combining)
            };
            job.append(&text, 0.0, text_format);
        }

        ui.label(job);
    }
}

#[cfg(test)]
fn terminal_output_row_badges(row: &legion_protocol::TerminalOutputRowProjection) -> Vec<String> {
    if let Some((prefix, detail)) = row.redacted_payload.split_once(" • ")
        && prefix.starts_with("command block ")
    {
        let mut badges = vec![
            prefix
                .replacen("command block ", "command-", 1)
                .replace(' ', "-"),
        ];
        badges.extend(detail.split(" • ").map(|segment| segment.to_string()));
        return badges;
    }

    let mut badges = Vec::new();
    if row.is_stderr {
        badges.push("stderr".to_string());
    }
    if row.truncated {
        badges.push("truncated".to_string());
    }
    badges.push(match row.redaction {
        legion_protocol::RedactionHint::None => "redacted=none".to_string(),
        legion_protocol::RedactionHint::MetadataOnly => "redacted=metadata-only".to_string(),
        legion_protocol::RedactionHint::Full => "redacted=full".to_string(),
    });
    badges.push(format!("{} bytes", row.byte_count));
    badges
}

fn render_activity_stream(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
) {
    section_label(ui, "Activity", Some(theme::tokens().accent.violet));
    theme::code_frame().show(ui, |ui| {
        if projected_product_mode(snapshot) != DesktopProductMode::Manual
            && !model.product_ai_stream_label.is_empty()
        {
            ui.label(theme::code_muted(format!(
                "Assistant response · {}{}{}",
                model.product_ai_stream_label,
                if model.product_ai_streamed {
                    " · sse-deltas"
                } else {
                    " · single-chunk"
                },
                if model.product_ai_stream_in_flight {
                    " · in-flight"
                } else {
                    ""
                }
            )));
            // The renderer lives behind the `ai` feature; the label above does
            // not. Without this gate the call fails to resolve in an
            // AI-less build — which is how `--no-default-features` broke, and
            // how the perf harness came to report a green `--strict` run with
            // its only budgeted workload silently downgraded to `skipped`.
            #[cfg(feature = "ai")]
            if !model.product_ai_stream_chunks.is_empty() {
                // Join SSE deltas into one markdown document for the rail.
                let body = model.product_ai_stream_chunks.join("");
                let mut noop = Vec::new();
                render_streaming_assistant_rows(
                    ui,
                    std::slice::from_ref(&body),
                    "No stream body",
                    6,
                    None,
                    &mut noop,
                );
            }
        }
        render_compact_rows(
            ui,
            &activity_rows(snapshot),
            "No recent workspace activity",
            8,
        );
    });
}

fn activity_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let assistant_requests = snapshot.assisted_ai_projection.request_count;
    if assistant_requests > 0 {
        rows.push(format!(
            "{assistant_requests} assistant request{} in this workspace",
            if assistant_requests == 1 { "" } else { "s" }
        ));
    }
    let delegated_plans = snapshot.delegated_task_projection.plan_count;
    if delegated_plans > 0 {
        rows.push(format!(
            "{delegated_plans} delegated plan{} available",
            if delegated_plans == 1 { "" } else { "s" }
        ));
    }
    let verification_runs = snapshot.verification_run_projection.rows.len();
    if verification_runs > 0 {
        rows.push(format!(
            "{verification_runs} verification run{} recorded",
            if verification_runs == 1 { "" } else { "s" }
        ));
    }
    if !snapshot.status_messages.is_empty() {
        rows.push(format!(
            "{} recent workspace notice{}",
            snapshot.status_messages.len(),
            if snapshot.status_messages.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    rows
}

fn render_diagnostics_panel(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    section_label(ui, "Diagnostics", Some(theme::tokens().accent.orange));
    theme::code_frame().show(ui, |ui| {
        render_compact_rows(ui, &model.bottom_console_rows, "No internal diagnostics", 8);
        ui.label(theme::code_muted(format!(
            "settings schema={} theme={} notifications={}",
            model.settings.schema_version,
            model.settings.theme_label,
            model.settings.toast_verbosity_label
        )));
        for row in model.settings.font_fallback_rows.iter().take(8) {
            ui.label(theme::code_muted(trim_middle(row, 96)));
        }
        for row in model.language_rows.iter().take(12) {
            ui.label(theme::code_muted(trim_middle(row, 96)));
        }
        for row in model.operational_health_rows.iter().take(4) {
            ui.label(theme::code_muted(trim_middle(row, 96)));
        }
    });
}

/// Renders per-diagnostic problem rows as selectable rows (D3, T4).
///
/// Each row shows `severity path:line message`. Clicking opens the file at
/// the problem's start line via `DesktopAction::NavigateToProblem`.
/// A problem with no path renders as plain text, because a row that cannot
/// say where the problem is has nowhere to send a click.
///
/// The row at `selected_index` is the keyboard-focused row (T4);
/// `ProblemNext`/`ProblemPrev`/`ProblemActivate` move selection and navigate.
///
/// Rendered with `selectable_label` and a real selected state rather than a
/// click-sensed `Label` carrying a chevron in its text. egui publishes a plain
/// label as static text: no `Action::Click`, no focus. Every row in this panel
/// therefore reached assistive technology -- and anything else reading the
/// accessibility tree -- as a sentence that could not be activated, while the
/// mouse opened the file perfectly. Selection had the same shape of problem: a
/// glyph glued to the front of the string is invisible to anything that asks a
/// control whether it is selected, and it left every unselected row's name
/// beginning with two spaces.
fn render_problem_rows(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    selected_index: usize,
    actions: &mut Vec<DesktopAction>,
) {
    let problems = &snapshot.language_tooling_projection.problems;
    if problems.is_empty() {
        ui.label(theme::muted("No problems"));
        return;
    }
    const LIMIT: usize = 12;
    for (i, problem) in problems.iter().take(LIMIT).enumerate() {
        let location = problem
            .path
            .as_ref()
            .map(|path| {
                // `display_path`, not the canonical path: on Windows every one
                // of these carries the \\\\?\\ extended-length prefix, which the
                // breadcrumb and status bar already strip and this row did not
                // -- so the panel named the file in a shape no reader has ever
                // typed.
                let shown = crate::path_display::display_path(&path.0);
                if let Some(range) = &problem.range {
                    format!("{}:{}", shown, range.start.line)
                } else {
                    shown.into_owned()
                }
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        // The diagnostic code, when the server sent one.
        //
        // Not decoration: `message` is replaced by a per-severity placeholder
        // before it reaches this projection, so without the code every error
        // row in the panel reads identically and a reader cannot tell a
        // mismatched type from a moved value. `E0308` is the one field that
        // still distinguishes them, and it is a structured identifier rather
        // than the server's prose -- the DIAGNOSTICS surface already renders
        // it and the source label next to the message for that reason.
        //
        // This narrows the gap; it does not close it. A code names the class
        // of error and not what is wrong in this line, and a server that sends
        // no code leaves the row exactly as uninformative as before.
        let code = problem
            .code_label
            .as_deref()
            .map(|code| format!("{code} "))
            .unwrap_or_default();
        let label = trim_middle(
            &format!(
                "{:?} {} {}{}",
                problem.severity, location, code, problem.message
            ),
            110,
        );
        // Only clickable when the problem says where it is.
        if let (Some(path), nav_line) = (
            problem.path.as_ref().map(|p| p.0.clone()),
            problem.range.as_ref().map(|r| r.start.line).unwrap_or(0),
        ) {
            let response = ui.selectable_label(i == selected_index, theme::body(&label));
            // Publish the selection, because egui 0.34 does not.
            //
            // `selectable_label` is `Button::selectable` here, and `Button`
            // reports itself with `WidgetInfo::labeled` -- the label and the
            // enabled flag, and nothing about being selected. So the row would
            // have been clickable and still unable to say which one the
            // keyboard is on, which is half of what the chevron was doing
            // badly. Restating the info stamps the state onto the same node.
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    ui.is_enabled(),
                    i == selected_index,
                    &label,
                )
            });
            if response.clicked() {
                actions.push(DesktopAction::NavigateToProblem {
                    path,
                    line: nav_line,
                });
            }
        } else {
            ui.label(theme::body(&label));
        }
    }
    if problems.len() > LIMIT {
        ui.label(theme::muted(format!(
            "{} more problems",
            problems.len() - LIMIT
        )));
    }
}

/// Longest a compact row may be before its middle is replaced by an ellipsis.
///
/// Named so callers that must keep a line readable end to end — the sandbox
/// panel's platform-limitation row, for one — can hold themselves to the same
/// budget in a test instead of guessing at it.
pub(crate) const COMPACT_ROW_CHAR_BUDGET: usize = 110;

fn render_compact_rows(ui: &mut egui::Ui, rows: &[String], empty: &str, limit: usize) {
    if rows.is_empty() {
        ui.label(theme::muted(empty));
        return;
    }
    for row in rows.iter().take(limit) {
        ui.label(theme::body(trim_middle(row, COMPACT_ROW_CHAR_BUDGET)));
    }
    if rows.len() > limit {
        ui.label(theme::muted(format!("{} more rows", rows.len() - limit)));
    }
}

fn sidebar_header(ui: &mut egui::Ui, title: &str, detail: String) {
    ui.horizontal(|ui| {
        ui.label(theme::eyebrow(title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::code_muted(trim_middle(&detail, 24)));
        });
    });
    ui.separator();
}

fn inspector_header(ui: &mut egui::Ui, title: &str, level: DesktopProductMode) {
    ui.horizontal(|ui| {
        ui.label(theme::heading(title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            pill(ui, level.label(), level_color(level), true);
        });
    });
    ui.separator();
}

fn status_dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

fn avatar(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(theme::dim(color, 30))
        .stroke(egui::Stroke::new(1.0_f32, theme::dim(color, 90)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.label(theme::accent(text, color));
        });
}

fn current_path(snapshot: &ShellProjectionSnapshot) -> &str {
    snapshot
        .active_buffer_projection
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<none>")
}

fn authority_ribbon_view_model(
    snapshot: &ShellProjectionSnapshot,
) -> DesktopAuthorityRibbonViewModel {
    let summary = match snapshot.product_mode {
        DockMode::Manual => "Manual · AI off · Workspace tools only",
        DockMode::Assist => "Assist · Suggestions require acceptance",
        DockMode::Delegate => "Delegate · Workspace scope · Changes remain proposals",
        DockMode::Automate => "Workflows · Reviews remain approval-gated",
    };
    DesktopAuthorityRibbonViewModel {
        summary: summary.to_string(),
        workspace_scope: snapshot
            .active_buffer_projection
            .workspace_id
            .or(snapshot.approval_checklist_projection.workspace_id)
            .map(|_| "Workspace scope".to_string()),
        provider_readiness: if snapshot.assisted_ai_projection.providers.is_empty() {
            None
        } else {
            let available = snapshot
                .assisted_ai_projection
                .providers
                .iter()
                .any(|provider| {
                    provider.availability == AssistedAiProviderAvailabilityState::Available
                });
            Some(if available {
                "Provider ready".to_string()
            } else {
                "Providers unavailable".to_string()
            })
        },
        approval_boundary: if snapshot.approval_checklist_projection.ready_for_approval {
            Some("Ready for approval · acceptance still required".to_string())
        } else if !snapshot.approval_checklist_projection.blockers.is_empty() {
            Some("Approval blocked".to_string())
        } else if !snapshot.approval_checklist_projection.gates.is_empty() {
            Some("Approval gates remain".to_string())
        } else {
            None
        },
    }
}

fn projected_cursor(snapshot: &ShellProjectionSnapshot) -> TextCoordinate {
    snapshot
        .active_buffer_projection
        .viewport
        .as_ref()
        .map(|viewport| viewport.cursor)
        .unwrap_or(TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: None,
        })
}

fn current_objective(snapshot: &ShellProjectionSnapshot) -> String {
    snapshot
        .delegated_task_projection
        .plan_rows
        .first()
        .map(|row| {
            row.labels
                .iter()
                .find(|label| !label.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| row.plan_id.0.clone())
        })
        .unwrap_or_else(|| "Delegated task".to_string())
}

fn delegate_owned_proposal_ids(snapshot: &ShellProjectionSnapshot) -> Vec<ProposalId> {
    if !delegated_task_owned_state_projected(snapshot) {
        return Vec::new();
    }
    snapshot
        .delegated_task_projection
        .proposal_preview_links
        .iter()
        .map(|link| link.proposal_id)
        .chain(
            snapshot
                .delegated_task_projection
                .step_summaries
                .iter()
                .filter_map(|step| step.proposal_id),
        )
        .fold(Vec::new(), |mut proposal_ids, proposal_id| {
            if !proposal_ids.contains(&proposal_id) {
                proposal_ids.push(proposal_id);
            }
            proposal_ids
        })
}

fn next_delegate_owned_proposal_id(snapshot: &ShellProjectionSnapshot) -> Option<ProposalId> {
    let owned = delegate_owned_proposal_ids(snapshot);
    let mut candidates = snapshot
        .delegated_task_projection
        .proposal_reviews
        .iter()
        .filter_map(|review| {
            owned
                .contains(&review.proposal_id)
                .then_some(review.proposal_id)
        })
        .collect::<Vec<_>>();
    for proposal_id in owned {
        if !candidates.contains(&proposal_id) {
            candidates.push(proposal_id);
        }
    }
    candidates.into_iter().find(|proposal_id| {
        delegate_owned_proposal_lifecycle(snapshot, *proposal_id).is_some_and(|state| {
            matches!(
                state,
                ProposalLifecycleState::Created
                    | ProposalLifecycleState::Validated
                    | ProposalLifecycleState::Previewed
                    | ProposalLifecycleState::Approved
            )
        })
    })
}

fn delegate_owned_proposal_lifecycle(
    snapshot: &ShellProjectionSnapshot,
    proposal_id: ProposalId,
) -> Option<ProposalLifecycleState> {
    snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .map(|row| row.lifecycle.state)
        .or_else(|| {
            snapshot
                .delegated_task_projection
                .proposal_preview_links
                .iter()
                .find(|link| link.proposal_id == proposal_id)
                .map(|link| link.lifecycle_state)
        })
}

fn delegated_plan_rows(
    snapshot: &ShellProjectionSnapshot,
    _model: &DesktopProjectionViewModel,
    skip: usize,
) -> Vec<String> {
    snapshot
        .delegated_task_projection
        .plan_rows
        .iter()
        .skip(skip)
        .map(|row| {
            format!(
                "{} {:?} {:?} risk={:?}",
                row.plan_id.0, row.plan_state, row.readiness, row.risk_label
            )
        })
        .collect::<Vec<_>>()
}

fn delegated_step_rows(
    snapshot: &ShellProjectionSnapshot,
    _model: &DesktopProjectionViewModel,
) -> Vec<String> {
    snapshot
        .delegated_task_projection
        .step_summaries
        .iter()
        .map(|row| {
            format!(
                "{} order={} {:?} proposal={:?}",
                row.step_id.0,
                row.order,
                row.state,
                row.proposal_id.map(|proposal| proposal.0)
            )
        })
        .collect::<Vec<_>>()
}

fn delegated_testing_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    matches!(
        snapshot.delegated_task_projection.runtime_activation,
        DelegatedTaskRuntimeActivationState::Verifying
    )
    .then(|| "Delegate verification is running".to_string())
    .into_iter()
    .collect()
}

fn delegated_done_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let label = match snapshot.delegated_task_projection.runtime_activation {
        DelegatedTaskRuntimeActivationState::Completed => "Delegate task completed",
        DelegatedTaskRuntimeActivationState::Cancelled => "Delegate task cancelled",
        DelegatedTaskRuntimeActivationState::Failed => "Delegate task failed",
        _ => return Vec::new(),
    };
    vec![label.to_string()]
}

fn proposal_board_rows(
    snapshot: &ShellProjectionSnapshot,
    _model: &DesktopProjectionViewModel,
) -> Vec<String> {
    delegated_proposal_rows(snapshot)
        .into_iter()
        .chain(
            snapshot
                .delegated_task_projection
                .tool_permission_requests
                .iter()
                .map(|request| {
                    format!(
                        "delegate permission {} {:?} {:?}",
                        trim_middle(&request.request_id, 32),
                        request.profile,
                        request.disposition
                    )
                }),
        )
        .collect()
}

fn delegated_proposal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let owned = delegate_owned_proposal_ids(snapshot);
    snapshot
        .delegated_task_projection
        .proposal_preview_links
        .iter()
        .filter(|link| owned.contains(&link.proposal_id))
        .map(|link| {
            format!(
                "delegate proposal {} payload={:?} risk={:?} lifecycle={:?}",
                link.proposal_id.0, link.payload_kind, link.risk_label, link.lifecycle_state
            )
        })
        .chain(
            snapshot
                .delegated_task_projection
                .proposal_reviews
                .iter()
                .filter(|review| owned.contains(&review.proposal_id))
                .flat_map(|review| {
                    review.hunks.iter().map(move |hunk| {
                        format!(
                            "delegate hunk {} proposal={} {:?}",
                            trim_middle(&hunk.hunk_id, 32),
                            review.proposal_id.0,
                            hunk.disposition
                        )
                    })
                }),
        )
        .collect()
}

fn level_color(level: DesktopProductMode) -> egui::Color32 {
    match level {
        DesktopProductMode::Manual => theme::tokens().modes.manual,
        DesktopProductMode::Assist => theme::tokens().modes.assist,
        DesktopProductMode::Delegate => theme::tokens().modes.delegate,
        DesktopProductMode::LegionWorkflows => theme::tokens().modes.workflows,
    }
}

fn risk_color(risk: ProposalRiskLabel) -> egui::Color32 {
    match risk {
        ProposalRiskLabel::Informational => theme::tokens().accent.cyan,
        ProposalRiskLabel::Low => theme::tokens().accent.green,
        ProposalRiskLabel::Medium => theme::tokens().accent.amber,
        ProposalRiskLabel::High => theme::tokens().accent.red,
        ProposalRiskLabel::Unknown => theme::tokens().text.muted,
    }
}

fn trim_middle(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    if max <= 3 {
        return "...".to_string();
    }
    let keep = max - 3;
    let head = keep / 2;
    let tail = keep - head;
    let start = value.chars().take(head).collect::<String>();
    let end = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}...{end}")
}

/// Map a pointer position over the editor text surface to a projected text coordinate.
pub fn editor_coordinate_from_pointer(
    pointer: egui::Pos2,
    text_origin: egui::Pos2,
    line_height: f32,
    char_width: f32,
    lines: &[DesktopCodeLineViewModel],
) -> Option<TextCoordinate> {
    if !line_height.is_finite()
        || !char_width.is_finite()
        || line_height <= 0.0
        || char_width <= 0.0
        || pointer.y < text_origin.y
    {
        return None;
    }
    let row = ((pointer.y - text_origin.y) / line_height).floor() as usize;
    let line = lines.get(row)?;
    Some(editor_coordinate_for_line_x(
        line,
        pointer.x,
        text_origin.x,
        char_width,
    ))
}

/// Return the word selection range containing a projected coordinate.
pub fn word_range_for_coordinate(
    line: &DesktopCodeLineViewModel,
    coordinate: TextCoordinate,
) -> Option<ProtocolTextRange> {
    if coordinate.line != line.number.saturating_sub(1) {
        return None;
    }
    let chars = line.text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return None;
    }
    let mut index =
        local_scalar_for_coordinate(line, coordinate)?.min(chars.len().saturating_sub(1));
    if !is_word_char(chars[index]) && index > 0 && is_word_char(chars[index - 1]) {
        index -= 1;
    }
    if !is_word_char(chars[index]) {
        return None;
    }

    let mut start = index;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = index + 1;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    Some(ProtocolTextRange {
        start: text_coordinate_for_line_scalar(line, start)?,
        end: text_coordinate_for_line_scalar(line, end)?,
    })
}

/// Return the full visible-line selection range for a code-canvas row.
pub fn line_range_for_code_line(line: &DesktopCodeLineViewModel) -> Option<ProtocolTextRange> {
    Some(ProtocolTextRange {
        start: text_coordinate_for_line_scalar(line, 0)?,
        end: text_coordinate_for_line_scalar(line, line.text.chars().count())?,
    })
}

fn code_line_truncation_marker(truncation_state: ViewportLineTruncationState) -> &'static str {
    match truncation_state {
        ViewportLineTruncationState::None => " ",
        ViewportLineTruncationState::Leading => "↤",
        ViewportLineTruncationState::Trailing => "↦",
        ViewportLineTruncationState::Both => "↔",
    }
}

/// Return the text coordinate where a same-line drag gesture began.
pub fn drag_anchor_for_line_pointer(
    line: &DesktopCodeLineViewModel,
    pointer_x: f32,
    total_drag_delta: egui::Vec2,
    origin_x: f32,
    char_width: f32,
) -> TextCoordinate {
    editor_coordinate_for_line_x(line, pointer_x - total_drag_delta.x, origin_x, char_width)
}

/// Build a drag selection range, preferring the gesture anchor over the stale projected cursor.
pub fn drag_selection_range(
    drag_anchor: Option<TextCoordinate>,
    current_cursor: TextCoordinate,
    coordinate: TextCoordinate,
) -> ProtocolTextRange {
    ProtocolTextRange {
        start: drag_anchor.unwrap_or(current_cursor),
        end: coordinate,
    }
}

fn editor_coordinate_for_line_x(
    line: &DesktopCodeLineViewModel,
    pointer_x: f32,
    origin_x: f32,
    char_width: f32,
) -> TextCoordinate {
    let raw_col = if pointer_x <= origin_x {
        0
    } else {
        ((pointer_x - origin_x) / char_width).floor() as u32
    };
    let scalar_index = raw_col.min(line.text.chars().count() as u32) as usize;
    let byte_column = line
        .text
        .char_indices()
        .nth(scalar_index)
        .map(|(offset, _)| offset)
        .unwrap_or(line.text.len());
    text_coordinate(line.number.saturating_sub(1), byte_column as u32)
}

fn code_drag_anchor_id(buffer_id: legion_protocol::BufferId) -> egui::Id {
    egui::Id::new(("legion_desktop_code_drag_anchor", buffer_id.0))
}

fn text_coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

fn is_word_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

/// Adapter-local render output.
///
/// `PartialEq` but not `Eq`: the observed dock fractions are `f32`, which has
/// no total equality. Comparing two outputs stays available; using one as a
/// hash key does not, and never did anything.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionViewOutput {
    /// True when adapter-local animation or timing needs another paint.
    pub needs_repaint: bool,
    /// Title displayed during this render.
    pub displayed_title: String,
    /// Bottom-tab model rows derived from the renderer's visible selection.
    pub bottom_tab_rows: Vec<String>,
    /// App-persistable bottom-panel selection after this frame's interactions.
    ///
    /// Diagnostics presentation does not overwrite this persisted selection.
    pub selected_bottom_panel: BottomPanelTab,
    /// Dock sizes this frame, as fractions of the shell, for persistence.
    ///
    /// Round-tripped through the runtime the same way `selected_bottom_panel`
    /// is: the renderer observes, the runtime decides whether it is worth
    /// storing. A field with `None` means that panel was not rendered.
    pub observed_dock_fractions: dock_geometry::DockFractions,
    /// Adapter actions requested by rendered controls.
    pub actions: Vec<DesktopAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopProductMode {
    Manual,
    Assist,
    Delegate,
    LegionWorkflows,
}

impl DesktopProductMode {
    fn label(self) -> &'static str {
        canonical_mode_entry(self).label
    }

    fn from_dock_mode(mode: DockMode) -> Self {
        match mode {
            DockMode::Manual => Self::Manual,
            DockMode::Assist => Self::Assist,
            DockMode::Delegate => Self::Delegate,
            DockMode::Automate => Self::LegionWorkflows,
        }
    }

    fn to_dock_mode(self) -> DockMode {
        match self {
            Self::Manual => DockMode::Manual,
            Self::Assist => DockMode::Assist,
            Self::Delegate => DockMode::Delegate,
            Self::LegionWorkflows => DockMode::Automate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeChromeSpec {
    mode: DesktopProductMode,
    ordinal: u8,
    icon: &'static str,
    micro: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BottomTabSpec {
    id: &'static str,
    label: &'static str,
    active: bool,
    color: egui::Color32,
    count: Option<usize>,
    selection: Option<BottomPanelTab>,
}

impl BottomTabSpec {
    fn new(
        id: &'static str,
        label: &'static str,
        active: bool,
        color: egui::Color32,
        count: Option<usize>,
        selection: BottomPanelTab,
    ) -> Self {
        Self {
            id,
            label,
            active,
            color,
            count,
            selection: Some(selection),
        }
    }

    fn diagnostics(active: bool) -> Self {
        Self {
            id: "diagnostics",
            label: "DIAGNOSTICS",
            active,
            color: theme::tokens().accent.orange,
            count: None,
            selection: None,
        }
    }
}

fn product_mode_switch_specs() -> [ModeChromeSpec; 4] {
    [
        ModeChromeSpec {
            mode: DesktopProductMode::Manual,
            ordinal: 1,
            icon: "keyboard",
            micro: "You write. AI stays quiet.",
        },
        ModeChromeSpec {
            mode: DesktopProductMode::Assist,
            ordinal: 2,
            icon: "sparkles",
            micro: "AI completes inline as you type.",
        },
        ModeChromeSpec {
            mode: DesktopProductMode::Delegate,
            ordinal: 3,
            icon: "layers",
            micro: "AI proposes multi-file diffs; you review and approve.",
        },
        ModeChromeSpec {
            mode: DesktopProductMode::LegionWorkflows,
            ordinal: 4,
            icon: "network",
            micro: "A full agent fleet plans, executes, tests, and reports.",
        },
    ]
}

fn canonical_mode_entry(mode: DesktopProductMode) -> &'static CanonicalProductMode {
    CANONICAL_PRODUCT_MODES
        .iter()
        .find(|entry| entry.variant == mode.to_dock_mode().to_product_mode())
        .expect("every renderer product mode maps to the canonical taxonomy")
}

fn projected_product_mode(snapshot: &ShellProjectionSnapshot) -> DesktopProductMode {
    DesktopProductMode::from_dock_mode(snapshot.product_mode)
}

fn projected_dock_mode(snapshot: &ShellProjectionSnapshot) -> DockMode {
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => DockMode::Manual,
        DesktopProductMode::Assist => DockMode::Assist,
        DesktopProductMode::Delegate => DockMode::Delegate,
        DesktopProductMode::LegionWorkflows => DockMode::Automate,
    }
}

fn desktop_theme_preference(preference: ThemePreferenceProjection) -> theme::ThemePreference {
    match preference {
        ThemePreferenceProjection::Dark => theme::ThemePreference::Dark,
        ThemePreferenceProjection::Light => theme::ThemePreference::Light,
        ThemePreferenceProjection::System => theme::ThemePreference::System,
    }
}

fn active_dock_layout<'a>(
    state: &'a DesktopProjectionViewState,
    mode: DockMode,
) -> DockLayoutRef<'a> {
    if let Some(layout) = state.dock_layouts.iter().find(|layout| layout.mode == mode) {
        DockLayoutRef::Borrowed(layout)
    } else {
        DockLayoutRef::Owned(DockLayout::standard(mode))
    }
}

enum DockLayoutRef<'a> {
    Borrowed(&'a DockLayout),
    Owned(DockLayout),
}

impl DockLayoutRef<'_> {
    fn as_layout(&self) -> &DockLayout {
        match self {
            Self::Borrowed(layout) => layout,
            Self::Owned(layout) => layout,
        }
    }
}

fn dock_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    let mode = projected_dock_mode(snapshot);
    let registry = PanelRegistry::standard();
    let layout_ref = active_dock_layout(state, mode);
    let layout = layout_ref.as_layout();
    let visible_count = registry.visible_for(mode).len();

    vec![
        format!(
            "dock registry: mode={} visible_panels={} registered_panels={}",
            mode.label(),
            visible_count,
            registry.panels().len()
        ),
        dock_side_row(
            DockSide::Left,
            layout.side(DockSide::Left),
            layout,
            &registry,
        ),
        dock_side_row(
            DockSide::Right,
            layout.side(DockSide::Right),
            layout,
            &registry,
        ),
        dock_side_row(
            DockSide::Bottom,
            layout.side(DockSide::Bottom),
            layout,
            &registry,
        ),
    ]
}

fn dock_side_row(
    side: DockSide,
    side_layout: &DockSideLayout,
    layout: &DockLayout,
    registry: &PanelRegistry,
) -> String {
    let visible = layout
        .visible_panel_ids(side, registry)
        .into_iter()
        .map(PanelId::as_str)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "dock side: {} pinned={} toolkit={} splitter={:.2} collapsed={} visible=[{}]",
        side.label(),
        side_layout.pinned_default.as_str(),
        side_layout.custom_toolkit.len(),
        side_layout.splitter_fraction,
        side_layout.collapsed,
        visible
    )
}

fn dock_panel_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    let mode = projected_dock_mode(snapshot);
    let registry = PanelRegistry::standard();
    let layout_ref = active_dock_layout(state, mode);
    let layout = layout_ref.as_layout();
    let mut rows = Vec::new();
    for side in [DockSide::Left, DockSide::Right, DockSide::Bottom] {
        for id in layout.visible_panel_ids(side, &registry) {
            if let Some(panel) = registry.panel(id) {
                let capabilities = panel
                    .capabilities
                    .iter()
                    .map(|capability| format!("{capability:?}"))
                    .collect::<Vec<_>>()
                    .join(",");
                rows.push(format!(
                    "dock panel: side={} id={} title={} requires_ai={} capabilities=[{}]",
                    side.label(),
                    panel.id.as_str(),
                    panel.title,
                    panel.requires_ai,
                    capabilities
                ));
            }
        }
    }
    rows
}

fn product_mode_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_product_mode(snapshot);
    let mut rows = vec![
        format!(
            "product mode: active={} app-owned projection",
            level.label()
        ),
        format!(
            "product modes: {}",
            product_mode_switch_specs()
                .iter()
                .map(|spec| canonical_mode_entry(spec.mode).label)
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    ];

    match level {
        DesktopProductMode::Manual => {
            rows.push("product-mode safety: Manual Mode has no AI dispatch path".to_string());
        }
        DesktopProductMode::Assist => {
            rows.push(
                "product-mode safety: assisted work is proposal-preview only; direct workspace apply unsupported"
                    .to_string(),
            );
        }
        DesktopProductMode::Delegate => {
            rows.push(
                "product-mode safety: delegated work is approval-gated; direct workspace apply unsupported"
                    .to_string(),
            );
        }
        DesktopProductMode::LegionWorkflows => {
            rows.push(format!(
                "product-mode safety: Legion Workflow sessions={}; apply remains proposal-mediated; unattended merge unsupported until approval",
                snapshot.legion_workflow_projection.total_session_count
            ));
        }
    }
    rows.push(
        "product-mode control: display-only; no provider, terminal, or apply authority".to_string(),
    );
    rows
}

fn autonomy_scale_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = projected_product_mode(snapshot);
    product_mode_switch_specs()
        .iter()
        .map(|spec| {
            let confirm = if mode_transition_policy(snapshot.product_mode, spec.mode.to_dock_mode())
                == ModeTransitionPolicy::Confirm
            {
                "required"
            } else {
                "none"
            };
            format!(
                "autonomy scale: n={} key={} label={} active={} icon={} confirm={} micro={}",
                spec.ordinal,
                canonical_mode_entry(spec.mode).shortcut_label,
                canonical_mode_entry(spec.mode).label,
                spec.mode == active,
                spec.icon,
                confirm,
                spec.micro
            )
        })
        .collect()
}

fn mode_confirmation_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = projected_product_mode(snapshot);
    product_mode_switch_specs()
        .iter()
        .map(|spec| {
            let label = canonical_mode_entry(spec.mode).label;
            if mode_transition_policy(snapshot.product_mode, spec.mode.to_dock_mode())
                == ModeTransitionPolicy::Confirm
            {
                format!(
                    "mode confirmation: target={} active={} required=true title=\"{}\" proposal_mediated=true bounded_permissions=true grants_permissions=false security_boundary=false body=\"{}\"",
                    label,
                    spec.mode == active,
                    mode_confirmation_title(spec.mode.to_dock_mode()),
                    MODE_CONFIRMATION_BODY
                )
            } else {
                format!(
                "mode confirmation: target={} active={} required=false",
                label,
                spec.mode == active
                )
            }
        })
        .collect()
}

fn command_palette_overlay(
    snapshot: &ShellProjectionSnapshot,
) -> DesktopCommandPaletteOverlayViewModel {
    let palette = &snapshot.palette_projection;
    DesktopCommandPaletteOverlayViewModel {
        open: palette.open,
        mode_label: palette.mode.label().to_string(),
        query: palette.query.clone(),
        scope_label: match palette.scope {
            SearchScopeProjection::ActiveFile => "Active File".to_string(),
            SearchScopeProjection::Workspace => "Workspace".to_string(),
        },
        result_rows: command_palette_result_rows(palette),
    }
}

fn toast_stack(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> DesktopToastStackViewModel {
    let dismissed_ids = state
        .dismissed_toast_ids
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let stack = ToastStackProjection::from_status_messages_with_verbosity(
        &snapshot.status_messages,
        &dismissed_ids,
        snapshot.settings_projection.toast_verbosity,
    );
    DesktopToastStackViewModel {
        visible: stack
            .visible
            .into_iter()
            .map(|toast| DesktopToastViewModel {
                id: toast.id,
                severity: toast.severity,
                title: toast.title,
                body: toast.body,
                action: toast.action,
                sticky: toast.sticky,
            })
            .collect(),
        overflow_count: stack.overflow_count,
    }
}

fn command_palette_result_rows(
    palette: &PaletteProjection,
) -> Vec<DesktopCommandPaletteResultViewModel> {
    let visible_start =
        command_palette_visible_result_start(palette.results.len(), palette.selected_index);
    palette
        .results
        .iter()
        .skip(visible_start)
        .take(COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
        .enumerate()
        .map(|(offset, result)| {
            let index = visible_start + offset;
            DesktopCommandPaletteResultViewModel {
                id: result.id.clone(),
                group_label: command_palette_group_label(&result.id).to_string(),
                kind_label: match result.kind {
                    PaletteResultKind::File => "File",
                    PaletteResultKind::Symbol => "Symbol",
                    PaletteResultKind::RecentBuffers => "Recent Buffers",
                    PaletteResultKind::Command => "Command",
                    PaletteResultKind::Search => "Search",
                    PaletteResultKind::StructuralSearch => "Structural Search",
                }
                .to_string(),
                title: result.title.clone(),
                detail: result.detail.clone(),
                shortcut_label: result.shortcut_label.clone(),
                match_indices: result.match_indices.clone(),
                selected: index == palette.selected_index && result.disabled_reason.is_none(),
                disabled_reason: result.disabled_reason.clone(),
            }
        })
        .collect()
}

pub(crate) fn command_palette_group_label(result_id: &str) -> &'static str {
    palette_command_group(result_id).label()
}

fn command_palette_visible_result_start(total: usize, selected_index: usize) -> usize {
    if total <= COMMAND_PALETTE_VISIBLE_RESULT_ROWS {
        return 0;
    }

    let selected_index = selected_index.min(total.saturating_sub(1));
    selected_index
        .saturating_add(1)
        .saturating_sub(COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
        .min(total - COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
}

fn command_palette_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let palette = &snapshot.palette_projection;
    let mut rows = vec![format!(
        "command palette overlay: open={} mode={} query=\"{}\" scope={:?} selected={} results={}",
        palette.open,
        palette.mode.label(),
        palette.query,
        palette.scope,
        palette.selected_index,
        palette.results.len()
    )];
    rows.extend(palette.results.iter().enumerate().map(|(index, result)| {
        format!(
            "command palette result: selected={} kind={:?} title=\"{}\" shortcut={} disabled={} matches={}",
            index == palette.selected_index,
            result.kind,
            result.title,
            result.shortcut_label.as_deref().unwrap_or("<none>"),
            result.disabled_reason.as_deref().unwrap_or("<none>"),
            result.match_indices.len()
        )
    }));
    rows
}

fn bottom_tab_rows(
    snapshot: &ShellProjectionSnapshot,
    selected: BottomPanelTab,
    diagnostics_active: bool,
) -> Vec<String> {
    bottom_tab_specs(snapshot, selected, diagnostics_active)
        .into_iter()
        .map(|tab| {
            let count = tab
                .count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "none".to_string());
            format!(
                "bottom tab: mode={} id={} label={} active={} count={}",
                projected_product_mode(snapshot).label(),
                tab.id,
                tab.label,
                tab.active,
                count
            )
        })
        .collect()
}

fn bottom_tab_specs(
    snapshot: &ShellProjectionSnapshot,
    selected: BottomPanelTab,
    diagnostics_active: bool,
) -> Vec<BottomTabSpec> {
    let problems = snapshot.language_tooling_projection.problems.len();
    let mut tabs = vec![
        BottomTabSpec::new(
            "term",
            "TERMINAL",
            !diagnostics_active && selected == BottomPanelTab::Terminal,
            theme::tokens().text.primary,
            None,
            BottomPanelTab::Terminal,
        ),
        BottomTabSpec::new(
            "problems",
            "PROBLEMS",
            !diagnostics_active && selected == BottomPanelTab::Problems,
            theme::tokens().accent.red,
            Some(problems),
            BottomPanelTab::Problems,
        ),
    ];
    tabs.push(BottomTabSpec::new(
        "activity",
        "ACTIVITY",
        !diagnostics_active && selected == BottomPanelTab::Activity,
        theme::tokens().accent.blue,
        None,
        BottomPanelTab::Activity,
    ));
    tabs.push(BottomTabSpec::diagnostics(diagnostics_active));
    tabs
}

fn delegated_task_owned_state_projected(snapshot: &ShellProjectionSnapshot) -> bool {
    let delegated = &snapshot.delegated_task_projection;
    delegated.runtime_activation != legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
        || delegated.plan_count > 0
        || !delegated.plan_rows.is_empty()
        || !delegated.step_summaries.is_empty()
}

fn delegated_task_is_blocked(snapshot: &ShellProjectionSnapshot) -> bool {
    use legion_protocol::{
        DelegatedTaskPlanState as PlanState, DelegatedTaskRuntimeActivationState,
    };
    let delegated = &snapshot.delegated_task_projection;
    delegated.runtime_activation == DelegatedTaskRuntimeActivationState::Blocked
        || !delegated.blockers.is_empty()
        || delegated
            .plan_rows
            .iter()
            .any(|row| row.plan_state == PlanState::Blocked)
}

fn delegate_lifecycle(snapshot: &ShellProjectionSnapshot) -> DelegateLifecycle {
    use legion_protocol::{
        DelegatedTaskPlanState as PlanState, DelegatedTaskRuntimeActivationState as RuntimeState,
    };
    let delegated = &snapshot.delegated_task_projection;
    match delegated.runtime_activation {
        RuntimeState::Planned
        | RuntimeState::SandboxAllocated
        | RuntimeState::Executing
        | RuntimeState::Verifying => DelegateLifecycle::Running,
        RuntimeState::WaitingForApproval | RuntimeState::Blocked => DelegateLifecycle::Waiting,
        RuntimeState::Completed | RuntimeState::Cancelled | RuntimeState::Failed => {
            DelegateLifecycle::Terminal
        }
        RuntimeState::NotEncoded => {
            if !delegated.plan_rows.is_empty()
                && delegated
                    .plan_rows
                    .iter()
                    .all(|row| matches!(row.plan_state, PlanState::Refused | PlanState::Cancelled))
            {
                DelegateLifecycle::Terminal
            } else if delegated.plan_rows.iter().any(|row| {
                matches!(
                    row.plan_state,
                    PlanState::Planned | PlanState::AwaitingApproval | PlanState::Blocked
                )
            }) || !delegated.blockers.is_empty()
            {
                DelegateLifecycle::Waiting
            } else {
                DelegateLifecycle::Draft
            }
        }
    }
}

fn delegated_runtime_is_cancellable(
    activation: legion_protocol::DelegatedTaskRuntimeActivationState,
) -> bool {
    matches!(
        activation,
        legion_protocol::DelegatedTaskRuntimeActivationState::Executing
            | legion_protocol::DelegatedTaskRuntimeActivationState::Verifying
    )
}

fn top_bar_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    vec![
        format!(
            "top bar identity: {LEGION_WORDMARK} workspace={}",
            snapshot.layout_projection.layout.title
        ),
        format!(
            "top bar modes: {} active={}",
            product_mode_switch_specs()
                .iter()
                .map(|spec| canonical_mode_entry(spec.mode).label)
                .collect::<Vec<_>>()
                .join(" | "),
            projected_product_mode(snapshot).label(),
        ),
        format!(
            "top bar command: label=Command presence={}",
            projected_presence_count_for_chrome(snapshot)
        ),
    ]
}

fn projected_presence_count_for_chrome(snapshot: &ShellProjectionSnapshot) -> usize {
    if projected_product_mode(snapshot) == DesktopProductMode::Manual {
        0
    } else {
        snapshot.collaboration_presence_projections.len()
    }
}

fn left_sidebar_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let selected = snapshot
        .explorer_projection
        .selection
        .as_ref()
        .map(|selection| selection.file_id.0.to_string())
        .unwrap_or_else(|| "none".to_string());
    vec![format!(
        "explorer chrome: title=EXPLORER · {} nodes={} selected_file={}",
        snapshot.layout_projection.layout.title,
        snapshot.explorer_projection.nodes.len(),
        selected
    )]
}

fn center_surface_label(surface: CenterSurface) -> &'static str {
    surface.label()
}

fn main_canvas_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let path = active
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<no file>");
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    let mut rows = vec![
        format!(
            "code canvas: tabs={} active_path={} dirty={} degraded={}",
            snapshot.daily_editing_projection.tabs.tabs.len(),
            path,
            active.dirty,
            active.degraded
        ),
        format!(
            "language cues: status={:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} definitions={} references={}",
            snapshot.language_tooling_projection.status,
            snapshot.language_tooling_projection.problems.len(),
            snapshot.language_tooling_projection.quick_fixes.len(),
            snapshot.language_tooling_projection.breadcrumbs.len(),
            snapshot.language_tooling_projection.sticky_scopes.len(),
            snapshot.language_tooling_projection.inlay_hints.len(),
            snapshot.language_tooling_projection.code_lenses.len(),
            snapshot.language_tooling_projection.completions.len(),
            snapshot.language_tooling_projection.definitions.len(),
            snapshot.language_tooling_projection.references.len()
        ),
        format!(
            "editor polish: sticky_headers={} code_folding={} minimap={} whitespace_guides={} indent_guides={} smooth_scrolling={} fold_ranges={} sticky_scopes={}",
            snapshot.settings_projection.editor.sticky_headers_visible,
            snapshot.settings_projection.editor.code_folding_visible,
            snapshot.settings_projection.editor.minimap_visible,
            snapshot.settings_projection.editor.whitespace_guides_visible,
            snapshot.settings_projection.editor.indent_guides_visible,
            snapshot.settings_projection.editor.smooth_scrolling_enabled,
            snapshot.active_buffer_projection.viewport.as_ref().map(|viewport| viewport.fold_ranges.len()).unwrap_or(0),
            snapshot.language_tooling_projection.sticky_scopes.len(),
        ),
        "keyboard: Tab accepts completion; Enter opens quick fixes; arrows move between results; Escape dismisses hover".to_string(),
        format!(
            "search strip: {}",
            search.header
        ),
        format!(
            "excerpt surface: sections={} lines={}",
            snapshot.excerpt_surface_projection.sections.len(),
            snapshot
                .excerpt_surface_projection
                .sections
                .iter()
                .map(|section| section.lines.len())
                .sum::<usize>()
        ),
        format!(
            "structural search strip: status={:?} matches={} proposal={:?}",
            snapshot.structural_search_projection.status.kind,
            snapshot.structural_search_projection.matches.len(),
            snapshot
                .structural_search_projection
                .proposal_id
                .map(|proposal| proposal.0)
        ),
    ];
    if let Some(prediction) = &snapshot
        .assist_inline_prediction_projection
        .active_prediction
    {
        rows.push(format!(
            "ghost prediction: id={} provider={} status={:?} latency={} stale={} range={} ghost={} replacement={}",
            prediction.prediction_id,
            prediction.provider_label,
            prediction.status,
            prediction_latency_label(prediction),
            prediction.stale,
            prediction.apply_range_label,
            prediction.ghost_text_label,
            prediction_replacement_label(prediction)
        ));
    }
    rows
}

fn directive_panel_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    vec![
        format!(
            "directive dock: proposals={} artifacts={} trust_items={} approval_gates={} proposal-mediated",
            snapshot.proposal_ledger_projection.rows.len(),
            snapshot.artifact_ledger_projection.rows.len(),
            snapshot.context_manifest_projection.manifest.items.len(),
            snapshot.approval_checklist_projection.gates.len()
        ),
        format!(
            "assistant console: requests={} refusals={} previews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.assisted_ai_projection.refusal_count,
            snapshot.assisted_ai_projection.preview_ready_count
        ),
        format!(
            "advanced surfaces: delegated={} plugins={} collaboration={} remote={}",
            snapshot.delegated_task_projection.plan_count,
            snapshot.plugin_contribution_projections.len(),
            snapshot.collaboration_gui_projection.session_rows.len(),
            snapshot.remote_gui_projection.session_rows.len()
        ),
    ]
}

fn bottom_console_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let health_rows = DesktopOperationalHealthSnapshot::from_projection(snapshot).rows();
    vec![
        format!(
            "bottom console: terminal_status={:?} terminal_rows={} omitted={} structural_matches={}",
            snapshot.terminal_panel_projection.status.kind,
            snapshot.terminal_panel_projection.output_rows.len(),
            snapshot
                .terminal_panel_projection
                .scrollback
                .omitted_row_count,
            snapshot.structural_search_projection.matches.len()
        ),
        format!(
            "workflow activity: status_messages={} health_rows={} audit=metadata-only",
            snapshot.status_messages.len(),
            health_rows.len()
        ),
        format!(
            "agent stream: assisted_requests={} delegated_steps={} verification_runs={} graph_nodes={} shared_reviews={} remote_reviews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.delegated_task_projection.step_summaries.len(),
            snapshot.verification_run_projection.rows.len(),
            snapshot.system_graph_projection.nodes.len(),
            snapshot
                .collaboration_gui_projection
                .shared_proposal_rows
                .len(),
            snapshot.remote_gui_projection.proposal_review_rows.len()
        ),
    ]
}

fn status_line_ending(active: &ActiveBufferProjection) -> Option<String> {
    if let Some(viewport) = &active.viewport {
        let mut saw_lf = false;
        let mut saw_crlf = false;
        for metric in &viewport.line_metrics {
            match metric.line_ending_width {
                1 => saw_lf = true,
                2 => saw_crlf = true,
                _ => {}
            }
        }
        return match (saw_lf, saw_crlf) {
            (true, true) => Some("Mixed EOL".to_string()),
            (true, false) => Some("LF".to_string()),
            (false, true) => Some("CRLF".to_string()),
            (false, false) => None,
        };
    }

    active.small_buffer_text().and_then(|preview| {
        let has_crlf = preview.contains("\r\n");
        let bytes = preview.as_bytes();
        let has_lf = bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\n' && (index == 0 || bytes[index - 1] != b'\r'));
        match (has_lf, has_crlf) {
            (true, true) => Some("Mixed EOL".to_string()),
            (true, false) => Some("LF".to_string()),
            (false, true) => Some("CRLF".to_string()),
            (false, false) => None,
        }
    })
}

fn status_language_for_path(path: &str) -> String {
    if has_ascii_extension(path, ".rs") {
        "rust"
    } else if has_ascii_extension(path, ".toml") {
        "toml"
    } else if has_ascii_extension(path, ".ts") || has_ascii_extension(path, ".tsx") {
        "typescript"
    } else if has_ascii_extension(path, ".js") || has_ascii_extension(path, ".jsx") {
        "javascript"
    } else if has_ascii_extension(path, ".md") {
        "markdown"
    } else if has_ascii_extension(path, ".json") {
        "json"
    } else {
        "text"
    }
    .to_string()
}

fn has_ascii_extension(path: &str, extension: &str) -> bool {
    let path = path.as_bytes();
    let extension = extension.as_bytes();
    path.len() > extension.len()
        && path[path.len() - extension.len()..].eq_ignore_ascii_case(extension)
}

/// The unsaved-changes prompt, as a centred modal.
///
/// This used to be a plain `ui.horizontal` appended to the central panel after
/// the code canvas — but the canvas allocates the panel's entire remaining
/// height, so the prompt was laid out past the bottom edge and rendered
/// off-screen at every window size. That was not cosmetic: `editor_input_enabled`
/// returns false while the prompt is active, so raising it disabled typing and
/// left the only two ways to dismiss it below the window. The app locked up and
/// looked like it had simply stopped responding.
///
/// A modal is also the honest shape for it. This is a blocking decision about
/// unsaved work; laying it out as ordinary flow content left it competing for
/// space with the file it was asking about.
fn render_close_dirty_prompt_modal(
    ctx: &egui::Context,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let Some(prompt) = &snapshot.daily_editing_projection.close_dirty_prompt else {
        return;
    };
    egui::Modal::new(egui::Id::new("legion_close_dirty_prompt")).show(ctx, |ui| {
        ui.set_max_width(360.0);
        ctx.accesskit_node_builder(ui.unique_id(), |node| {
            node.set_role(egui::accesskit::Role::Dialog);
            node.set_label("Unsaved changes");
            node.set_description(prompt.message.clone());
            node.set_modal();
        });
        ui.label(theme::title("Unsaved changes"));
        ui.add_space(6.0);
        // The projection already carries the sentence to show; composing one
        // here from the tab list would drift from what app authority says.
        ui.label(theme::body(prompt.message.clone()));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Save and close").clicked() {
                actions.push(DesktopAction::SaveDirtyClose {
                    buffer_id: prompt.buffer_id,
                });
            }
            if ui.button("Cancel").clicked() {
                actions.push(DesktopAction::CancelDirtyClose {
                    buffer_id: prompt.buffer_id,
                });
            }
        });
        ui.add_space(4.0);
        ui.label(theme::muted("Enter saves and closes · Escape cancels"));
        // There is deliberately no "Discard" here yet: no discard path exists
        // anywhere in app authority, so offering the button would either do
        // nothing or lie about what it did. Closing without saving is a real
        // gap, tracked separately — it needs a way to drop buffer edits that
        // app authority actually owns, not a renderer-side shortcut.
    });
}

fn render_explorer_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    if snapshot.explorer_projection.nodes.is_empty() {
        ui.label("<empty explorer>");
        return;
    }

    let selected = state.selected_explorer_file.or_else(|| {
        snapshot
            .explorer_projection
            .selection
            .as_ref()
            .map(|selection| selection.file_id)
    });
    for node in top_level_explorer_nodes(&snapshot.explorer_projection.nodes) {
        render_explorer_node(
            ui,
            node,
            &snapshot.explorer_projection.nodes,
            0,
            selected,
            state,
            actions,
        );
    }
}

/// Width of the disclosure slot every explorer row reserves.
const DISCLOSURE_SLOT_WIDTH: f32 = 14.0;

/// A collapsed (▸) or expanded (▾) disclosure triangle.
///
/// Painted rather than typed for the same reason the rail icons are: the
/// characters for these arrows are not in every font this app may end up
/// running with, and a tree whose disclosure markers render as `□` is worse
/// than one with no markers at all.
fn paint_disclosure_triangle(
    painter: &egui::Painter,
    slot: egui::Rect,
    expanded: bool,
    color: egui::Color32,
) {
    let center = slot.center();
    let reach = 3.5_f32;
    let points = if expanded {
        vec![
            egui::pos2(center.x - reach, center.y - reach * 0.6),
            egui::pos2(center.x + reach, center.y - reach * 0.6),
            egui::pos2(center.x, center.y + reach * 0.8),
        ]
    } else {
        vec![
            egui::pos2(center.x - reach * 0.6, center.y - reach),
            egui::pos2(center.x + reach * 0.8, center.y),
            egui::pos2(center.x - reach * 0.6, center.y + reach),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

fn render_explorer_node(
    ui: &mut egui::Ui,
    node: &legion_ui::ExplorerNodeProjection,
    nodes: &[legion_ui::ExplorerNodeProjection],
    depth: usize,
    selected: Option<FileId>,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    let is_expanded = state
        .expanded_explorer_paths
        .contains(&node.canonical_path.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space((depth as f32) * 12.0);
        // Every row reserves the same disclosure slot, directory or not, so the
        // names line up in a column. Files previously rendered a literal "-"
        // here and directories a "v"/">" inside a full button frame, which made
        // the tree read as a list of bulleted buttons rather than a tree.
        //
        // Keyed on `is_directory`, not on the child list: an empty directory
        // has no children and still needs its chevron, or it renders as a file
        // that refuses to open.
        let (slot, disclosure) = ui.allocate_exact_size(
            egui::vec2(DISCLOSURE_SLOT_WIDTH, 16.0),
            if node.is_directory {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        if node.is_directory {
            paint_disclosure_triangle(
                ui.painter(),
                slot,
                is_expanded,
                if disclosure.hovered() {
                    theme::tokens().text.primary
                } else {
                    theme::tokens().text.muted
                },
            );
            if disclosure.clicked() {
                actions.push(DesktopAction::ToggleExplorerPath {
                    path: node.canonical_path.0.clone(),
                });
            }
        }
        if ui
            .selectable_label(Some(node.file_id) == selected, &node.name)
            .clicked()
        {
            actions.push(DesktopAction::SelectExplorerFile {
                file_id: node.file_id,
            });
        }
    });

    if is_expanded {
        for child_id in &node.children {
            if let Some(child) = nodes
                .iter()
                .find(|candidate| candidate.file_id == *child_id)
            {
                render_explorer_node(ui, child, nodes, depth + 1, selected, state, actions);
            }
        }
    }
}

fn tab_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return vec!["<no open tabs>".to_string()];
    }

    tabs.iter()
        .map(|tab| {
            let active = if tab.active { "*" } else { " " };
            let dirty = if tab.dirty { " +" } else { "" };
            let pinned = if tab.pinned { " pinned" } else { "" };
            let preview = if tab.preview { " preview" } else { "" };
            let path = tab
                .file_path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<untitled>");
            format!(
                "{active} {}{} [buffer {}] {path}{pinned}{preview}",
                tab.title, dirty, tab.buffer_id.0
            )
        })
        .collect()
}

fn explorer_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    if snapshot.explorer_projection.nodes.is_empty() {
        return vec!["<empty explorer>".to_string()];
    }

    let selected = state.selected_explorer_file.or_else(|| {
        snapshot
            .explorer_projection
            .selection
            .as_ref()
            .map(|selection| selection.file_id)
    });

    let mut rows = Vec::new();
    for node in top_level_explorer_nodes(&snapshot.explorer_projection.nodes) {
        push_explorer_row(
            &mut rows,
            node,
            &snapshot.explorer_projection.nodes,
            0,
            selected,
            &state.expanded_explorer_paths,
        );
    }

    rows
}

fn top_level_explorer_nodes(
    nodes: &[legion_ui::ExplorerNodeProjection],
) -> Vec<&legion_ui::ExplorerNodeProjection> {
    let child_ids = nodes
        .iter()
        .flat_map(|node| node.children.iter().copied())
        .collect::<HashSet<_>>();
    nodes
        .iter()
        .filter(|node| !child_ids.contains(&node.file_id))
        .collect()
}

fn push_explorer_row(
    rows: &mut Vec<String>,
    node: &legion_ui::ExplorerNodeProjection,
    nodes: &[legion_ui::ExplorerNodeProjection],
    depth: usize,
    selected: Option<FileId>,
    expanded: &BTreeSet<String>,
) {
    let selection_marker = if Some(node.file_id) == selected {
        "*"
    } else {
        " "
    };
    let is_expanded = expanded.contains(&node.canonical_path.0);
    let expansion_marker = if !node.is_directory {
        "-"
    } else if is_expanded {
        "v"
    } else {
        ">"
    };
    let indent = "  ".repeat(depth);
    rows.push(format!(
        "{selection_marker} {expansion_marker} {indent}{} - {}",
        node.name, node.canonical_path.0
    ));

    if is_expanded {
        for child_id in &node.children {
            if let Some(child) = nodes
                .iter()
                .find(|candidate| candidate.file_id == *child_id)
            {
                push_explorer_row(rows, child, nodes, depth + 1, selected, expanded);
            }
        }
    }
}

fn active_buffer_lines(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    if active.buffer_id.is_none() {
        return vec!["<no active buffer>".to_string()];
    }

    if !active.degraded
        && let Some(text) = active.small_buffer_text()
    {
        if text.is_empty() {
            return vec!["<empty buffer>".to_string()];
        }
        return text.lines().map(ToString::to_string).collect();
    }

    if let Some(viewport) = &active.viewport {
        if viewport.line_slices.is_empty() {
            return vec!["<empty viewport>".to_string()];
        }
        return viewport
            .line_slices
            .iter()
            .map(|line| format!("{:>4}: {}", line.line_number + 1, line.visible_text))
            .collect();
    }

    vec!["<active buffer has no visible text>".to_string()]
}

fn large_file_banner_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let Some(viewport) = &snapshot.active_buffer_projection.viewport else {
        return Vec::new();
    };
    let Some(status) = &viewport.large_file_status else {
        return Vec::new();
    };

    let size_mb = status.byte_len as f64 / (1024.0 * 1024.0);
    // Name the state and the capability reduction explicitly, and distinguish
    // the two large-file modes. They cost the user different things: a degraded
    // buffer holds the whole file and defers overlays, a streamed one never
    // held it at all, so anything needing the entire text is not slow — it is
    // unavailable. One banner for both would promise something.
    let mut rows = vec![match viewport.mode {
        ViewportProjectionMode::StreamingLargeFile => format!(
            "\u{26a0} large-file streaming mode ({:.1} MB) \u{2014} capabilities reduced",
            size_mb
        ),
        _ => format!(
            "\u{26a0} large-file degraded mode ({:.1} MB) \u{2014} capabilities reduced",
            size_mb
        ),
    }];
    if viewport.mode == ViewportProjectionMode::StreamingLargeFile {
        rows.push(
            "This file is read from disk in chunks and is never held in memory in full."
                .to_string(),
        );
        rows.push(
            "  \u{2022} capability reduced: operations needing the whole file at once".to_string(),
        );
    }
    if !status.message.is_empty() {
        rows.push(status.message.clone());
    }
    rows.extend(status.disabled_overlay_reasons.iter().map(|reason| {
        format!(
            "  \u{2022} capability reduced: {}",
            sanitize_large_file_reason(reason)
        )
    }));
    rows
}

fn sanitize_large_file_reason(value: &str) -> String {
    let label = value.trim();
    if label.is_empty() || looks_like_local_path(label) {
        return "metadata-redacted".to_string();
    }

    let normalized = label
        .chars()
        .filter(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | ':' | ',' | '=')
        })
        .take(96)
        .collect::<String>();
    if normalized.trim().is_empty() {
        "metadata-redacted".to_string()
    } else {
        normalized
    }
}

fn looks_like_local_path(value: &str) -> bool {
    value.contains('\\') || value.contains('/') || value.contains(":\\")
}

fn active_buffer_code_lines(snapshot: &ShellProjectionSnapshot) -> Vec<DesktopCodeLineViewModel> {
    let active = &snapshot.active_buffer_projection;
    if active.buffer_id.is_none() {
        return Vec::new();
    }

    if let Some(viewport) = &active.viewport
        && !viewport.line_slices.is_empty()
    {
        return viewport
            .line_slices
            .iter()
            .enumerate()
            .map(|(index, line)| DesktopCodeLineViewModel {
                number: line.line_number + 1,
                text: line.visible_text.clone(),
                highlights: semantic_highlights_for_line(
                    line.line_number,
                    &line.visible_text,
                    &viewport.semantic_token_overlays,
                ),
                truncation_state: line.truncation_state,
                byte_range: line.byte_range,
                utf16_range: line.utf16_range,
                line_start_byte_offset: viewport
                    .line_metrics
                    .get(index)
                    .and_then(|metric| metric.line_start_byte_offset),
                logical_end_byte: viewport.line_metrics.get(index).and_then(|metric| {
                    metric
                        .line_start_byte_offset
                        .and_then(|start| start.checked_add(metric.byte_length))
                }),
                line_start_utf16_offset: viewport
                    .line_metrics
                    .get(index)
                    .and_then(|metric| metric.line_start_utf16_offset),
            })
            .collect();
    }

    if !active.degraded
        && let Some(text) = active.small_buffer_text()
    {
        return small_buffer_code_lines(text);
    }

    Vec::new()
}

fn small_buffer_code_lines(text: &str) -> Vec<DesktopCodeLineViewModel> {
    let mut rows = Vec::new();
    let mut byte_start = 0u64;
    let mut utf16_start = 0u64;
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        let content = segment.strip_suffix('\n').unwrap_or(segment);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let byte_len = content.len() as u64;
        let utf16_len = content.encode_utf16().count() as u32;
        rows.push(DesktopCodeLineViewModel {
            number: index as u32 + 1,
            text: content.to_string(),
            highlights: Vec::new(),
            truncation_state: ViewportLineTruncationState::None,
            byte_range: ByteRange::new(byte_start, byte_start + byte_len),
            utf16_range: Utf16Range {
                start: legion_protocol::Utf16Position {
                    line: index as u32,
                    character: 0,
                },
                end: legion_protocol::Utf16Position {
                    line: index as u32,
                    character: utf16_len,
                },
            },
            line_start_byte_offset: Some(byte_start),
            logical_end_byte: Some(byte_start + byte_len),
            line_start_utf16_offset: Some(utf16_start),
        });
        byte_start += segment.len() as u64;
        utf16_start += u64::from(utf16_len)
            + if segment.ends_with("\r\n") {
                2_u64
            } else {
                1_u64
            };
    }
    if rows.is_empty() || text.ends_with('\n') {
        let index = rows.len() as u32;
        rows.push(DesktopCodeLineViewModel {
            number: index + 1,
            text: String::new(),
            highlights: Vec::new(),
            truncation_state: ViewportLineTruncationState::None,
            byte_range: ByteRange::new(byte_start, byte_start),
            utf16_range: Utf16Range {
                start: legion_protocol::Utf16Position {
                    line: index,
                    character: 0,
                },
                end: legion_protocol::Utf16Position {
                    line: index,
                    character: 0,
                },
            },
            line_start_byte_offset: Some(byte_start),
            logical_end_byte: Some(byte_start),
            line_start_utf16_offset: Some(utf16_start),
        });
    }
    rows
}

fn semantic_highlights_for_line(
    line_number: u32,
    visible_text: &str,
    overlays: &[ViewportSemanticTokenOverlay],
) -> Vec<DesktopCodeHighlightSpan> {
    let max_col = visible_text.chars().count() as u32;
    let mut spans = overlays
        .iter()
        .filter(|overlay| overlay.line_number == line_number)
        .filter_map(|overlay| {
            let start_col = overlay.start_col.min(max_col);
            let end_col = overlay.end_col.min(max_col);
            (start_col < end_col).then_some(DesktopCodeHighlightSpan {
                start_col,
                end_col,
                kind: overlay.kind,
            })
        })
        .collect::<Vec<_>>();
    spans.sort_by_key(|span| (span.start_col, span.end_col));
    spans
}

fn editor_status_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let Some(buffer_id) = active.buffer_id else {
        return vec!["editor: no active buffer".to_string()];
    };

    let path = active
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<untitled>");
    let dirty = if active.dirty { "dirty" } else { "clean" };
    let mode = if active
        .viewport
        .as_ref()
        .is_some_and(|viewport| viewport.mode == ViewportProjectionMode::StreamingLargeFile)
    {
        "StreamingLargeFile"
    } else if active.degraded {
        "DegradedLargeFile"
    } else if active.viewport.is_some() {
        "viewport"
    } else {
        "small-buffer"
    };

    vec![format!(
        "editor: buffer {} {dirty} {mode} path={path}",
        buffer_id.0
    )]
}

fn close_prompt_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let Some(prompt) = &snapshot.daily_editing_projection.close_dirty_prompt else {
        return Vec::new();
    };

    let path = prompt
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<untitled>");
    vec![format!(
        "close_dirty buffer {} {} path={path}: {}",
        prompt.buffer_id.0, prompt.title, prompt.message
    )]
}

fn viewport_metadata_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let states = &snapshot.daily_editing_projection.viewport_states;
    if states.is_empty() {
        if let Some(viewport) = &snapshot.active_buffer_projection.viewport {
            return vec![format!(
                "viewport buffer {} cursor={} selections={} scroll={}:{} mode={:?}",
                viewport.buffer_id.0,
                coordinate_label(&viewport.cursor),
                viewport.selections.len(),
                viewport.scroll.top_line,
                viewport.scroll.left_column,
                viewport.mode
            )];
        }
        return vec!["<no viewport state>".to_string()];
    }

    states
        .iter()
        .map(|state| {
            let cursor = state
                .cursor
                .as_ref()
                .map(coordinate_label)
                .unwrap_or_else(|| "<none>".to_string());
            format!(
                "viewport buffer {} cursor={} selections={} scroll={}:{}",
                state.buffer_id.0,
                cursor,
                state.selections.len(),
                state.scroll.top_line,
                state.scroll.left_column
            )
        })
        .collect()
}

fn coordinate_label(coordinate: &legion_protocol::TextCoordinate) -> String {
    format!("{}:{}", coordinate.line, coordinate.character)
}

fn status_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .status_messages
        .iter()
        .map(|status| {
            let severity = match status.severity {
                StatusSeverity::Info => "info",
                StatusSeverity::Warning => "warning",
                StatusSeverity::Error => "error",
            };
            match save_rejection_status_marker(&status.message) {
                Some(marker) => format!("{severity} {marker}: {}", status.message),
                None => format!("{severity}: {}", status.message),
            }
        })
        .collect()
}

fn save_rejection_status_marker(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("save") {
        return None;
    }
    if lower.contains("conflict") {
        Some("save_conflict")
    } else if lower.contains("stale") {
        Some("save_stale")
    } else if lower.contains("denied") {
        Some("save_denied")
    } else if lower.contains("reject") {
        Some("save_rejected")
    } else {
        None
    }
}

fn proposal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let ledger = &snapshot.proposal_ledger_projection;
    let mut rows = Vec::new();
    if let Some(selected) = ledger.selected_proposal_id {
        rows.push(format!("selected proposal: {}", selected.0));
    }

    for row in ledger.rows.iter().take(12) {
        rows.push(format!(
            "proposal {}: {} [{} {:?} {:?}] payload={:?} rollback={:?}",
            row.proposal_id.0,
            row.title,
            row.lifecycle.label,
            row.risk_label,
            row.privacy_label,
            row.payload_kind,
            row.rollback
        ));
        rows.push(format!(
            "proposal {} diff: {:?} targets={} hunks={} +{} -{} omitted={} hash={}",
            row.proposal_id.0,
            row.diff_summary.kind,
            row.diff_summary.target_count,
            row.diff_summary.hunk_count,
            row.diff_summary.inserted_line_count,
            row.diff_summary.deleted_line_count,
            row.diff_summary.omitted_hunk_count,
            row.diff_summary
                .diff_hash
                .as_ref()
                .map(|hash| hash.value.as_str())
                .unwrap_or("<none>")
        ));
        rows.push(format!(
            "proposal {} targets: {:?} shown={} omitted={} redaction={}",
            row.proposal_id.0,
            row.target_coverage.coverage_kind,
            row.target_coverage.targets.len(),
            row.target_coverage.omitted_target_count,
            redaction_label(&row.target_coverage.redaction_hints)
        ));
        rows.extend(row.target_coverage.targets.iter().take(4).map(|target| {
            format!(
                "proposal {} target {}: {:?} file={:?} buffer={:?} path={} ranges={} redaction={}",
                row.proposal_id.0,
                target.target_id,
                target.kind,
                target.file_id.map(|file| file.0),
                target.buffer_id.map(|buffer| buffer.0),
                target
                    .path
                    .as_ref()
                    .map(|path| path.0.as_str())
                    .unwrap_or("<redacted>"),
                target.byte_ranges.len(),
                redaction_label(&target.redaction_hints)
            )
        }));
        rows.push(format!(
            "proposal {} context: {} categories={} items={} omitted={} redaction={}",
            row.proposal_id.0,
            row.context_manifest.manifest_id,
            row.context_manifest.category_count,
            row.context_manifest.total_item_count,
            row.context_manifest.omitted_item_count,
            redaction_label(&row.context_manifest.redaction_hints)
        ));
        if !row.preview_warnings.is_empty() {
            rows.push(format!(
                "proposal {} warnings: {}",
                row.proposal_id.0,
                row.preview_warnings.len()
            ));
        }
        rows.extend(row.preview_warnings.iter().take(4).map(|warning| {
            format!(
                "proposal {} warning {} {:?}: {}",
                row.proposal_id.0, warning.code, warning.kind, warning.message
            )
        }));
        if !row.diagnostics.is_empty() {
            rows.push(format!(
                "proposal {} diagnostics: {}",
                row.proposal_id.0,
                row.diagnostics.len()
            ));
        }
    }
    if ledger.omitted_row_count > 0 {
        rows.push(format!(
            "proposal ledger omitted rows: {}",
            ledger.omitted_row_count
        ));
    }
    rows
}

#[cfg(test)]
#[path = "view/projection_view_tests.rs"]
mod tests;
