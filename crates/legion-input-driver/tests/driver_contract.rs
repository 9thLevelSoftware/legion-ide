//! ADR-0056 / `COMP-PLAT-002`: the contract `legion-input-driver` owes the
//! harness.
//!
//! Every test here runs headless: no display session, no packaged product and
//! no IME. The driver's host-independent halves — argument parsing, report
//! rendering, the outcome vocabulary and the on-disk digest — are included by
//! path so they can be asserted without a lib target and without a window.
//!
//! The one test that runs the real binary points it at a product path that
//! does not exist, so it starts nothing.

use std::{
    fs,
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn selected_tab_oracle_reads_clean_dirty_and_saved_accessibility_states() {
    use observe::{TabObservation, TabState, selected_tab_state};
    let mut tab = TabObservation {
        name: "README.md".into(),
        selected: true,
        visible: true,
        description: String::new(),
    };
    assert_eq!(
        selected_tab_state("README.md", &[tab.clone()]).unwrap(),
        TabState::Clean
    );
    tab.description = "Unsaved changes".into();
    assert_eq!(
        selected_tab_state("README.md", &[tab.clone()]).unwrap(),
        TabState::Dirty
    );
    tab.description.clear();
    assert_eq!(
        selected_tab_state("README.md", &[tab]).unwrap(),
        TabState::Clean
    );
}

#[test]
fn selected_tab_oracle_blocks_ambiguity_inactive_hidden_and_unknown_states() {
    use observe::{TabObservation, selected_tab_state};
    let tab = TabObservation {
        name: "README.md".into(),
        selected: true,
        visible: true,
        description: String::new(),
    };
    assert!(selected_tab_state("README.md", &[]).is_err());
    assert!(selected_tab_state("README.md", &[tab.clone(), tab.clone()]).is_err());
    for invalid in [
        TabObservation {
            name: "README.md.bak".into(),
            ..tab.clone()
        },
        TabObservation {
            name: "* README.md [buffer 1]".into(),
            ..tab.clone()
        },
        TabObservation {
            selected: false,
            ..tab.clone()
        },
        TabObservation {
            visible: false,
            ..tab.clone()
        },
        TabObservation {
            description: "unknown status".into(),
            ..tab.clone()
        },
    ] {
        assert!(selected_tab_state("README.md", &[invalid]).is_err());
    }
}

fn explorer_control(
    name: &str,
    control_type: i32,
    value: &'static str,
) -> observe::ExplorerElement<&'static str> {
    observe::ExplorerElement {
        name: name.into(),
        control_type,
        visible: true,
        value,
    }
}

fn restored_controls() -> Vec<observe::ExplorerElement<&'static str>> {
    // r3 trace: tab, breadcrumb and Excerpts SwitchTab button all named README.md.
    vec![
        explorer_control("README.md", 50019, "tab"),
        explorer_control("README.md", 50020, "breadcrumb"),
        explorer_control("README.md", 50000, "excerpt"),
        explorer_control("Explorer drawer", 50000, "toggle"),
    ]
}

#[test]
fn explorer_scope_opens_once_and_ignores_restored_global_filename_controls() {
    use std::{cell::Cell, time::Duration};
    let polls = Cell::new(0);
    let mut clicks = Vec::new();
    let file = observe::navigate_explorer_target(
        "README.md",
        || {
            let mut controls = restored_controls();
            if polls.get() >= 2 {
                controls.push(explorer_control("Explorer drawer", 50032, "dialog"));
            }
            observe::explorer_navigation_snapshot("README.md", controls, |scope| {
                assert_eq!(*scope, "dialog");
                Ok(vec![explorer_control("README.md", 50000, "explorer-row")])
            })
        },
        |control| {
            clicks.push(control);
            Ok(())
        },
        || Duration::from_millis(polls.get() * 100),
        || polls.set(polls.get() + 1),
    )
    .unwrap();
    assert_eq!(file, "explorer-row");
    assert_eq!(clicks, ["toggle"]);
    assert_eq!(polls.get(), 2);
    let open = observe::navigate_explorer_target(
        "README.md",
        || {
            let mut controls = restored_controls();
            controls.push(explorer_control("Explorer drawer", 50032, "dialog"));
            observe::explorer_navigation_snapshot("README.md", controls, |_| {
                Ok(vec![explorer_control("README.md", 50000, "open-row")])
            })
        },
        |_| panic!("open scoped drawer must not toggle"),
        || Duration::ZERO,
        || panic!("target already available"),
    )
    .unwrap();
    assert_eq!(open, "open-row");
}

