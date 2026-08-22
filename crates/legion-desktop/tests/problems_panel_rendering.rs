//! The Problems panel as a user actually meets it: rendered, then clicked.
//!
//! Diagnostics were covered in two places and neither was the panel. The
//! runtime harness (`diagnostics_harness.rs`) proves `publishDiagnostics`
//! reaches `language_tooling_projection.problems`, and `keyboard_nav.rs`
//! proves `ProblemNext`/`ProblemActivate` move an index and return `Opened`.
//! Between the projection and the outcome enum sits the thing the user sees —
//! a row in a panel, with a label, at coordinates, that can be clicked — and
//! nothing rendered a frame to check it was there.
//!
//! That gap is not academic. `PR-LANG-001` in the product readiness ledger
//! names "renderer-backed diagnostics panel UX evidence" as one of its two
//! remaining promotion blockers, and a projection test cannot supply it by
//! construction: a panel that never draws its rows, or draws them where no
//! pointer can reach, passes every assertion in both existing suites.
//!
//! So these tests drive the real `DesktopEframeApp`, read the real
//! accessibility tree, and click real centres. `t4_problem_activate_happy_path`
//! is also thinner than it looks — it asserts `DesktopWorkflowOutcome::Opened`
//! and not *which* file at *which* line — so the click test here asserts the
//! destination as well as the fact that something opened.

use std::path::{Path, PathBuf};

mod common;
use common::{TempWorkspace, click_at, full_frame_input, rendered_text};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

/// Centre of the clickable node whose label *contains* `needle`.
///
/// `common::clickable_center` matches labels exactly, which no problem row can
/// satisfy: a row reads `severity path:line message`, the path is an absolute
/// temp-directory path, and `trim_middle` replaces the middle of anything over
/// 110 characters with an ellipsis. Head and tail survive that, so a substring
/// taken from either end still identifies the row.
fn clickable_center_containing(output: &egui::FullOutput, needle: &str) -> Option<egui::Pos2> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label().is_some_and(|label| label.contains(needle))
                && node.supports_action(egui::accesskit::Action::Click))
            .then(|| node.bounds())
            .flatten()
        })
        .map(|bounds| {
            egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            )
        })
}

/// Whether any text the frame exposes contains `needle`.
fn frame_shows(output: &egui::FullOutput, needle: &str) -> bool {
    rendered_text(output)
        .iter()
        .any(|text| text.contains(needle))
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// The `file:` URI form the language server would send for `path`.
///
/// Built the way `keyboard_nav.rs` builds it, because that form is what makes
/// the projection backfill `problem.path` — and a problem without a path
/// renders as plain text with no click sense at all, which would make every
/// click assertion below fail for a reason that has nothing to do with the
/// panel.
fn lsp_uri(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    format!(
        "file:///{}",
        text.replace('\\', "/").trim_start_matches('/')
    )
}

/// Publish `count` diagnostics against the open buffer, one per line from 0.
fn publish_diagnostics(runtime: &mut DesktopRuntime, path: &Path, messages: &[(u32, &str)]) {
    let buffer_id = runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a file must be open before diagnostics can be published against it");
    let diagnostics: Vec<serde_json::Value> = messages
        .iter()
        .map(|(line, message)| {
            serde_json::json!({
                "range": {
                    "start": { "line": line, "character": 0 },
                    "end": { "line": line, "character": 4 }
                },
                "severity": 1,
                "source": "rustc",
                "code": "E0308",
                "message": message
            })
        })
        .collect();
    let params = serde_json::json!({
        "uri": lsp_uri(path),
        "diagnostics": diagnostics,
    });
    runtime
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, true, None)
        .expect("publishDiagnostics should project into the shell snapshot");
}

/// A workspace file with twenty lines, open, carrying `messages` as diagnostics.
fn app_with_diagnostics(
    workspace: &TempWorkspace,
    messages: &[(u32, &str)],
) -> (DesktopEframeApp, PathBuf) {
    // Twenty lines so any line a test names actually exists in the file --
    // `OpenPathAtPosition` against a line past the end would clamp, and a
    // clamped cursor would make a wrong-line bug look like a pass.
    let body: String = (0..20).map(|i| format!("// line {i}\n")).collect();
    let file = workspace.write("src/main.rs", &body);
    let mut runtime = open_runtime(workspace.path());
    runtime
        .handle_action(DesktopAction::OpenPathText(
            file.to_string_lossy().into_owned(),
        ))
        .expect("opening a workspace file should succeed");
    publish_diagnostics(&mut runtime, &file, messages);
    (DesktopEframeApp::new(runtime), file)
}

