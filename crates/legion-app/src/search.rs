//! Bounded lexical search over the active file and the workspace.
//!
//! This module owns the app-side lexical search pipeline: query parsing (mode
//! and glob prefixes), the per-line match walk that produces
//! [`SearchResultProjection`] rows, and the [`AppComposition`] entry points that
//! drive them.  It was extracted from `lib.rs` so that search work does not have
//! to contend for the crate root, which is the merge chokepoint for this crate.
//!
//! Structural (syntax-aware) search still lives in `lib.rs`; only the lexical
//! path moved here.

use std::sync::Arc;

use globset::{Glob, GlobSet, GlobSetBuilder};
use legion_editor::BufferMode;
use legion_project::{
    SearchPattern, SearchPatternKind, WorkspaceSearchBatch, WorkspaceSearchFilters,
    WorkspaceSearchQuery,
};
use legion_protocol::{
    CanonicalPath, FileId, ProtocolTextRange, TextCoordinate, TimestampMillis, WorkspaceId,
};
use legion_ui::{
    SearchProjection, SearchResultProjection, SearchScopeProjection, SearchStatusKindProjection,
    SearchStatusProjection,
};

use crate::{
    ActiveFileMetadata, AppComposition, AppCompositionError, BufferId, SEARCH_DEFAULT_RESULT_LIMIT,
    SEARCH_MAX_RESULT_LIMIT, SEARCH_SNIPPET_LIMIT_BYTES,
};

/// Explicit search option overrides. `None` means "fall through to
/// text-prefix parsing" for the corresponding option.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchQueryOptions {
    /// Override case-sensitivity. `None` defers to `case` / `icase` prefixes.
    pub case_sensitive: Option<bool>,
    /// Override whole-word matching. `None` defers to `word:` prefix.
    pub whole_word: Option<bool>,
    /// Override regex mode. `None` defers to `regex:` / `re:` prefixes.
    pub use_regex: Option<bool>,
}

/// Parsed and compiled search query, including effective option values.
struct ParsedSearchQuery {
    /// Compiled search pattern ready for `search_workspace_stream`.
    pattern: SearchPattern,
    /// Include/exclude glob filters (from `include:` / `exclude:` prefixes).
    filters: WorkspaceSearchFilters,
    /// The raw pattern text (without mode/option prefixes).
    search_text: String,
    /// Whether the pattern was compiled as literal (not regex). Used to
    /// decide whether the indexed back-end can be used.
    is_literal: bool,
    /// Effective case-sensitivity after applying overrides.
    case_sensitive: bool,
    /// Effective whole-word mode after applying overrides.
    whole_word: bool,
    /// Effective regex mode after applying overrides.
    use_regex: bool,
}

#[derive(Debug, Default)]
struct SearchBuildResult {
    results: Vec<SearchResultProjection>,
    omitted_result_count: usize,
    omitted_file_count: usize,
    diagnostics: Vec<String>,
    degraded_limited: bool,
    validation_error: Option<String>,
    /// Number of files skipped because they were detected as binary by the
    /// NUL-byte heuristic.  Propagated from `WorkspaceSearchReport` into
    /// `SearchProjection` so the desktop panel can display the count.
    skipped_binary_count: usize,
    /// Effective search options used for this result set.
    case_sensitive: bool,
    /// Effective whole-word option used for this result set.
    whole_word: bool,
    /// Effective regex mode used for this result set.
    use_regex: bool,
}

struct SearchTextInput<'a> {
    query_id: &'a str,
    pattern: &'a SearchPattern,
    scope: SearchScopeProjection,
    workspace_id: Option<WorkspaceId>,
    buffer_id: Option<BufferId>,
    file_id: Option<FileId>,
    file_path: Option<CanonicalPath>,
    text: &'a str,
    limit: usize,
    result: &'a mut SearchBuildResult,
}

struct SearchLineInput<'a> {
    query_id: &'a str,
    pattern: &'a SearchPattern,
    scope: SearchScopeProjection,
    workspace_id: Option<WorkspaceId>,
    buffer_id: Option<BufferId>,
    file_id: Option<FileId>,
    file_path: Option<CanonicalPath>,
    line_number: u32,
    line_text: &'a str,
    absolute_line_start: u64,
    limit: usize,
    result: &'a mut SearchBuildResult,
}

