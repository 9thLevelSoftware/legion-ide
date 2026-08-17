//! P2.F1.T4: a runnable code lens carries a command a terminal can run.
//!
//! rust-analyzer publishes "Run" and "Debug" as code lenses. The lens's
//! `command` field holds `rust-analyzer.runSingle`, which is a handle into
//! rust-analyzer's private protocol, not a shell command — the real invocation
//! lives in the command's `arguments`. `ActivateLanguageCodeLens` hands
//! `command_label` straight to the terminal, so projecting the handle there
//! produces a lens that looks runnable, activates, and fails.

use legion_lsp::project_code_lens_response;
use serde_json::json;

/// A lens shaped the way rust-analyzer actually sends a test runnable.
fn run_test_lens() -> serde_json::Value {
    json!([
        {
            "range": {
                "start": { "line": 4, "character": 0 },
                "end": { "line": 4, "character": 7 }
            },
            "command": {
                "title": "\u{25b6}\u{fe0e} Run Test",
                "command": "rust-analyzer.runSingle",
                "arguments": [
                    {
                        "label": "test my_module::my_test",
                        "kind": "cargo",
                        "args": {
                            "cargoArgs": ["test", "--package", "legion-lsp", "--lib"],
                            "executableArgs": ["my_module::my_test", "--exact", "--nocapture"],
                            "workspaceRoot": "/w"
                        }
                    }
                ]
            }
        }
    ])
}

#[test]
fn a_runnable_lens_projects_the_cargo_invocation_not_the_command_id() {
    let lenses = project_code_lens_response(&run_test_lens(), "rust-analyzer", 10);
    let lens = lenses.first().expect("one lens projected");

    assert_eq!(
        lens.command_label,
        "cargo test --package legion-lsp --lib -- my_module::my_test --exact --nocapture",
        "the terminal receives this string; it has to be a command"
    );
    assert!(
        !lens.command_label.contains("runSingle"),
        "the LSP command id is a protocol handle, not something to run"
    );
}

/// The kind label is what `ActivateLanguageCodeLens` gates on before launching.
#[test]
fn a_runnable_lens_is_marked_runnable_so_activation_will_accept_it() {
    let lenses = project_code_lens_response(&run_test_lens(), "rust-analyzer", 10);
    assert!(
        lenses[0].kind_label.contains("runnable"),
        "activation refuses any lens whose kind is not runnable, got {:?}",
        lenses[0].kind_label
    );
}

/// An ordinary lens is left exactly as it was.
#[test]
fn a_non_runnable_lens_keeps_its_command_id_and_its_kind() {
    let payload = json!([
        {
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 3 }
            },
            "command": {
                "title": "3 implementations",
                "command": "rust-analyzer.showReferences"
            }
        }
    ]);
    let lenses = project_code_lens_response(&payload, "rust-analyzer", 10);
    let lens = lenses.first().expect("one lens projected");
    assert_eq!(lens.command_label, "rust-analyzer.showReferences");
    assert!(
        !lens.kind_label.contains("runnable"),
        "a lens with no runnable arguments must not become launchable"
    );
}

/// A lens that names itself a runnable but carries no cargo arguments is not
/// one.
///
/// Marking it runnable anyway would let activation hand the bare command id to
/// a shell — the exact failure this whole change exists to prevent — so the
/// absence of `cargoArgs` has to fall through to the ordinary path.
#[test]
fn a_runnable_command_without_arguments_is_not_treated_as_runnable() {
    let payload = json!([
        {
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 3 }
            },
            "command": {
                "title": "Run",
                "command": "rust-analyzer.runSingle"
            }
        }
    ]);
    let lenses = project_code_lens_response(&payload, "rust-analyzer", 10);
    let lens = lenses.first().expect("one lens projected");
    assert!(
        !lens.kind_label.contains("runnable"),
        "no cargoArgs means no command to run, whatever the lens calls itself"
    );
    assert_eq!(lens.command_label, "rust-analyzer.runSingle");
}

/// A debug lens is a runnable too — `debugSingle` carries the same arguments.
#[test]
fn a_debug_lens_projects_its_cargo_invocation() {
    let payload = json!([
        {
            "range": {
                "start": { "line": 4, "character": 0 },
                "end": { "line": 4, "character": 7 }
            },
            "command": {
                "title": "Debug",
                "command": "rust-analyzer.debugSingle",
                "arguments": [
                    {
                        "args": {
                            "cargoArgs": ["test", "--no-run"],
                            "executableArgs": []
                        }
                    }
                ]
            }
        }
    ]);
    let lenses = project_code_lens_response(&payload, "rust-analyzer", 10);
    assert_eq!(lenses[0].command_label, "cargo test --no-run");
    assert!(
        !lenses[0].command_label.ends_with("--"),
        "an empty executableArgs must not leave a dangling separator"
    );
}