/// Render a frame, click the PROBLEMS tab, and return the frame that shows it.
///
/// The bottom console opens on TERMINAL, so every test here has to make the
/// same switch a user would. Doing it through the rendered tab rather than by
/// setting the field is the point: if the tab is not clickable, the panel is
/// not reachable, and that is a finding rather than a setup inconvenience.
fn show_problems_panel(app: &mut DesktopEframeApp, expected_count: usize) -> egui::FullOutput {
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let label = format!("PROBLEMS ({expected_count})");
    let tab = clickable_center_containing(&primed, &label).unwrap_or_else(|| {
        panic!(
            "the bottom console should offer a clickable `{label}` tab; rendered text was {:?}",
            rendered_text(&primed)
        )
    });
    click_at(app, tab)
}

#[test]
fn a_projected_diagnostic_is_visible_in_the_rendered_problems_panel() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, file) = app_with_diagnostics(&workspace, &[(5, "borrow of moved value")]);

    let output = show_problems_panel(&mut app, 1);

    // What the row actually says, which is not what the server said.
    //
    // `legion_lsp::redacted_diagnostic_message` replaces every message with a
    // per-severity placeholder before it reaches this projection, so a panel
    // full of rustc errors reads "Error <path>:<line> LSP error diagnostic"
    // and nothing else. That is deliberate and guarded --
    // `language_tooling_workflow::language_tooling_ingests_lsp_diagnostic_projection_and_preserves_lexical_rows`
    // feeds a diagnostic whose text embeds `SECRET_SOURCE_BODY` and asserts it
    // does not survive, because a server's message can quote the source it is
    // complaining about and ADR-0028 keeps raw source out of these records.
    //
    // It is also why the panel cannot yet do its job: severity is already an
    // icon, location is already a path, and the one field that would tell a
    // reader whether this is a missing semicolon or a borrow-check failure is
    // the one that is blank. Resolving that is a product decision about how
    // much of a server's prose may be shown locally -- not something to settle
    // by deleting the guard -- so this test pins the behaviour as it stands
    // rather than pretending either half of the conflict away.
    assert!(
        frame_shows(&output, "LSP error diagnostic"),
        "the row must carry the redacted severity text the projection supplies; \
         rendered text was {:?}",
        rendered_text(&output)
    );
    assert!(
        !frame_shows(&output, "borrow of moved value"),
        "the server's own message must not reach the row while the redaction \
         stands -- if this fails the policy changed and this test should say so \
         out loud; rendered text was {:?}",
        rendered_text(&output)
    );
    // The code is the only field left that distinguishes one error from
    // another once the message has been replaced by its severity.
    assert!(
        frame_shows(&output, "E0308"),
        "the row must carry the diagnostic code the server sent; rendered text was {:?}",
        rendered_text(&output)
    );
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .expect("the workspace file should have a name");
    assert!(
        frame_shows(&output, &format!("{name}:5")),
        "the row must name the file and the line the diagnostic is on -- a message \
         with no location is not something a reader can act on; rendered text was {:?}",
        rendered_text(&output)
    );
    // The canonical path carries Windows' extended-length prefix. The
    // breadcrumb and status bar have stripped it since `path_display` was
    // written; this row did not, so the panel named files in a shape nobody
    // types and nobody recognises.
    assert!(
        !frame_shows(&output, r"\\?\"),
        "no rendered row may show the verbatim path prefix; rendered text was {:?}",
        rendered_text(&output)
    );
}

#[test]
fn clicking_a_rendered_problem_row_opens_the_file_at_its_line() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, file) = app_with_diagnostics(&workspace, &[(7, "mismatched types")]);

    // Close the file the diagnostic points at, so opening it is something the
    // click has to do rather than something that was already true. Without
    // this the cursor assertion is the only load-bearing one, and a row that
    // navigated nowhere would still find the right buffer active.
    let open_buffer = app
        .runtime_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("the diagnostic's file should be open before the tab is closed");
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::CloseTab {
            buffer_id: open_buffer,
        })
        .expect("closing the tab should succeed");

    let panel = show_problems_panel(&mut app, 1);
    let row = clickable_center_containing(&panel, "main.rs:7").unwrap_or_else(|| {
        panic!(
            "the problem row must be clickable where it is drawn; rendered text was {:?}",
            rendered_text(&panel)
        )
    });

    let _ = click_at(&mut app, row);

    let snapshot = app.runtime_snapshot();
    let opened = snapshot
        .active_buffer_projection
        .file_path
        .as_ref()
        .map(|path| path.0.clone())
        .unwrap_or_default();
    assert!(
        opened.ends_with(
            file.file_name()
                .and_then(|name| name.to_str())
                .expect("the workspace file should have a name")
        ),
        "clicking the row must open the file the diagnostic is in, got {opened:?}"
    );
    assert_eq!(
        snapshot
            .active_buffer_projection
            .viewport
            .as_ref()
            .map(|viewport| viewport.cursor.line),
        Some(7),
        "clicking the row must land the cursor on the diagnostic's line -- opening \
         the right file at the top of it is the bug this assertion exists to catch"
    );
}