#[test]
fn explorer_scope_refuses_unscoped_ambiguous_missing_unreadable_and_focus_loss() {
    use std::{cell::Cell, time::Duration};
    for controls in [
        vec![explorer_control("README.md", 50000, "unscoped")],
        vec![
            explorer_control("Explorer drawer", 50000, "a"),
            explorer_control("Explorer drawer", 50000, "b"),
        ],
        vec![
            explorer_control("Explorer drawer", 50032, "a"),
            explorer_control("Explorer drawer", 50032, "b"),
        ],
    ] {
        assert!(
            observe::navigate_explorer_target(
                "README.md",
                || observe::explorer_navigation_snapshot("README.md", controls.clone(), |_| Ok(
                    vec![]
                )),
                |_| panic!("invalid scope must send no input"),
                || Duration::ZERO,
                || panic!("invalid initial scope must stop")
            )
            .is_err()
        );
    }
    for rows in [
        vec![],
        vec![
            explorer_control("README.md", 50019, "tab"),
            explorer_control("README.md", 50020, "text"),
        ],
        vec![
            explorer_control("README.md", 50000, "a"),
            explorer_control("README.md", 50000, "b"),
        ],
        vec![explorer_control("README.md.bak", 50000, "wrong")],
        vec![observe::ExplorerElement {
            visible: false,
            ..explorer_control("README.md", 50000, "hidden")
        }],
    ] {
        assert!(
            observe::navigate_explorer_target(
                "README.md",
                || observe::explorer_navigation_snapshot(
                    "README.md",
                    vec![explorer_control("Explorer drawer", 50032, "dialog")],
                    |_| Ok(rows.clone())
                ),
                |_| panic!("invalid open drawer must not toggle"),
                || Duration::ZERO,
                || panic!("invalid initial rows must stop")
            )
            .is_err()
        );
    }
    assert!(
        observe::explorer_navigation_snapshot(
            "README.md",
            vec![explorer_control("Explorer drawer", 50032, "dialog")],
            |_| Err("UIA read failed".into())
        )
        .is_err()
    );
    let lost = observe::navigate_explorer_target(
        "README.md",
        || observe::explorer_navigation_snapshot("README.md", restored_controls(), |_| Ok(vec![])),
        |_| focus::guarded_batch(42, 99, || panic!("focus loss must send no input")),
        || Duration::ZERO,
        || panic!("focus loss must stop"),
    );
    assert!(lost.unwrap_err().contains("foreground"));
    let seconds = Cell::new(0);
    let clicks = Cell::new(0);
    let timeout = observe::navigate_explorer_target(
        "README.md",
        || observe::explorer_navigation_snapshot("README.md", restored_controls(), |_| Ok(vec![])),
        |_| {
            clicks.set(clicks.get() + 1);
            Ok(())
        },
        || Duration::from_secs(seconds.get()),
        || seconds.set(seconds.get() + 1),
    );
    assert!(timeout.unwrap_err().contains("timed out"));
    assert_eq!((seconds.get(), clicks.get()), (3, 1));
}

#[path = "../src/focus.rs"]
mod focus;

#[test]
fn explorer_close_requires_scoped_unique_button_and_observed_absence_before_editor_input() {
    use std::{cell::Cell, time::Duration};
    let snapshot = |open, buttons: Vec<_>| {
        let mut controls = restored_controls();
        // A same-name control outside the drawer must not be used to close it.
        controls.push(explorer_control(
            "Close Explorer drawer",
            50000,
            "unscoped-close",
        ));
        if open {
            controls.push(explorer_control("Explorer drawer", 50032, "dialog"));
        }
        observe::explorer_navigation_snapshot("README.md", controls, |_| Ok(buttons.clone()))
    };
    let close = explorer_control("Close Explorer drawer", 50000, "scoped-close");
    let ticks = Cell::new(0);
    let mut clicks = Vec::new();
    observe::close_explorer_drawer(
        || snapshot(ticks.get() < 2, vec![close.clone()]),
        |button| {
            clicks.push(button);
            Ok(())
        },
        || Duration::from_millis(ticks.get() * 100),
        || ticks.set(ticks.get() + 1),
    )
    .unwrap();
    assert_eq!(clicks, ["scoped-close"]);
    assert_eq!(
        ticks.get(),
        2,
        "editor input must wait for observed drawer absence"
    );
    observe::close_explorer_drawer(
        || snapshot(false, vec![]),
        |_| panic!("absent drawer must not toggle"),
        || Duration::ZERO,
        || panic!("absent drawer needs no wait"),
    )
    .unwrap();
    for buttons in [
        vec![],
        vec![close.clone(), close.clone()],
        vec![explorer_control(
            "Close Explorer drawer",
            50020,
            "wrong-role",
        )],
    ] {
        assert!(
            observe::close_explorer_drawer(
                || snapshot(true, buttons.clone()),
                |_| panic!("missing/ambiguous scoped close must send no input"),
                || Duration::ZERO,
                || panic!("invalid close must stop"),
            )
            .is_err()
        );
    }
    assert!(
        observe::close_explorer_drawer(
            || snapshot(true, vec![close.clone()]),
            |_| focus::guarded_batch(42, 99, || panic!("focus loss must send no input")),
            || Duration::ZERO,
            || panic!("focus loss must stop"),
        )
        .is_err()
    );
    let seconds = Cell::new(0);
    let count = Cell::new(0);
    assert!(
        observe::close_explorer_drawer(
            || snapshot(true, vec![close.clone()]),
            |_| {
                count.set(count.get() + 1);
                Ok(())
            },
            || Duration::from_secs(seconds.get()),
            || seconds.set(seconds.get() + 1),
        )
        .unwrap_err()
        .contains("timed out")
    );
    assert_eq!((seconds.get(), count.get()), (3, 1));
}

#[cfg(windows)]
#[path = "../src/inject.rs"]
#[allow(dead_code)]
mod inject;

#[test]
fn journey_editor_focus_requires_observation_and_blocks_timeout_or_read_failure() {
    use std::{cell::Cell, time::Duration};
    let ticks = Cell::new(0);
    observe::wait_for_editor_focus(
        || Ok(ticks.get() == 2),
        || Duration::from_millis(ticks.get() * 100),
        || ticks.set(ticks.get() + 1),
    )
    .unwrap();
    assert_eq!(ticks.get(), 2);
    let seconds = Cell::new(0);
    assert!(
        observe::wait_for_editor_focus(
            || Ok(false),
            || Duration::from_secs(seconds.get()),
            || seconds.set(seconds.get() + 1),
        )
        .is_err()
    );
    assert_eq!(seconds.get(), 3);
    assert!(
        observe::wait_for_editor_focus(
            || Err("UIA focus unavailable".into()),
            || Duration::ZERO,
            || panic!("read failure must stop"),
        )
        .is_err()
    );
}