fn parse_search_query(
    query: &str,
    options: SearchQueryOptions,
) -> Result<ParsedSearchQuery, String> {
    let mut mode = SearchPatternKind::Literal;
    // Text-prefix defaults; explicit overrides are applied below.
    let mut case_sensitive = true;
    let mut whole_word = false;
    let mut include_globs = Vec::<String>::new();
    let mut exclude_globs = Vec::<String>::new();
    let mut pattern_parts = Vec::<String>::new();

    for token in query.split_whitespace() {
        if let Some(pattern) = token.strip_prefix("regex:") {
            mode = SearchPatternKind::Regex;
            if !pattern.is_empty() {
                pattern_parts.push(pattern.to_string());
            }
            continue;
        }
        if let Some(pattern) = token.strip_prefix("re:") {
            mode = SearchPatternKind::Regex;
            if !pattern.is_empty() {
                pattern_parts.push(pattern.to_string());
            }
            continue;
        }
        if let Some(pattern) = token.strip_prefix("literal:") {
            mode = SearchPatternKind::Literal;
            if !pattern.is_empty() {
                pattern_parts.push(pattern.to_string());
            }
            continue;
        }
        if let Some(pattern) = token.strip_prefix("word:") {
            whole_word = true;
            if !pattern.is_empty() {
                pattern_parts.push(pattern.to_string());
            }
            continue;
        }
        if token == "word" {
            whole_word = true;
            continue;
        }
        if token == "case" {
            case_sensitive = true;
            continue;
        }
        if token == "icase" || token == "nocase" {
            case_sensitive = false;
            continue;
        }
        if let Some(pattern) = token.strip_prefix("include:") {
            if pattern.is_empty() {
                return Err("include glob is empty".to_string());
            }
            include_globs.push(pattern.to_string());
            continue;
        }
        if let Some(pattern) = token.strip_prefix("exclude:") {
            if pattern.is_empty() {
                return Err("exclude glob is empty".to_string());
            }
            exclude_globs.push(pattern.to_string());
            continue;
        }
        pattern_parts.push(token.to_string());
    }

    // Explicit override flags take precedence over text-prefix parsing.
    if let Some(v) = options.case_sensitive {
        case_sensitive = v;
    }
    if let Some(v) = options.whole_word {
        whole_word = v;
    }
    if let Some(true) = options.use_regex {
        mode = SearchPatternKind::Regex;
    } else if options.use_regex == Some(false) {
        mode = SearchPatternKind::Literal;
    }
    let effective_use_regex = matches!(mode, SearchPatternKind::Regex);

    let pattern = pattern_parts.join(" ").trim().to_string();
    if pattern.is_empty() {
        return Err("Search query is empty".to_string());
    }

    let compiled_pattern = match mode {
        SearchPatternKind::Literal => SearchPattern::literal(&pattern, case_sensitive, whole_word)
            .map_err(|err| format!("invalid literal search: {err}"))?,
        SearchPatternKind::Regex => SearchPattern::regex(&pattern, case_sensitive, whole_word)
            .map_err(|err| format!("invalid regex search: {err}"))?,
    };

    let include = compile_search_globset(&include_globs)?;
    let exclude = compile_search_globset(&exclude_globs)?;

    Ok(ParsedSearchQuery {
        pattern: compiled_pattern,
        filters: WorkspaceSearchFilters { include, exclude },
        search_text: pattern,
        is_literal: !effective_use_regex,
        case_sensitive,
        whole_word,
        use_regex: effective_use_regex,
    })
}

fn compile_search_globset(patterns: &[String]) -> Result<Option<Arc<GlobSet>>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(pattern).map_err(|err| format!("invalid search glob `{pattern}`: {err}"))?,
        );
    }
    builder
        .build()
        .map(|set| Some(Arc::new(set)))
        .map_err(|err| format!("invalid search glob set: {err}"))
}

pub(crate) fn normalize_search_limit(limit: usize) -> usize {
    if limit == 0 {
        SEARCH_DEFAULT_RESULT_LIMIT
    } else {
        limit.min(SEARCH_MAX_RESULT_LIMIT)
    }
}

