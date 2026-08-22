//! The rules governing the shared Problems list.
//!
//! Every diagnostic a reader sees passes through here. The list spans the
//! workspace rather than the active buffer, which is what makes these rules
//! worth stating in one place: two producers write to it (the LSP publish path
//! and the index-backed read leg), the desktop shell holds a keyboard selection
//! as a bare index into it, and quick fixes are derived from a capped view of
//! it. Each of those has been the source of a defect where one producer'"'"'s
//! notion of the list contradicted another'"'"'s.
//!
//! Extracted from `lib.rs` under the roadmap'"'"'s extract-before-modify rule.

use legion_protocol::{
    FileId, LanguageProblemProjection, LanguageQuickFixProjection, LanguageToolingProjection,
    RedactionHint, WorkspaceId,
};

use crate::bounded_label;

/// Quick fixes for `problems`, with `active`'s rows offered first.
///
/// `language_quick_fixes_for_problems` takes the first 50 rows. While the
/// problems list held one file that cap was about one file's diagnostics;
/// now that it spans the workspace the cap decides *whose* fixes exist, and
/// the only file whose fixes a reader can act on is the one they are in. A
/// crowded workspace would otherwise leave the file under the cursor with
/// none.
///
/// The list itself is never reordered -- the Problems panel's keyboard
/// selection indexes into it -- so this builds a separate view.
pub(crate) fn language_quick_fixes_prioritizing(
    problems: &[LanguageProblemProjection],
    active: FileId,
) -> Vec<LanguageQuickFixProjection> {
    let mut ordered = Vec::with_capacity(problems.len());
    ordered.extend(
        problems
            .iter()
            .filter(|problem| problem.file_id == Some(active))
            .cloned(),
    );
    ordered.extend(
        problems
            .iter()
            .filter(|problem| problem.file_id != Some(active))
            .cloned(),
    );
    language_quick_fixes_for_problems(&ordered)
}

pub(crate) fn language_quick_fixes_for_problems(
    problems: &[LanguageProblemProjection],
) -> Vec<LanguageQuickFixProjection> {
    problems
        .iter()
        .take(50)
        .enumerate()
        .map(|(index, problem)| {
            let code_label = problem
                .code_label
                .clone()
                .unwrap_or_else(|| "diagnostic".to_string());
            LanguageQuickFixProjection {
                action_id: language_quick_fix_action_id(index, problem),
                title: format!(
                    "Prepare code action for {}",
                    bounded_label(code_label.clone(), 64)
                ),
                kind_label: "quickfix.diagnostic".to_string(),
                problem_code_label: problem.code_label.clone(),
                problem_range: problem.range,
                severity: problem.severity,
                source_label: problem.source_label.clone(),
                proposal_id: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }
        })
        .collect()
}

/// The projection a language read builds on when it names a different buffer.
///
/// Everything the previous buffer's reads produced is dropped -- its hover, its
/// completions, its outline, its call hierarchy -- because none of it describes
/// the buffer this result is about, and showing one file's outline against
/// another is worse than showing none.
///
/// `problems` is the exception, and deliberately so. The Problems panel is a
/// workspace-wide list: `ingest_lsp_diagnostics` curates it that way, retaining
/// every row that belongs to a *different* file and replacing only the rows for
/// the file whose diagnostics just arrived. Dropping the list here contradicted
/// that directly -- a single hover emptied the panel of every diagnostic in the
/// workspace, including the ones the hover had nothing to do with, and nothing
/// republished them until the server happened to send that file's diagnostics
/// again. Quick fixes are rebuilt from the rows that survive, because a fix
/// offered for a problem that is no longer listed is an action with no subject.
///
/// "Workspace-wide" is the whole of the exception, and `workspace` is what
/// bounds it. Opening workspace B keeps this same workflow, so B's first read
/// arrives here holding A's rows; carrying them over would republish A's
/// diagnostics under B, and a problem row records no workspace of its own --
/// so the per-file replacement that normally retires a stale row could never
/// find them, and clicking one would send the reader at a path outside the
/// workspace they are in. A different workspace is a different list.
pub(crate) fn language_projection_for_new_identity(
    previous: &LanguageToolingProjection,
    workspace: WorkspaceId,
) -> LanguageToolingProjection {
    let mut projection = LanguageToolingProjection::empty();
    projection.operations = previous.operations.clone();
    projection.cancellation_count = previous.cancellation_count;
    projection.stale_result_count = if previous.buffer_id.is_some() {
        previous.stale_result_count.saturating_add(1)
    } else {
        previous.stale_result_count
    };
    if previous
        .workspace_id
        .is_none_or(|previous| previous == workspace)
    {
        // Index-owned rows are dropped here for the same reason the read leg
        // drops them: nothing retracts them, so a row kept past the buffer it
        // was computed for is a row that can never leave.
        projection.problems = previous
            .problems
            .iter()
            .filter(|problem| problem.source_label.as_deref() != Some("legion-index"))
            .cloned()
            .collect();
        projection.quick_fixes = language_quick_fixes_for_problems(&projection.problems);
    }
    projection
}

pub(crate) fn language_quick_fix_action_id(
    index: usize,
    problem: &LanguageProblemProjection,
) -> String {
    let code = problem.code_label.as_deref().unwrap_or("diagnostic");
    let safe_code = sanitized_action_component(code, 64);
    let (line, character) = problem
        .range
        .map(|range| (range.start.line, range.start.character))
        .unwrap_or((0, 0));
    format!("quickfix:{safe_code}:{line}:{character}:{index}")
}

pub(crate) fn sanitized_action_component(value: &str, limit: usize) -> String {
    let sanitized = value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | ':') {
                Some(character)
            } else if character.is_ascii_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .take(limit)
        .collect::<String>();
    if sanitized.is_empty() {
        "diagnostic".to_string()
    } else {
        sanitized
    }
}