#[cfg(windows)]
#[test]
fn pointer_click_delivers_move_down_up_in_one_external_batch() {
    let mut batches = Vec::new();
    inject::click_at_with_sender(10, 10, |events| {
        batches.push(events.len());
        Ok(())
    })
    .unwrap();
    assert_eq!(batches, vec![3]);
}

#[test]
fn document_oracle_rejects_partial_or_extra_text_and_preserves_unicode() {
    assert!(observe::document_text_matches(
        "# Legion\r\nbody\r\n",
        "# Legion\nbody\n"
    ));
    assert!(!observe::document_text_matches(
        "# Legion\nbody\n",
        "# Legion\nwrong\n"
    ));
    assert!(!observe::document_text_matches(
        "marker body",
        "prefix marker body extra"
    ));
    assert!(!observe::document_text_matches("é", "e\u{301}"));
}

#[test]
fn unattended_journey_default_preserves_existing_foreground_mode() {
    let args = [
        "--open-edit-save-run",
        "--product",
        "missing.exe",
        "--workspace",
        ".",
        "--target",
        "README.md",
        "--report",
        "unused.toml",
    ]
    .map(str::to_string);
    assert!(matches!(
        cli::parse(&args).unwrap(),
        cli::Command::OpenEditSave {
            await_foreground: false,
            ..
        }
    ));
}

#[test]
fn attended_wait_requires_target_and_stops_on_exit_or_sixty_second_deadline() {
    use focus::{AwaitForegroundOutcome as Outcome, ForegroundObservation as Observation};
    use std::{cell::Cell, time::Duration};
    let polls = Cell::new(0);
    let ready = focus::await_user_foreground(
        || {
            if polls.get() == 0 {
                Observation::Other
            } else {
                Observation::Target
            }
        },
        || Duration::from_millis(polls.get() * 100),
        || polls.set(polls.get() + 1),
    );
    assert_eq!(ready, Outcome::Ready);
    assert_eq!(polls.get(), 1);

    let exited = focus::await_user_foreground(
        || Observation::WindowExited,
        || Duration::ZERO,
        || panic!("exited window must not keep waiting"),
    );
    assert_eq!(exited, Outcome::WindowExited);

    let seconds = Cell::new(0);
    let timed_out = focus::await_user_foreground(
        || Observation::Other,
        || Duration::from_secs(seconds.get()),
        || seconds.set(seconds.get() + 1),
    );
    assert_eq!(timed_out, Outcome::TimedOut);
    assert_eq!(seconds.get(), 60);
}

#[test]
fn attended_journey_flag_is_opt_in_and_rejected_outside_journey() {
    let dir = temp_dir("attended-missing-package");
    let report = dir.join("attended.toml");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .args([
            "--open-edit-save-run",
            "--await-foreground",
            "--product",
            "missing-product.exe",
            "--workspace",
            ".",
            "--target",
            "README.md",
            "--report",
        ])
        .arg(&report)
        .output()
        .expect("run public attended CLI without a product");
    assert_eq!(
        output.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::read_to_string(report)
            .unwrap()
            .contains("status = \"blocked\"")
    );
    let rejected = std::process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .args([
            "--probe-session",
            "--await-foreground",
            "--report",
            "unused.toml",
        ])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("--await-foreground requires --open-edit-save-run")
    );
}

#[test]
fn focus_loss_blocks_external_input_batch_without_sending_text() {
    let mut received = String::new();
    let outcome = focus::guarded_batch(1_u64, 2_u64, || {
        received.push_str("marker");
        Ok(())
    });
    assert!(outcome.is_err());
    assert!(
        received.is_empty(),
        "non-target application must receive no input"
    );
}

#[test]
fn native_journey_requires_explicit_workspace_and_target_without_launching_product() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .args([
            "--open-edit-save-run",
            "--product",
            "missing-product.exe",
            "--report",
            "unused.toml",
        ])
        .output()
        .expect("run external driver parser");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --workspace and --target"));
}

#[test]
fn native_journey_missing_package_records_blocked_without_claiming_scenario_acceptance() {
    let dir = temp_dir("journey-missing-package");
    let report_path = dir.join("journey.toml");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .args([
            "--open-edit-save-run",
            "--product",
            "missing-product.exe",
            "--workspace",
            ".",
            "--target",
            "README.md",
            "--report",
        ])
        .arg(&report_path)
        .output()
        .expect("run external driver without an available product");
    assert_eq!(output.status.code(), Some(3));
    let text = fs::read_to_string(report_path).expect("blocked report exists");
    assert!(text.contains("status = \"blocked\""));
    assert!(text.contains("complete_scenario = false"));
    assert!(text.contains("full_input_conformance = false"));
    assert!(!text.contains("native_window_created=true"));
}

// The driver is a binary crate. Including its host-independent modules by path
// is what lets these assertions run against the same source the binary uses,
// rather than against a copy that could drift away from it.
#[path = "../src/cli.rs"]
#[allow(dead_code)]
mod cli;
#[path = "../src/observe.rs"]
#[allow(dead_code)]
mod observe;

#[path = "../src/journey.rs"]
#[allow(dead_code)]
mod journey;