#[test]
fn the_problems_tab_counts_the_problems_behind_it() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, _file) = app_with_diagnostics(
        &workspace,
        &[
            (1, "first problem"),
            (2, "second problem"),
            (3, "third problem"),
        ],
    );

    // Read the count off the closed tab: somebody working in the terminal has
    // to be able to see that problems exist without switching to look.
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        frame_shows(&primed, "PROBLEMS (3)"),
        "the tab must carry the number of problems while it is closed; rendered text was {:?}",
        rendered_text(&primed)
    );
}

#[test]
fn clearing_the_diagnostics_empties_the_rendered_panel() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, file) = app_with_diagnostics(&workspace, &[(3, "unresolved import")]);

    let panel = show_problems_panel(&mut app, 1);
    assert!(
        frame_shows(&panel, "main.rs:3"),
        "the problem must be on screen before the clear is meaningful"
    );

    publish_diagnostics(app.runtime_mut_for_test(), &file, &[]);
    let cleared = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        !frame_shows(&cleared, "main.rs:3"),
        "a cleared diagnostic must leave the panel -- a row that outlives the \
         problem sends the reader to code that is already correct; rendered text was {:?}",
        rendered_text(&cleared)
    );
    assert!(
        frame_shows(&cleared, "No problems"),
        "an empty panel must say so rather than render blank; rendered text was {:?}",
        rendered_text(&cleared)
    );
    assert!(
        frame_shows(&cleared, "PROBLEMS (0)"),
        "the tab count must fall with the rows; rendered text was {:?}",
        rendered_text(&cleared)
    );
}

/// Whether the frame marks a row containing `needle` as the selected one.
///
/// Read from the accessibility tree's own state rather than from the row's
/// text. A marker written into the label is invisible to everything that asks
/// a control what it is, which is the defect this reads around. egui reports a
/// selectable widget's selection as `toggled`, so that is what to ask for.
fn row_is_selected(output: &egui::FullOutput, needle: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label().is_some_and(|label| label.contains(needle))
                    && node.toggled() == Some(egui::accesskit::Toggled::True)
            })
        })
}

#[test]
fn the_rendered_panel_marks_the_keyboard_focused_row() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, _file) =
        app_with_diagnostics(&workspace, &[(1, "first problem"), (2, "second problem")]);

    let panel = show_problems_panel(&mut app, 2);
    assert!(
        row_is_selected(&panel, "main.rs:1"),
        "the first row must be the selected one before anything moves it; \
         rendered text was {:?}",
        rendered_text(&panel)
    );

    app.runtime_mut_for_test()
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext should succeed");
    let moved = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        row_is_selected(&moved, "main.rs:2"),
        "ProblemNext must move the selection in the frame the user is looking at \
         -- an index that advances invisibly is keyboard navigation nobody can \
         follow; rendered text was {:?}",
        rendered_text(&moved)
    );
    assert!(
        !row_is_selected(&moved, "main.rs:1"),
        "only one row may be selected; rendered text was {:?}",
        rendered_text(&moved)
    );
}

/// A language read must not empty the Problems panel.
///
/// `ingest_language_read` rebuilt the language projection from empty whenever a
/// read named a different buffer, and `problems` went with it. The panel is a
/// workspace-wide list -- `ingest_lsp_diagnostics` retains rows for every other
/// file on purpose -- so one hover cleared diagnostics that had nothing to do
/// with it, and nothing republished them until that file's server spoke again.
#[test]
fn a_language_read_leaves_the_problems_alone() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, _file) = app_with_diagnostics(&workspace, &[(4, "unresolved import")]);

    // The shell issues reads on its own -- a hover debounce fires and the
    // index leg answers it -- and that read is what used to clear the list.
    // One buffer is the whole scenario: the defect needed no second file.
    let panel = show_problems_panel(&mut app, 1);
    assert!(
        frame_shows(&panel, "main.rs:4"),
        "the problem must be listed before the read that used to clear it"
    );

    for _ in 0..4 {
        let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));
    }

    let after = app.runtime_snapshot();
    assert_eq!(
        after.language_tooling_projection.problems.len(),
        1,
        "the diagnostic must survive the reads the shell issues on its own; \
         status was {:?}",
        after.language_tooling_projection.status_message
    );
}