fn search_status_for_result(
    scope: SearchScopeProjection,
    result: &SearchBuildResult,
) -> SearchStatusProjection {
    if let Some(message) = &result.validation_error {
        return SearchStatusProjection {
            kind: SearchStatusKindProjection::ValidationError,
            message: message.clone(),
        };
    }

    if result.degraded_limited {
        return SearchStatusProjection {
            kind: SearchStatusKindProjection::DegradedLimited,
            message: if result.results.is_empty() {
                "Search was limited to degraded viewport content; no visible matches".to_string()
            } else {
                format!(
                    "Search was limited to degraded viewport content; {} visible matches",
                    result.results.len()
                )
            },
        };
    }

    if result.results.is_empty() {
        SearchStatusProjection {
            kind: SearchStatusKindProjection::NoResults,
            message: "No search results".to_string(),
        }
    } else {
        let scope_label = match scope {
            SearchScopeProjection::ActiveFile => "active file",
            SearchScopeProjection::Workspace => "workspace",
        };
        SearchStatusProjection {
            kind: SearchStatusKindProjection::Completed,
            message: format!("Found {} results in {scope_label}", result.results.len()),
        }
    }
}

fn build_search_projection(
    query_id: Option<String>,
    scope: SearchScopeProjection,
    query_label: String,
    result_limit: usize,
    status: SearchStatusProjection,
    result: SearchBuildResult,
) -> SearchProjection {
    SearchProjection {
        query_id,
        scope,
        query_label,
        status,
        results: result.results,
        result_limit,
        omitted_result_count: result.omitted_result_count,
        omitted_file_count: result.omitted_file_count,
        skipped_binary_count: result.skipped_binary_count,
        case_sensitive: result.case_sensitive,
        whole_word: result.whole_word,
        use_regex: result.use_regex,
        diagnostics: result.diagnostics,
        generated_at: TimestampMillis::now(),
        schema_version: 1,
    }
}

fn collect_search_results_for_text(input: SearchTextInput<'_>) {
    let mut absolute_line_start = 0_u64;
    for (line_number, line) in input.text.split_inclusive('\n').enumerate() {
        let line_text = line.trim_end_matches(&['\r', '\n'][..]);
        collect_search_results_for_line(SearchLineInput {
            query_id: input.query_id,
            pattern: input.pattern,
            scope: input.scope,
            workspace_id: input.workspace_id,
            buffer_id: input.buffer_id,
            file_id: input.file_id,
            file_path: input.file_path.clone(),
            line_number: line_number as u32,
            line_text,
            absolute_line_start,
            limit: input.limit,
            result: input.result,
        });
        absolute_line_start = absolute_line_start.saturating_add(line.len() as u64);
    }
}

fn count_chars_up_to(text: &str, byte_idx: usize) -> usize {
    text.get(..byte_idx)
        .map_or_else(|| text.chars().count(), |prefix| prefix.chars().count())
}

fn collect_search_results_for_line(input: SearchLineInput<'_>) {
    let matches = input.pattern.find_ranges(input.line_text);
    if matches.is_empty() {
        return;
    }

    for match_range in matches {
        let byte_start = match_range.start;
        let byte_end = match_range.end;
        let character_start = count_chars_up_to(input.line_text, byte_start) as u32;
        let character_end = count_chars_up_to(input.line_text, byte_end) as u32;
        let (snippet, snippet_truncated) = bounded_search_snippet(input.line_text);
        let row = SearchResultProjection {
            query_id: input.query_id.to_string(),
            scope: input.scope,
            workspace_id: input.workspace_id,
            buffer_id: input.buffer_id,
            file_id: input.file_id,
            file_path: input.file_path.clone(),
            line_number: input.line_number,
            range: ProtocolTextRange {
                start: TextCoordinate {
                    line: input.line_number,
                    character: character_start,
                    byte_offset: Some(input.absolute_line_start + byte_start as u64),
                    utf16_offset: Some(character_start as u64),
                },
                end: TextCoordinate {
                    line: input.line_number,
                    character: character_end,
                    byte_offset: Some(input.absolute_line_start + byte_end as u64),
                    utf16_offset: Some(character_end as u64),
                },
            },
            snippet,
            snippet_truncated,
            stale: false,
        };
        push_bounded_search_result(input.result, input.limit, row);
    }
}

fn push_bounded_search_result(
    result: &mut SearchBuildResult,
    limit: usize,
    row: SearchResultProjection,
) {
    if result.results.len() < limit {
        result.results.push(row);
    } else {
        result.omitted_result_count = result.omitted_result_count.saturating_add(1);
    }
}

fn bounded_search_snippet(line: &str) -> (String, bool) {
    if line.len() <= SEARCH_SNIPPET_LIMIT_BYTES {
        return (line.to_string(), false);
    }

    let mut end = SEARCH_SNIPPET_LIMIT_BYTES;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}...", &line[..end]), true)
}

