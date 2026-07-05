//! Desktop search display helpers.

use legion_ui::{SearchProjection, SearchScopeProjection, SearchStatusKindProjection};

/// Testable search display model derived only from `SearchProjection`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSearchViewModel {
    /// Header row describing query and scope.
    pub header: String,
    /// Status rows for idle, no-results, errors, cancellation, and degraded limits.
    pub status_rows: Vec<String>,
    /// Bounded result rows.
    pub result_rows: Vec<String>,
    /// Diagnostic rows for skipped or limited files/results.
    pub diagnostic_rows: Vec<String>,
}

impl DesktopSearchViewModel {
    /// Builds a desktop search view model without accessing app/editor/workspace state.
    pub fn from_projection(projection: &SearchProjection) -> Self {
        let scope = match projection.scope {
            SearchScopeProjection::ActiveFile => "active file",
            SearchScopeProjection::Workspace => "workspace",
        };
        let query = if projection.query_label.is_empty() {
            "<empty>"
        } else {
            projection.query_label.as_str()
        };
        let mut status_rows = vec![format!(
            "{:?}: {}",
            projection.status.kind, projection.status.message
        )];
        if projection.omitted_result_count > 0 {
            status_rows.push(format!(
                "{} results omitted by limit {}",
                projection.omitted_result_count, projection.result_limit
            ));
        }
        if projection.omitted_file_count > 0 {
            status_rows.push(format!("{} files skipped", projection.omitted_file_count));
        }
        if projection.skipped_binary_count > 0 {
            status_rows.push(format!(
                "{} binary files skipped",
                projection.skipped_binary_count
            ));
        }
        if projection.status.kind == SearchStatusKindProjection::Idle {
            status_rows.push("Search idle".to_string());
        }

        let result_rows = projection
            .results
            .iter()
            .map(|row| {
                let path = row
                    .file_path
                    .as_ref()
                    .map(|path| path.0.as_str())
                    .unwrap_or("<active buffer>");
                let truncated = if row.snippet_truncated {
                    " truncated"
                } else {
                    ""
                };
                // Stale results (superseded by a newer query) are tagged so
                // the renderer can apply a de-emphasised visual treatment.
                let stale_tag = if row.stale { " [stale]" } else { "" };
                format!(
                    "{}:{}:{}{}{} {}",
                    path,
                    row.line_number + 1,
                    row.range.start.character + 1,
                    truncated,
                    stale_tag,
                    normalize_snippet(&row.snippet)
                )
            })
            .collect::<Vec<_>>();

        let mut diagnostic_rows = projection.diagnostics.clone();
        if projection.status.kind == SearchStatusKindProjection::NoResults
            && projection.diagnostics.is_empty()
        {
            diagnostic_rows.push("No results".to_string());
        }

        // Build a compact option tag reflecting active toggles, e.g. "[Cc][W][.*]".
        let mut option_tags = String::new();
        if projection.case_sensitive {
            option_tags.push_str("[Cc]");
        } else {
            option_tags.push_str("[ci]");
        }
        if projection.whole_word {
            option_tags.push_str("[W]");
        }
        if projection.use_regex {
            option_tags.push_str("[.*]");
        }
        let header = if option_tags.is_empty() {
            format!("Search {scope}: {query}")
        } else {
            format!("Search {scope}: {query} {option_tags}")
        };

        Self {
            header,
            status_rows,
            result_rows,
            diagnostic_rows,
        }
    }
}

/// Normalize a search snippet for single-line row display by replacing control
/// characters (newlines, carriage returns, tabs, etc.) with spaces so embedded
/// control characters cannot break the one-line `path:line:col snippet` format.
fn normalize_snippet(snippet: &str) -> String {
    snippet
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}
