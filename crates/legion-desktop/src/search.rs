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
    /// Helpful empty-result guidance when a search completed without matches.
    pub empty_state: Option<String>,
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
        let query = projection.query_label.trim();
        let mut status_rows = vec![match projection.status.kind {
            SearchStatusKindProjection::Idle => {
                format!("Enter a search term to find text in the {scope}.")
            }
            SearchStatusKindProjection::Running => "Searching…".to_string(),
            SearchStatusKindProjection::Completed | SearchStatusKindProjection::NoResults => {
                "Search finished.".to_string()
            }
            SearchStatusKindProjection::Cancelled => "Search stopped.".to_string(),
            SearchStatusKindProjection::ValidationError => {
                "Check the search term and try again.".to_string()
            }
            SearchStatusKindProjection::DegradedLimited => {
                "Search used the available text and may have missed matches.".to_string()
            }
            SearchStatusKindProjection::Error => "Search could not finish. Try again.".to_string(),
        }];
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
        let result_rows = projection
            .results
            .iter()
            .map(|row| {
                let path = row
                    .file_path
                    .as_ref()
                    .map(|path| path.0.as_str())
                    .unwrap_or("Current file");
                let truncated = if row.snippet_truncated {
                    " preview shortened"
                } else {
                    ""
                };
                // Stale results (superseded by a newer query) are tagged so
                // the renderer can apply a de-emphasised visual treatment.
                let stale_tag = if row.stale { " [outdated]" } else { "" };
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

        let diagnostic_rows = projection.diagnostics.clone();
        let empty_state =
            (projection.status.kind == SearchStatusKindProjection::NoResults).then(|| {
                match projection.scope {
                    SearchScopeProjection::ActiveFile => {
                        "No matches. Try another term or search the workspace.".to_string()
                    }
                    SearchScopeProjection::Workspace => {
                        "No matches. Try another term or search the active file.".to_string()
                    }
                }
            });

        // Build a compact option tag reflecting *non-default* active toggles.
        // Only emit a tag for options that deviate from the plain default:
        //   [Cc]  — case-sensitive mode explicitly active
        //   [W]   — whole-word matching active
        //   [.*]  — regex mode active
        // Case-insensitive (the plain user default) produces no tag so that
        // ordinary searches keep a clean header.
        let mut option_tags = String::new();
        if !query.is_empty() && projection.case_sensitive {
            option_tags.push_str("[Cc]");
        }
        if !query.is_empty() && projection.whole_word {
            option_tags.push_str("[W]");
        }
        if !query.is_empty() && projection.use_regex {
            option_tags.push_str("[.*]");
        }
        let base_header = if query.is_empty() {
            format!("Search the {scope}")
        } else if projection.status.kind == SearchStatusKindProjection::Completed {
            let count = projection.results.len();
            format!(
                "{count} match{} in the {scope} for \"{query}\"",
                if count == 1 { "" } else { "es" }
            )
        } else {
            format!("Search the {scope} for \"{query}\"")
        };
        let header = if option_tags.is_empty() {
            base_header
        } else {
            format!("{base_header} {option_tags}")
        };

        Self {
            header,
            status_rows,
            result_rows,
            empty_state,
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