impl AppComposition {
    /// Run bounded lexical search through app-owned editor/workspace authority.
    ///
    /// When a new query supersedes previous results, the existing
    /// `SearchResultProjection` rows are immediately marked `stale = true` so
    /// that callers can show de-emphasised results while the new search runs.
    pub fn run_search(
        &mut self,
        query_id: String,
        scope: SearchScopeProjection,
        query: String,
        limit: usize,
        options: SearchQueryOptions,
    ) -> Result<SearchProjection, AppCompositionError> {
        // Mark current results stale before running the new query so that any
        // caller holding a snapshot sees them de-emphasised.
        //
        // STALE-MARKER VISIBILITY LIMITATION (synchronous model): because
        // `run_search` is synchronous and single-threaded, the stale flag set
        // here is immediately overwritten by the `search_projection` assignment
        // at the end of this call in the same stack frame.  In the current
        // model there is therefore *zero practical visibility window* for the
        // stale state — no external observer can read the transient stale
        // snapshot.  The flag is meaningful only if/when an asynchronous
        // search path is introduced (where the caller could read the stale
        // projection between dispatching the new query and receiving its
        // result).  Keep the logic in place as a forwards-compatible hook, but
        // document that it has no effect today.
        let incoming_query_id = query_id.as_str();
        let current_is_different = self
            .search_projection
            .query_id
            .as_deref()
            .is_none_or(|prev| prev != incoming_query_id);
        if current_is_different && !self.search_projection.results.is_empty() {
            for row in &mut self.search_projection.results {
                row.stale = true;
            }
            self.search_projection.generated_at = TimestampMillis::now();
        }

        let result_limit = normalize_search_limit(limit);
        let query_label = query.trim().to_string();
        if query_label.is_empty() {
            self.search_projection = build_search_projection(
                Some(query_id),
                scope,
                query_label,
                result_limit,
                SearchStatusProjection {
                    kind: SearchStatusKindProjection::ValidationError,
                    message: "Search query is empty".to_string(),
                },
                SearchBuildResult::default(),
            );
            return Ok(self.search_projection.clone());
        }

        let result = match scope {
            SearchScopeProjection::ActiveFile => {
                self.run_active_file_search(&query_id, &query_label, result_limit, options)?
            }
            SearchScopeProjection::Workspace => {
                self.run_workspace_search(&query_id, &query_label, result_limit, options)?
            }
        };

        let status = search_status_for_result(scope, &result);
        self.search_projection = build_search_projection(
            Some(query_id),
            scope,
            query_label,
            result_limit,
            status,
            result,
        );
        Ok(self.search_projection.clone())
    }

    /// Cancel the projected search by query id.
    pub fn cancel_search(&mut self, query_id: String) -> SearchProjection {
        if self.search_projection.query_id.as_deref() == Some(query_id.as_str()) {
            self.search_projection.status = SearchStatusProjection {
                kind: SearchStatusKindProjection::Cancelled,
                message: "Search cancelled".to_string(),
            };
            self.search_projection.generated_at = TimestampMillis::now();
        }
        let projection = self.search_projection.clone();
        self.sync_search_palette_results();
        projection
    }

    fn run_active_file_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
        options: SearchQueryOptions,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let buffer_id = self.active_documents.require_active_buffer()?;
        let metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
            .ok_or(AppCompositionError::ActiveFileMissing)?;

        if matches!(self.editor.buffer_mode(buffer_id)?, BufferMode::Degraded) {
            return self.run_degraded_active_file_search(
                query_id, query, limit, buffer_id, metadata, options,
            );
        }

