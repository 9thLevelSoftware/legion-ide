//! Call hierarchy must be reachable by a person, not just by a match arm.
//!
//! The app layer answers "who calls this" and "what does this call" and has
//! done since the routing landed, but nothing on the outside could ask: there
//! was no `DesktopAction`, no bridge translation and no key. That is the exact
//! failure mode the intent-reachability gate exists for — a complete, tested
//! capability with no door — so these tests assert the door, not the room.
//!
//! Each test drives the real `DesktopRuntime` (and, for the keys, the real
//! headless `eframe` frame) and asserts on the operation the app recorded.
//! `LanguageToolingOperationKind::IncomingCalls` / `OutgoingCalls` are only
//! ever pushed by the call-hierarchy request path, so an assertion on them
//! cannot be satisfied by a neighbouring language read that happens to run.

use std::path::{Path, PathBuf};

mod common;
use common::TempWorkspace;

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{LanguageToolingOperationKind, TextCoordinate};
use legion_ui::{CommandDispatchIntent, ShellProjectionSnapshot};

/// A workspace holding one Rust file with a call in it, plus that file's path.
fn workspace_with_calls(prefix: &'static str) -> (TempWorkspace, PathBuf) {
    let workspace = TempWorkspace::new(prefix);
    let source = workspace.write(
        "src/main.rs",
        "fn helper() -> u32 {\n    7\n}\n\nfn main() {\n    let value = helper();\n    println!(\"{value}\");\n}\n",
    );
    (workspace, source)
}

fn open_runtime(root: &Path, source: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open the workspace with the file active")
}

fn caret() -> TextCoordinate {
    TextCoordinate {
        line: 5,
        character: 16,
        byte_offset: Some(60),
        utf16_offset: Some(60),
    }
}

/// Every call-hierarchy operation the app has recorded, oldest first.
fn call_hierarchy_operations(
    snapshot: &ShellProjectionSnapshot,
) -> Vec<LanguageToolingOperationKind> {
    snapshot
        .language_tooling_projection
        .operations
        .iter()
        .map(|operation| operation.kind)
        .filter(|kind| {
            matches!(
                kind,
                LanguageToolingOperationKind::IncomingCalls
                    | LanguageToolingOperationKind::OutgoingCalls
            )
        })
        .collect()
}