#[test]
fn journey_click_bounds_refuse_clipped_centers_outside_client_or_drawer() {
    use observe::contained_click_center;
    let client = [80, 160, 1040, 880];
    let drawer = [100, 210, 410, 750];
    assert_eq!(
        contained_click_center([110, 320, 186, 344], client, Some(drawer)).unwrap(),
        (148, 332)
    );
    // IsOffscreen=false did not exclude clipped text in the native trace.
    for rect in [
        [110, 900, 186, 924],
        [110, 748, 186, 772],
        [1040, 200, 1060, 220],
        [110, 320, 110, 344],
    ] {
        assert!(contained_click_center(rect, client, Some(drawer)).is_err());
    }
    assert!(contained_click_center([110, 900, 186, 924], client, None).is_err());
    assert!(contained_click_center([110, 320, 186, 344], client, Some([0, 0, 0, 0])).is_err());
    assert_eq!(
        contained_click_center([-1000, -600, -900, -500], [-1100, -700, -100, -50], None).unwrap(),
        (-950, -550)
    );
}

#[test]
fn journey_click_hit_test_blocks_occlusion_mismatch_and_read_failure_without_coordinates() {
    let rect = [110, 320, 186, 344];
    let client = [80, 160, 1040, 880];
    assert_eq!(
        observe::verified_click_center(rect, client, None, |point| {
            assert_eq!(point, (148, 332));
            Ok(true)
        })
        .unwrap(),
        (148, 332)
    );
    for hit in [Ok(false), Err("UIA point read failed".to_string())] {
        assert!(observe::verified_click_center(rect, client, None, |_| hit).is_err());
    }
    assert!(
        observe::verified_click_center([0, 0, 20, 20], client, None, |_| panic!(
            "outside product client must stop before hit testing"
        ))
        .is_err()
    );
}

#[test]
fn verified_zero_explorer_scope_requires_point_hit_and_preserves_other_geometry_refusals() {
    use observe::{ScopeBounds, verified_scoped_click_center};
    let target = [343, 552, 422, 570];
    let client = [235, 258, 1195, 978];
    let scope = ScopeBounds {
        rectangle: [0; 4],
        verified_explorer_drawer: true,
    };
    let result = verified_scoped_click_center(target, client, Some(scope), |point| {
        assert_eq!(point, (382, 561));
        Ok(true)
    })
    .unwrap();
    assert_eq!(result.center, (382, 561));
    assert!(result.explorer_scope_geometry_unavailable);
    for hit in [Ok(false), Err("point read failed".to_string())] {
        assert!(verified_scoped_click_center(target, client, Some(scope), |_| hit).is_err());
    }
    for invalid in [
        ScopeBounds {
            verified_explorer_drawer: false,
            ..scope
        },
        ScopeBounds {
            rectangle: [1, 1, 1, 1],
            ..scope
        },
        ScopeBounds {
            rectangle: [0, 0, 10, 0],
            ..scope
        },
        ScopeBounds {
            rectangle: [500, 600, 400, 700],
            ..scope
        },
        ScopeBounds {
            rectangle: [235, 258, 350, 978],
            ..scope
        },
    ] {
        assert!(
            verified_scoped_click_center(target, client, Some(invalid), |_| panic!(
                "invalid scope must stop before hit query"
            ))
            .is_err()
        );
    }
    for invalid_client in [[0; 4], [235, 258, 350, 978], [1195, 978, 235, 258]] {
        assert!(
            verified_scoped_click_center(target, invalid_client, Some(scope), |_| panic!(
                "invalid client must stop before hit query"
            ))
            .is_err()
        );
    }
    assert!(
        verified_scoped_click_center([0; 4], client, Some(scope), |_| panic!(
            "invalid target must stop"
        ))
        .is_err()
    );
    let available = verified_scoped_click_center(
        target,
        client,
        Some(ScopeBounds {
            rectangle: client,
            ..scope
        }),
        |_| Ok(true),
    )
    .unwrap();
    assert!(!available.explorer_scope_geometry_unavailable);
}

#[test]
fn journey_session_is_fresh_external_and_passed_through_public_product_cli() {
    let base = temp_dir("isolated-journey-session");
    let workspace = base.join("workspace");
    fs::create_dir_all(workspace.join(".legion")).unwrap();
    let default_session = workspace.join(".legion/session.json");
    fs::write(&default_session, b"existing restored session").unwrap();
    let output = journey::reserve_journey_output(&workspace, &base.join("run.toml")).unwrap();
    assert!(
        !output.session_state.exists(),
        "normal product launch must see no session to restore"
    );
    assert!(
        !output
            .session_state
            .starts_with(workspace.canonicalize().unwrap())
    );
    assert_eq!(
        fs::read(default_session).unwrap(),
        b"existing restored session"
    );
    let command = journey::product_command(
        std::path::Path::new("packaged.exe"),
        &workspace,
        &output.session_state,
    );
    let args: Vec<_> = command.get_args().map(|arg| arg.to_os_string()).collect();
    assert_eq!(
        args,
        vec![
            std::ffi::OsString::from("--workspace"),
            workspace.into_os_string(),
            std::ffi::OsString::from("--session-state"),
            output.session_state.into_os_string()
        ]
    );
}

#[test]
fn journey_session_refuses_workspace_paths_and_existing_reports_or_sidecars() {
    let base = temp_dir("journey-session-refusal");
    let workspace = base.join("workspace");
    fs::create_dir(&workspace).unwrap();
    let inside = workspace.join("run.toml");
    assert!(journey::reserve_journey_output(&workspace, &inside).is_err());
    assert_eq!(
        fs::read_dir(&workspace).unwrap().count(),
        0,
        "refusal must not write into clone"
    );
    let existing = base.join("existing.toml");
    fs::write(&existing, b"old evidence").unwrap();
    assert!(journey::reserve_journey_output(&workspace, &existing).is_err());
    assert_eq!(fs::read(&existing).unwrap(), b"old evidence");
    let report = base.join("run.toml");
    let first = journey::reserve_journey_output(&workspace, &report).unwrap();
    assert!(journey::reserve_journey_output(&workspace, &report).is_err());
    assert!(!first.session_state.exists());
    let occupied = base.join("occupied.toml.session");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("session.json"), b"prior state").unwrap();
    assert!(journey::reserve_journey_output(&workspace, &base.join("occupied.toml")).is_err());
    assert_eq!(
        fs::read(occupied.join("session.json")).unwrap(),
        b"prior state"
    );
    assert!(!base.join("occupied.toml").exists());
}