        let text = self.editor.text(buffer_id)?;
        let parsed = match parse_search_query(query, options) {
            Ok(parsed) => parsed,
            Err(message) => {
                return Ok(SearchBuildResult {
                    validation_error: Some(message),
                    ..SearchBuildResult::default()
                });
            }
        };
        let mut result = SearchBuildResult {
            case_sensitive: parsed.case_sensitive,
            whole_word: parsed.whole_word,
            use_regex: parsed.use_regex,
            ..SearchBuildResult::default()
        };
        collect_search_results_for_text(SearchTextInput {
            query_id,
            pattern: &parsed.pattern,
            scope: SearchScopeProjection::ActiveFile,
            workspace_id: Some(metadata.identity.workspace_id),
            buffer_id: Some(buffer_id),
            file_id: Some(metadata.identity.file_id),
            file_path: Some(metadata.identity.canonical_path),
            text,
            limit,
            result: &mut result,
        });
        Ok(result)
    }

    fn run_degraded_active_file_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
        buffer_id: BufferId,
        metadata: ActiveFileMetadata,
        options: SearchQueryOptions,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let scroll = self.active_documents.viewport_scroll_for(buffer_id);
        let viewport = self
            .editor
            .viewport_projection(legion_protocol::EditorViewportRequest {
                buffer_id,
                scroll,
                dimensions: legion_protocol::ViewportDimensions {
                    width_px: 800,
                    height_px: 384,
                },
            })?;
        let parsed = match parse_search_query(query, options) {
            Ok(parsed) => parsed,
            Err(message) => {
                return Ok(SearchBuildResult {
                    validation_error: Some(message),
                    ..SearchBuildResult::default()
                });
            }
        };
        let mut result = SearchBuildResult {
            degraded_limited: true,
            diagnostics: vec![
                "Active-file search is limited to the visible viewport in degraded mode"
                    .to_string(),
            ],
            case_sensitive: parsed.case_sensitive,
            whole_word: parsed.whole_word,
            use_regex: parsed.use_regex,
            ..SearchBuildResult::default()
        };

        for slice in &viewport.line_slices {
            collect_search_results_for_line(SearchLineInput {
                query_id,
                pattern: &parsed.pattern,
                scope: SearchScopeProjection::ActiveFile,
                workspace_id: Some(metadata.identity.workspace_id),
                buffer_id: Some(buffer_id),
                file_id: Some(metadata.identity.file_id),
                file_path: Some(metadata.identity.canonical_path.clone()),
                line_number: slice.line_number,
                line_text: &slice.visible_text,
                absolute_line_start: slice.byte_range.start,
                limit,
                result: &mut result,
            });
        }

        Ok(result)
    }

    fn run_workspace_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
        options: SearchQueryOptions,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let workspace_id = self.active_documents.require_workspace_id()?;
        let parsed = match parse_search_query(query, options) {
            Ok(parsed) => parsed,
            Err(message) => {
                return Ok(SearchBuildResult {
                    validation_error: Some(message),
                    ..SearchBuildResult::default()
                });
            }
        };
        let mut result = SearchBuildResult {
            case_sensitive: parsed.case_sensitive,
            whole_word: parsed.whole_word,
            use_regex: parsed.use_regex,
            ..SearchBuildResult::default()
        };
        let backend_query = WorkspaceSearchQuery {
            workspace_id,
            pattern: parsed.pattern,
            search_text: parsed.search_text,
            filters: parsed.filters,
            result_limit: limit,
            batch_size: 32,
            use_indexed_backend: self.settings.indexed_workspace_search_enabled
                && parsed.is_literal,
        };

        let report = self.workspace.search_workspace_stream(
            backend_query,
            |batch: WorkspaceSearchBatch| {
                result.omitted_file_count = result
                    .omitted_file_count
                    .saturating_add(batch.omitted_file_count);
                result.omitted_result_count = result
                    .omitted_result_count
                    .saturating_add(batch.omitted_hit_count);
                result.diagnostics.extend(batch.diagnostics);
                for hit in batch.hits {
                    let byte_start = hit.byte_range.start as usize;
                    let byte_end = hit.byte_range.end as usize;
                    let character_start = count_chars_up_to(&hit.line_text, byte_start) as u32;
                    let character_end = count_chars_up_to(&hit.line_text, byte_end) as u32;
                    result.results.push(SearchResultProjection {
                        query_id: query_id.to_string(),
                        scope: SearchScopeProjection::Workspace,
                        workspace_id: Some(workspace_id),
                        buffer_id: self.editor.buffer_for_file(workspace_id, hit.file_id),
                        file_id: Some(hit.file_id),
                        file_path: Some(hit.canonical_path),
                        line_number: hit.line_number,
                        range: ProtocolTextRange {
                            start: TextCoordinate {
                                line: hit.line_number,
                                character: character_start,
                                byte_offset: Some(hit.byte_range.start),
                                utf16_offset: Some(character_start as u64),
                            },
                            end: TextCoordinate {
                                line: hit.line_number,
                                character: character_end,
                                byte_offset: Some(hit.byte_range.end),
                                utf16_offset: Some(character_end as u64),
                            },
                        },
                        snippet: hit.snippet,
                        snippet_truncated: hit.snippet_truncated,
                        stale: false,
                    });
                }
                true
            },
        )?;
        // Propagate binary-skip count from the workspace report so it is
        // visible to the user via SearchProjection.skipped_binary_count.
        result.skipped_binary_count = report.skipped_binary_count;

        Ok(result)
    }
}