/// Ctrl/Cmd+Alt+H, optionally with Shift, as the shell's headless frame sees it.
fn call_hierarchy_key_input(shift: bool) -> egui::RawInput {
    let modifiers = egui::Modifiers {
        command: true,
        alt: true,
        shift,
        ..egui::Modifiers::default()
    };
    egui::RawInput {
        focused: true,
        modifiers,
        events: vec![egui::Event::Key {
            key: egui::Key::H,
            physical_key: Some(egui::Key::H),
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..egui::RawInput::default()
    }
}

/// Ctrl/Cmd+H — find-and-replace, and deliberately not call hierarchy.
fn find_replace_key_input() -> egui::RawInput {
    let modifiers = egui::Modifiers {
        command: true,
        ..egui::Modifiers::default()
    };
    egui::RawInput {
        focused: true,
        modifiers,
        events: vec![egui::Event::Key {
            key: egui::Key::H,
            physical_key: Some(egui::Key::H),
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..egui::RawInput::default()
    }
}

#[test]
fn show_incoming_calls_action_records_an_incoming_calls_operation() {
    let (workspace, source) = workspace_with_calls("legion_desktop_call_hierarchy_incoming");
    let mut runtime = open_runtime(workspace.path(), &source);

    assert!(
        call_hierarchy_operations(&runtime.projection_snapshot()).is_empty(),
        "opening a workspace must not ask a call-hierarchy question on its own"
    );

    let outcome = runtime
        .handle_action(DesktopAction::ShowIncomingCalls { position: caret() })
        .expect("showing incoming calls should reach the app");

    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::LanguageToolingUpdated,
        "the incoming-calls gesture must land in language tooling, not fall through to a no-op"
    );
    assert_eq!(
        call_hierarchy_operations(&runtime.projection_snapshot()),
        vec![LanguageToolingOperationKind::IncomingCalls],
        "the app must record exactly the callers question that was asked"
    );
}

#[test]
fn show_outgoing_calls_action_records_an_outgoing_calls_operation() {
    let (workspace, source) = workspace_with_calls("legion_desktop_call_hierarchy_outgoing");
    let mut runtime = open_runtime(workspace.path(), &source);

    let outcome = runtime
        .handle_action(DesktopAction::ShowOutgoingCalls { position: caret() })
        .expect("showing outgoing calls should reach the app");

    assert_eq!(
        outcome,
        DesktopWorkflowOutcome::LanguageToolingUpdated,
        "the outgoing-calls gesture must land in language tooling, not fall through to a no-op"
    );
    assert_eq!(
        call_hierarchy_operations(&runtime.projection_snapshot()),
        vec![LanguageToolingOperationKind::OutgoingCalls],
        "direction is the whole point of the two gestures; callees must not record as callers"
    );
}

#[test]
fn bridge_carries_the_active_buffer_and_the_caret_into_the_intent() {
    let (workspace, source) = workspace_with_calls("legion_desktop_call_hierarchy_bridge");
    let runtime = open_runtime(workspace.path(), &source);
    let snapshot = runtime.projection_snapshot();
    let active = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("launching with a file should make its buffer active");

    let bridge = DesktopCommandBridge::new();
    let position = caret();

    match bridge.translate(DesktopAction::ShowIncomingCalls { position }, &snapshot) {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ShowIncomingCalls {
            buffer_id,
            position: dispatched,
        }) => {
            assert_eq!(
                buffer_id, active,
                "callers must be asked about the open file"
            );
            assert_eq!(
                dispatched, position,
                "the caret the user pressed at is the symbol they meant"
            );
        }
        other => panic!("incoming calls should translate to its own intent, got {other:?}"),
    }

    match bridge.translate(DesktopAction::ShowOutgoingCalls { position }, &snapshot) {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ShowOutgoingCalls {
            buffer_id,
            position: dispatched,
        }) => {
            assert_eq!(
                buffer_id, active,
                "callees must be asked about the open file"
            );
            assert_eq!(
                dispatched, position,
                "the caret the user pressed at is the symbol they meant"
            );
        }
        other => panic!("outgoing calls should translate to its own intent, got {other:?}"),
    }
}

#[test]
fn command_alt_h_asks_for_callers_and_shift_asks_for_callees() {
    let (workspace, source) = workspace_with_calls("legion_desktop_call_hierarchy_keys");
    let runtime = open_runtime(workspace.path(), &source);
    let mut app = DesktopEframeApp::new(runtime);

    let _ = app.run_headless_full_frame(call_hierarchy_key_input(false));
    assert_eq!(
        call_hierarchy_operations(&app.runtime_snapshot()),
        vec![LanguageToolingOperationKind::IncomingCalls],
        "Ctrl/Cmd+Alt+H must reach the app — a binding that produces no request is the \
         defect this suite exists to retire"
    );

    let _ = app.run_headless_full_frame(call_hierarchy_key_input(true));
    assert_eq!(
        call_hierarchy_operations(&app.runtime_snapshot()),
        vec![
            LanguageToolingOperationKind::IncomingCalls,
            LanguageToolingOperationKind::OutgoingCalls,
        ],
        "Shift must flip the direction rather than repeat the callers question"
    );
}

#[test]
fn command_h_still_means_find_and_replace() {
    let (workspace, source) = workspace_with_calls("legion_desktop_call_hierarchy_no_collision");
    let runtime = open_runtime(workspace.path(), &source);
    let mut app = DesktopEframeApp::new(runtime);

    let _ = app.run_headless_full_frame(find_replace_key_input());

    assert!(
        call_hierarchy_operations(&app.runtime_snapshot()).is_empty(),
        "Ctrl/Cmd+H is the published find-replace binding; the Alt-carrying call-hierarchy \
         keys must not also fire on it"
    );
    assert!(
        app.runtime_snapshot().find_bar_projection.replace_visible,
        "and find-replace itself must still open, so the new keys did not shadow it"
    );
}