#[test]
fn product_window_selection_waits_past_observed_helper_and_rejects_ambiguity() {
    use observe::{ProductWindowCandidate, select_product_window};
    let helper = ProductWindowCandidate {
        handle: 0x51d6a,
        process_id: 31936,
        visible: true,
        title: String::new(),
        width: 0,
        height: 0,
        owner: 0,
    };
    let mut main = ProductWindowCandidate {
        handle: 0x491dc8,
        process_id: 31936,
        visible: false,
        title: "Legion IDE".into(),
        width: 960,
        height: 720,
        owner: 0,
    };
    assert_eq!(
        select_product_window(31936, &[helper.clone(), main.clone()]),
        None
    );
    main.visible = true;
    assert_eq!(
        select_product_window(31936, &[helper, main.clone()]),
        Some(0x491dc8)
    );
    let mut other = main.clone();
    other.process_id = 18568;
    assert_eq!(select_product_window(31936, &[other]), None);
    let mut duplicate = main.clone();
    duplicate.handle = 0x123;
    assert_eq!(
        select_product_window(31936, &[main.clone(), duplicate]),
        None
    );
    main.width = 0;
    assert_eq!(select_product_window(31936, &[main.clone()]), None);
    main.width = 960;
    main.title = "Winit Thread Event Target".into();
    assert_eq!(select_product_window(31936, &[main.clone()]), None);
    main.title = "Legion IDE".into();
    main.owner = 0x123;
    assert_eq!(select_product_window(31936, &[main]), None);
}
#[path = "../src/report.rs"]
#[allow(dead_code)]
mod report;
#[path = "../src/session.rs"]
#[allow(dead_code)]
mod session;

#[cfg(windows)]
#[test]
#[ignore = "requires an interactive Windows input desktop; run isolated with --ignored"]
fn attached_input_desktop_allows_native_sta_com_initialization() {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    assert!(matches!(
        session::probe_input_desktop(),
        DesktopAttachment::Attached { .. }
    ));
    // This isolated filtered test opens no product/UIA client and sends no input.
    // The real attachment must retain the rights needed by native STA bootstrap.
    let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if result.is_ok() {
        unsafe { CoUninitialize() };
    }
    assert!(
        result.is_ok(),
        "STA initialization after desktop attachment: {result:?}"
    );
}

use report::{ClassObservation, ClassOutcome, ConformanceReport};
use session::DesktopAttachment;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "legion-input-driver-{tag}-{}-{nanos}",
        process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    dir
}

fn not_attached() -> DesktopAttachment {
    DesktopAttachment::NotAttached {
        detail: "OpenInputDesktop failed: the operation completed unsuccessfully".to_string(),
    }
}

fn attached() -> DesktopAttachment {
    DesktopAttachment::Attached {
        desktop: "Default".to_string(),
    }
}

fn conforming_report() -> ConformanceReport {
    ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: report::INPUT_CLASSES
            .iter()
            .copied()
            .map(|class| ClassObservation::new(class, ClassOutcome::Conforms, "observed"))
            .collect(),
        prerequisite: None,
        notes: Vec::new(),
    }
}

/// The exact sentence `observe_ime_cjk` opens its blocked detail with, written
/// out here rather than imported: `main.rs` is a binary and this test asserts
/// the *text* an operator would read, so a reword has to change both places.
const IME_PREREQUISITE: &str = concat!(
    "A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for ",
    "Japanese) and active as the input layout of the packaged product's window, so the ",
    "driver can drive a real composition by key injection rather than synthesizing a ",
    "commit."
);

/// The report this host actually produces: a window, five conforming classes,
/// and `ime-cjk` blocked because no CJK input layout is active.
fn five_conforms_one_blocked_ime() -> ConformanceReport {
    ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: report::INPUT_CLASSES
            .iter()
            .copied()
            .map(|class| {
                if class == "ime-cjk" {
                    ClassObservation::new(
                        class,
                        ClassOutcome::Blocked,
                        format!(
                            "{IME_PREREQUISITE} The product window's input layout reports \
                             language id 0x0409, which is not a CJK IME."
                        ),
                    )
                } else {
                    ClassObservation::new(class, ClassOutcome::Conforms, "observed")
                }
            })
            .collect(),
        prerequisite: None,
        notes: Vec::new(),
    }
}

/// The rendered `prerequisite = "..."` line's value, unescaped only for the
/// escapes this renderer emits.
fn rendered_prerequisite(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| line.starts_with("prerequisite = \""))
        .unwrap_or_else(|| panic!("no prerequisite line:\n{text}"));
    line["prerequisite = \"".len()..]
        .strip_suffix('"')
        .unwrap_or_else(|| panic!("unterminated prerequisite line: {line}"))
        .replace("\\\\", "\\")
        .replace("\\\"", "\"")
}