/// The selection stays on its problem when the list around it changes.
///
/// The Problems panel'"'"'s selection was a bare index for three rounds of review,
/// and each fix left a residue: removing a sort stopped one reorder, splicing
/// replacements in place stopped another, and both still moved rows whenever
/// the number of rows above them changed. A read that finds one fewer lexical
/// diagnostic than last time shifts everything behind it by one, and the
/// highlight lands on a problem the reader never chose -- which the next
/// activation then opens.
///
/// So the selection is stored as the identity of the problem it is on and the
/// index is derived. This drives it through the rendered panel: select the
/// second row, publish a different set of diagnostics that moves it, and assert
/// the same row is still the selected one.
#[test]
fn the_selection_follows_its_problem_when_rows_move() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, file) = app_with_diagnostics(
        &workspace,
        &[
            (1, "first problem"),
            (2, "second problem"),
            (3, "third problem"),
        ],
    );

    let panel = show_problems_panel(&mut app, 3);
    assert!(
        row_is_selected(&panel, "main.rs:1"),
        "the first row starts selected; rendered text was {:?}",
        rendered_text(&panel)
    );

    // Move to the row for line 2 and remember what the reader is looking at.
    app.runtime_mut_for_test()
        .handle_action(DesktopAction::ProblemNext)
        .expect("ProblemNext should succeed");
    let moved = app.run_headless_full_frame(full_frame_input(Vec::new()));
    assert!(
        row_is_selected(&moved, "main.rs:2"),
        "the selection should be on line 2 before the list changes; rendered \
         text was {:?}",
        rendered_text(&moved)
    );

    // Republish without the first diagnostic. Every row behind it moves up one,
    // which is exactly the shift a positional selection cannot survive.
    publish_diagnostics(
        app.runtime_mut_for_test(),
        &file,
        &[(2, "second problem"), (3, "third problem")],
    );
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        row_is_selected(&after, "main.rs:2"),
        "the selection must stay on the problem it was on, not the position it \
         held -- a highlight that slides onto another diagnostic opens a file \
         nobody chose on the next activation; rendered text was {:?}",
        rendered_text(&after)
    );
    assert!(
        !row_is_selected(&after, "main.rs:3"),
        "only the problem the reader selected may be selected; rendered text \
         was {:?}",
        rendered_text(&after)
    );
}

/// The selection nobody chose is still the reader's selection.
///
/// Row 0 renders as selected the moment diagnostics appear. Until something is
/// remembered about it there is nothing for the resolver to hold on to, so a
/// republish that inserts a diagnostic *ahead* of it leaves the index at 0, the
/// highlight moves to the new row, and the next activation opens it. Nothing
/// the reader did caused that and nothing on screen said it happened.
///
/// The previous selection test moves to the second row before mutating the
/// list, so it never exercised this: it proved a chosen selection is durable
/// while the default one still was not.
#[test]
fn the_default_selection_survives_a_diagnostic_arriving_ahead_of_it() {
    let workspace = TempWorkspace::new("legion_desktop_problems_panel");
    let (mut app, file) =
        app_with_diagnostics(&workspace, &[(5, "first problem"), (9, "second problem")]);

    // No navigation at all: this is the selection the reader was given.
    let panel = show_problems_panel(&mut app, 2);
    assert!(
        row_is_selected(&panel, "main.rs:5"),
        "the first row starts selected; rendered text was {:?}",
        rendered_text(&panel)
    );

    // The server republishes with a new diagnostic ahead of the selected one.
    publish_diagnostics(
        app.runtime_mut_for_test(),
        &file,
        &[
            (2, "newly arrived problem"),
            (5, "first problem"),
            (9, "second problem"),
        ],
    );
    let after = app.run_headless_full_frame(full_frame_input(Vec::new()));

    assert!(
        row_is_selected(&after, "main.rs:5"),
        "the default selection must stay on its problem when one arrives ahead \
         of it -- a highlight that slides onto a row the reader has never seen \
         opens it on the next activation; rendered text was {:?}",
        rendered_text(&after)
    );
    assert!(
        !row_is_selected(&after, "main.rs:2"),
        "the newly arrived diagnostic must not steal the selection; rendered \
         text was {:?}",
        rendered_text(&after)
    );
}