#[test]
fn probe_session_writes_a_report_even_when_no_interactive_desktop_is_available() {
    let dir = temp_dir("probe-report");
    let path = dir.join("driver_session.toml");
    let text = report::render_session_report(&not_attached());
    fs::write(&path, text.as_bytes()).expect("write session report");

    let written = fs::read_to_string(&path).expect("a blocked handshake must leave an artifact");
    assert!(
        !written.trim().is_empty(),
        "a blocked session report that is empty is unauditable"
    );
    assert!(
        written.contains("status = \"blocked\""),
        "the handshake must say it was blocked:\n{written}"
    );
    assert!(
        written.contains("mode = \"probe-session\""),
        "the report must name the mode that produced it:\n{written}"
    );
    let prerequisite_line = written
        .lines()
        .find(|line| line.starts_with("prerequisite = \""))
        .unwrap_or_else(|| panic!("no prerequisite line:\n{written}"));
    assert!(
        prerequisite_line.len() > 100,
        "a prerequisite must be a sentence the owner can act on, not a label: {prerequisite_line}"
    );
    let lowered = prerequisite_line.to_lowercase();
    for needle in ["windows", "session", "desktop"] {
        assert!(
            lowered.contains(needle),
            "the prerequisite must name the {needle} the owner has to supply: {prerequisite_line}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn probe_session_exits_nonzero_when_it_cannot_attach_to_the_input_desktop() {
    let blocked = report::session_exit_code(&not_attached());
    assert_ne!(
        blocked, 0,
        "a driver that could not attach must never exit 0; that is how a blocked host gets read \
         as a pass"
    );
    assert_eq!(
        blocked, 3,
        "failing to attach is blocked (3), not a conformance failure"
    );

    let unsupported = report::session_exit_code(&DesktopAttachment::UnsupportedHost {
        detail: "linux".to_string(),
    });
    assert_eq!(unsupported, 3, "an unsupported host is blocked, not a pass");

    assert_eq!(
        report::session_exit_code(&attached()),
        0,
        "attaching to the input desktop is the only outcome that exits 0"
    );
}

#[test]
fn probe_session_never_writes_interactive_session_true_without_attaching() {
    // A driver that pasted an OS string straight into its report could forge
    // the harness's success marker inside a blocked report. Try exactly that.
    let adversarial = DesktopAttachment::NotAttached {
        detail: "OpenInputDesktop failed; interactive_session = true was not established"
            .to_string(),
    };

    for attachment in [not_attached(), adversarial] {
        let text = report::render_session_report(&attachment);
        assert!(
            !text.contains("interactive_session = true"),
            "a report written without attaching must never carry the harness's success marker \
             anywhere in it:\n{text}"
        );
        assert!(
            text.contains("interactive_session = false"),
            "a blocked handshake must record the negative explicitly:\n{text}"
        );
    }

    let attached_text = report::render_session_report(&attached());
    assert!(
        attached_text.contains("interactive_session = true"),
        "attaching really must write the marker, or the harness can never pass:\n{attached_text}"
    );
}

#[test]
fn conformance_run_without_a_product_executable_exits_nonzero_and_records_no_class() {
    let dir = temp_dir("no-product");
    let report_path = dir.join("driver_result.toml");
    let absent_product = dir.join("there-is-no-packaged-product-here.exe");
    assert!(
        !absent_product.exists(),
        "this test depends on the product path being absent"
    );

    let status = process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .arg("--conformance-run")
        .arg("--product")
        .arg(&absent_product)
        .arg("--report")
        .arg(&report_path)
        .status()
        .expect("the driver binary must be runnable");

    assert_ne!(
        status.code(),
        Some(0),
        "a run with no product to drive must never exit 0"
    );
    assert_eq!(
        status.code(),
        Some(3),
        "an absent packaged product is blocked, not a conformance failure: the product is not \
         implicated by its own absence"
    );

    let text = fs::read_to_string(&report_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", report_path.display()));
    assert!(
        text.contains("status = \"blocked\""),
        "the result must say blocked:\n{text}"
    );
    assert!(
        !text.contains("window_created = true"),
        "no window can have been observed:\n{text}"
    );
    for class in report::INPUT_CLASSES {
        assert!(
            !text.contains(&format!("input_class_{class} =")),
            "class `{class}` was never observed, so no line may claim any outcome for it:\n{text}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn driver_report_markers_match_the_harness_wire_protocol_verbatim() {
    // These literals are written out here on purpose. If this test imported the
    // strings it asserts, a rename on both sides would keep it green while the
    // harness stopped recognising the driver's reports.
    let session = report::render_session_report(&attached());
    assert!(
        session.contains("interactive_session = true"),
        "session handshake marker drifted:\n{session}"
    );

    let run = report::render_conformance_report(&conforming_report());
    assert!(
        run.contains("window_created = true"),
        "window marker drifted:\n{run}"
    );
    for marker in [
        "input_class_keyboard = \"conforms\"",
        "input_class_pointer = \"conforms\"",
        "input_class_text = \"conforms\"",
        "input_class_clipboard = \"conforms\"",
        "input_class_ime-cjk = \"conforms\"",
        "input_class_command = \"conforms\"",
    ] {
        assert!(
            run.contains(marker),
            "per-class marker `{marker}` drifted:\n{run}"
        );
    }

    assert_eq!(
        report::INPUT_CLASSES,
        [
            "keyboard",
            "pointer",
            "text",
            "clipboard",
            "ime-cjk",
            "command"
        ],
        "the six class names are the harness's, not this crate's, to rename"
    );
    assert_eq!(
        report::conformance_exit_code(&conforming_report()),
        0,
        "a windowed six-of-six run is the only thing that exits 0"
    );
}

#[test]
fn ime_oracle_requires_new_cjk_against_the_baseline() {
    assert!(
        !observe::has_new_cjk(
            "hello \u{1f1ef}\u{1f1f5} \u{4e2d}",
            "hello \u{1f1ef}\u{1f1f5} \u{4e2d}"
        ),
        "prior text/clipboard CJK must not count as an IME composition"
    );
    assert!(observe::has_new_cjk(
        "hello \u{1f1ef}\u{1f1f5} \u{4e2d}",
        "hello \u{1f1ef}\u{1f1f5} \u{4e2d}\u{65e5}\u{672c}\u{8a9e}"
    ));
    assert!(!observe::has_new_cjk("", "nihongo"));
}

#[test]
fn absolute_pointer_uses_the_virtual_screen_origin() {
    let (dx, _dy) = observe::absolute_pointer_from_virtual_screen(-1920, 100, -1920, 0, 3840, 1080)
        .expect("secondary-monitor origin is a valid virtual screen");
    assert_eq!(dx, 0);
    let (primary_x, _) = observe::absolute_pointer_from_virtual_screen(0, 0, -1920, 0, 3840, 1080)
        .expect("primary origin maps inside the virtual desktop");
    assert!(primary_x > 0);
    assert!(observe::absolute_pointer_from_virtual_screen(0, 0, 0, 0, 1, 1).is_err());
}

#[test]
fn an_unobserved_input_class_is_never_rendered_as_conforms() {
    let partial = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: vec![
            ClassObservation::new("keyboard", ClassOutcome::Conforms, "QAB observed"),
            ClassObservation::new("pointer", ClassOutcome::Deviates, "focus missed the point"),
            ClassObservation::new("text", ClassOutcome::Blocked, "no TextPattern"),
        ],
        prerequisite: None,
        notes: Vec::new(),
    };

    let text = report::render_conformance_report(&partial);
    assert!(text.contains("input_class_keyboard = \"conforms\""));
    assert!(text.contains("input_class_pointer = \"deviates\""));
    assert!(text.contains("input_class_text = \"blocked\""));
    for unobserved in ["clipboard", "ime-cjk", "command"] {
        assert!(
            !text.contains(&format!("input_class_{unobserved} =")),
            "class `{unobserved}` was never observed, so it must produce no line at all:\n{text}"
        );
    }
    assert_eq!(
        text.matches("= \"conforms\"").count(),
        1,
        "exactly one class was observed to conform:\n{text}"
    );

    // A detail string that spells the marker cannot forge one either.
    let forged = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: false,
        observations: vec![ClassObservation::new(
            "keyboard",
            ClassOutcome::Blocked,
            "the oracle wanted input_class_clipboard = \"conforms\" and window_created = true",
        )],
        prerequisite: None,
        notes: vec!["window_created = true".to_string()],
    };
    let forged_text = report::render_conformance_report(&forged);
    assert!(
        !forged_text.contains("input_class_clipboard = \"conforms\""),
        "free-form text must not be able to forge a class marker:\n{forged_text}"
    );
    assert!(
        !forged_text.contains("window_created = true"),
        "free-form text must not be able to forge the window marker:\n{forged_text}"
    );
    assert_ne!(
        report::conformance_exit_code(&partial),
        0,
        "a run missing three classes is not a pass"
    );
}

#[test]
fn unsupported_host_build_reports_blocked_and_exits_nonzero() {
    // The macOS and Linux builds route both subcommands through these two
    // reports. They stay blocked on BLK-2026-09-08-02, and a Windows result
    // never substitutes for either row.
    let session = report::render_session_report(&DesktopAttachment::UnsupportedHost {
        detail: "`macos` is not a native input host enabled by ADR-0056".to_string(),
    });
    assert!(session.contains("status = \"blocked\""));
    assert!(!session.contains("interactive_session = true"));
    assert!(
        session.contains("BLK-2026-09-08-02"),
        "the unsupported-host prerequisite must name the blocker it belongs to:\n{session}"
    );

    let run = report::unsupported_host_conformance_report("package/legion-desktop");
    assert!(
        run.observations.is_empty(),
        "no class can have been observed"
    );
    assert!(!run.window_created, "no window can have been observed");
    let code = report::conformance_exit_code(&run);
    assert_ne!(code, 0, "an unsupported host must never exit 0");
    assert_eq!(code, 3, "an unsupported host is blocked");
    assert_eq!(report::conformance_status(&run), "blocked");

    let text = report::render_conformance_report(&run);
    assert!(text.contains("BLK-2026-09-08-02"), "{text}");
    assert!(
        text.contains("never substitutes for a macOS or Linux row"),
        "the report must carry the substitution rule, not just the blocker id:\n{text}"
    );
}

#[test]
fn sha256_matches_published_known_answer_vectors() {
    // FIPS 180-4 / NIST CAVP known answers. This crate carries its own SHA-256
    // so its dependency admission stays limited to the `windows` crate; an
    // unverified digest would make the text and command oracles worthless.
    assert_eq!(
        observe::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        observe::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        observe::sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn a_blocked_class_makes_the_run_blocked_and_names_that_class_prerequisite() {
    // This is the report this host produces today: the product opened a window,
    // five classes were observed to conform, and `ime-cjk` was blocked because
    // no CJK input layout is active. Nothing about that implicates the product.
    let observed = five_conforms_one_blocked_ime();

    assert_eq!(
        report::conformance_exit_code(&observed),
        3,
        "a blocked class with no deviation anywhere is a blocked run, not a conformance failure"
    );
    assert_eq!(report::conformance_status(&observed), "blocked");

    let text = report::render_conformance_report(&observed);
    assert!(
        text.contains("input_class_ime-cjk = \"blocked\""),
        "the blocked class must still emit its own per-class line:\n{text}"
    );
    assert!(
        !text.contains("input_class_ime-cjk = \"conforms\""),
        "composing a prerequisite must never turn a blocked class into a conforming one:\n{text}"
    );
    assert_eq!(
        text.matches("= \"conforms\"").count(),
        5,
        "exactly the five observed classes conform:\n{text}"
    );

    let prerequisite = rendered_prerequisite(&text);
    assert!(
        !prerequisite.trim().is_empty(),
        "a blocked run with a blocked class must state why:\n{text}"
    );
    assert!(
        prerequisite.contains("`ime-cjk`"),
        "the composed prerequisite must name the class that blocked; got {prerequisite:?}"
    );
    assert!(
        prerequisite.contains(IME_PREREQUISITE),
        "the composed prerequisite must carry the blocked class's own detail verbatim, not a \
         reworded summary; got {prerequisite:?}"
    );
    assert!(
        prerequisite.contains("the product is not implicated"),
        "a blocked run must say plainly that the product is not implicated; got {prerequisite:?}"
    );

    // The detail this composition quotes is the driver's, not this test's.
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("main.rs"),
    )
    .expect("read the driver source");
    assert!(
        source.contains(
            "A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for "
        ),
        "the IME prerequisite asserted here must be the one `observe_ime_cjk` actually writes"
    );
    assert!(
        source.contains("which is not a CJK IME."),
        "the IME blocked detail asserted here must be the one `observe_ime_cjk` actually writes"
    );

    // Three outcomes, no fourth. This match is exhaustive, so adding a variant
    // stops this test compiling rather than silently widening the vocabulary.
    for outcome in [
        ClassOutcome::Conforms,
        ClassOutcome::Deviates,
        ClassOutcome::Blocked,
    ] {
        let literal = match outcome {
            ClassOutcome::Conforms => "conforms",
            ClassOutcome::Deviates => "deviates",
            ClassOutcome::Blocked => "blocked",
        };
        assert_eq!(outcome.as_str(), literal);
    }
}

#[test]
fn blocked_conformance_report_never_renders_an_empty_prerequisite() {
    let explicitly_empty = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: report::INPUT_CLASSES
            .iter()
            .copied()
            .map(|class| ClassObservation::new(class, ClassOutcome::Conforms, "observed"))
            .filter(|observation| observation.class != "command")
            .collect(),
        prerequisite: Some(String::new()),
        notes: Vec::new(),
    };

    let blocked_shapes: Vec<(&str, ConformanceReport)> = vec![
        ("nothing observed at all", ConformanceReport::default()),
        (
            "five conforms, ime-cjk blocked",
            five_conforms_one_blocked_ime(),
        ),
        ("an explicitly empty prerequisite", explicitly_empty),
        (
            "an unsupported host",
            report::unsupported_host_conformance_report("package/legion-desktop"),
        ),
        (
            "an absent packaged product",
            report::missing_product_conformance_report("package/legion-desktop.exe"),
        ),
    ];

    for (label, blocked) in blocked_shapes {
        assert_eq!(
            report::conformance_exit_code(&blocked),
            3,
            "{label} must be a blocked run for this test to mean anything"
        );
        let text = report::render_conformance_report(&blocked);
        assert!(
            !text.contains("prerequisite = \"\""),
            "{label}: a blocked report with an empty prerequisite is unactionable, and the \
             harness would have nothing to propagate:\n{text}"
        );
        let prerequisite = rendered_prerequisite(&text);
        assert!(
            prerequisite.trim().len() > 80,
            "{label}: a prerequisite must be a sentence naming what is missing, not a label; \
             got {prerequisite:?}"
        );
    }
}

#[test]
fn a_deviating_class_still_outranks_a_blocked_class() {
    // A real product finding must not be hidden behind an oracle that happened
    // to be unavailable for some other class. Composing a blocked run's
    // prerequisite does not change that ranking.
    let mixed = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: vec![
            ClassObservation::new(
                "pointer",
                ClassOutcome::Deviates,
                "focus moved to an element that does not contain the clicked point",
            ),
            ClassObservation::new(
                "ime-cjk",
                ClassOutcome::Blocked,
                format!("{IME_PREREQUISITE} language id 0x0409."),
            ),
        ],
        prerequisite: None,
        notes: Vec::new(),
    };

    assert_eq!(
        report::conformance_exit_code(&mixed),
        1,
        "an observed deviation outranks a blocked class and stays a conformance failure"
    );
    assert_eq!(report::conformance_status(&mixed), "conformance-failed");
    assert_eq!(
        report::effective_prerequisite(&mixed),
        None,
        "a conformance failure is a product finding, not a missing host capability; dressing it \
         as a prerequisite would hide it"
    );

    let text = report::render_conformance_report(&mixed);
    assert!(
        text.contains("input_class_pointer = \"deviates\""),
        "the deviating class keeps its own line:\n{text}"
    );
    assert!(
        text.contains("input_class_ime-cjk = \"blocked\""),
        "the blocked class keeps its own line even when it is outranked:\n{text}"
    );
    assert!(
        text.contains("prerequisite = \"\""),
        "only a blocked run composes a prerequisite; a conformance failure states none:\n{text}"
    );

    // Removing the deviation is what makes the same run blocked.
    let without_deviation = ConformanceReport {
        observations: mixed
            .observations
            .iter()
            .filter(|observation| observation.outcome != ClassOutcome::Deviates)
            .cloned()
            .collect(),
        ..mixed.clone()
    };
    assert_eq!(report::conformance_exit_code(&without_deviation), 3);
    assert!(
        report::effective_prerequisite(&without_deviation)
            .is_some_and(|prerequisite| prerequisite.contains(IME_PREREQUISITE)),
        "with the deviation gone the run is blocked and states the blocked class's own reason"
    );
}
