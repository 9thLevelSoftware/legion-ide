//! ADR-0056 / `COMP-PLAT-002`: the native input acceptance harness must fail
//! *loudly and distinguishably* on a host that cannot answer the question.
//!
//! Every test here runs headless: no display session, no input driver, no
//! packaged binary. Driver discovery and child-process launching are injected
//! boundaries, so these assertions test the code and not the host.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::native_product_acceptance::{
    self as npa, AcceptanceReport, DriverAvailability, InputDriverProbe,
    NativeProductAcceptanceOptions, SubprocessLauncher, EXIT_BLOCKED, EXIT_CONFORMANCE_FAILED,
    EXIT_OPERATIONAL_ERROR, EXIT_PASSED, REPORT_FILE_NAME,
};

/// A driver-discovery fixture. The real probe touches the filesystem; this one
/// answers whatever the test needs, so no test depends on the host.
struct FixtureProbe {
    availability: DriverAvailability,
}

impl InputDriverProbe for FixtureProbe {
    fn probe(&self) -> DriverAvailability {
        self.availability.clone()
    }
}

/// A launcher that records every child process it is asked to start and starts
/// none of them.
#[derive(Default)]
struct RecordingLauncher {
    launches: Mutex<Vec<String>>,
}

impl RecordingLauncher {
    fn launch_count(&self) -> usize {
        self.launches.lock().expect("launch log").len()
    }
}

impl SubprocessLauncher for RecordingLauncher {
    fn launch(&self, program: &Path, args: &[String], _working_dir: &Path) -> io::Result<i32> {
        self.launches.lock().expect("launch log").push(format!(
            "{} {}",
            program.display(),
            args.join(" ")
        ));
        Ok(0)
    }
}

/// A launcher that plays a scripted driver run.
///
/// [`RecordingLauncher`] answers `Ok(0)` to everything and writes nothing, so
/// it cannot express "the driver exited 3 after writing this result". This is
/// its sibling rather than a change to it: the assertions that depend on a
/// launcher which starts nothing and returns zero stay exactly as they were.
struct ScriptedLauncher {
    session_exit_code: i32,
    session_result: String,
    run_exit_code: i32,
    run_result: Option<String>,
    launches: Mutex<Vec<String>>,
}

impl Default for ScriptedLauncher {
    fn default() -> Self {
        Self {
            session_exit_code: 0,
            session_result: "# legion-input-driver --probe-session\nstatus = \"attached\"\n\
                             interactive_session = true\n"
                .to_string(),
            run_exit_code: EXIT_PASSED,
            run_result: Some(driver_result_text(
                "passed",
                Some(0),
                true,
                &conforming_classes(),
                "",
                &[],
            )),
            launches: Mutex::new(Vec::new()),
        }
    }
}

impl ScriptedLauncher {
    /// A launcher whose conformance run exits `run_exit_code` after writing
    /// `run_result`. The session handshake succeeds, so the run is reached.
    fn conformance(run_exit_code: i32, run_result: String) -> Self {
        Self {
            run_exit_code,
            run_result: Some(run_result),
            ..Self::default()
        }
    }
}

impl SubprocessLauncher for ScriptedLauncher {
    fn launch(&self, program: &Path, args: &[String], _working_dir: &Path) -> io::Result<i32> {
        self.launches.lock().expect("launch log").push(format!(
            "{} {}",
            program.display(),
            args.join(" ")
        ));
        let report_path = args
            .iter()
            .position(|arg| arg == "--report")
            .and_then(|index| args.get(index + 1))
            .map(PathBuf::from)
            .expect("the harness always passes --report <path>");
        if args.iter().any(|arg| arg == "--probe-session") {
            fs::write(&report_path, self.session_result.as_bytes())?;
            return Ok(self.session_exit_code);
        }
        if let Some(result) = &self.run_result {
            fs::write(&report_path, result.as_bytes())?;
        }
        Ok(self.run_exit_code)
    }
}

/// The exact sentence `observe_ime_cjk` opens its blocked detail with. Written
/// out here, and cross-checked against the driver source by
/// `driver_blocked_result_carries_the_drivers_exact_prerequisite_string`, so
/// this fixture cannot drift into a prerequisite nothing actually emits.
const IME_PREREQUISITE: &str = concat!(
    "A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for ",
    "Japanese) and active as the input layout of the packaged product's window, so the ",
    "driver can drive a real composition by key injection rather than synthesizing a ",
    "commit."
);

/// The top-level prerequisite the driver composes for this host: five classes
/// conforming, `ime-cjk` blocked, its own detail quoted verbatim.
fn composed_ime_prerequisite() -> String {
    format!(
        "This host could not answer COMP-PLAT-002 for the packaged product, and the product is \
         not implicated. Supply what these observations name and re-run: input class `ime-cjk` \
         was blocked: {IME_PREREQUISITE} The product window's input layout reports language id \
         0x0409, which is not a CJK IME."
    )
}

fn conforming_classes() -> Vec<(&'static str, &'static str)> {
    npa::INPUT_CLASSES
        .iter()
        .copied()
        .map(|class| (class, "conforms"))
        .collect()
}

/// This host's expected result: a window, five conforming classes, `ime-cjk`
/// blocked.
fn five_conforms_one_blocked_ime() -> Vec<(&'static str, &'static str)> {
    npa::INPUT_CLASSES
        .iter()
        .copied()
        .map(|class| {
            if class == "ime-cjk" {
                (class, "blocked")
            } else {
                (class, "conforms")
            }
        })
        .collect()
}

/// Escape for a TOML basic string, exactly as the driver's `toml_string` does
/// after its marker scrub.
fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Render a `driver_result.toml` in the shape
/// `crates/legion-input-driver/src/report.rs` writes.
fn driver_result_text(
    status: &str,
    exit_code: Option<i32>,
    window_created: bool,
    classes: &[(&str, &str)],
    prerequisite: &str,
    notes: &[&str],
) -> String {
    let mut text = String::new();
    text.push_str("# legion-input-driver --conformance-run (ADR-0056)\n");
    text.push_str("schema_version = 1\n");
    text.push_str("driver = \"legion-input-driver\"\n");
    text.push_str("mode = \"conformance-run\"\n");
    text.push_str(&format!("status = \"{}\"\n", escape(status)));
    if let Some(exit_code) = exit_code {
        text.push_str(&format!("exit_code = {exit_code}\n"));
    }
    text.push_str("product = \"package/legion-desktop.exe\"\n");
    text.push_str(&format!("window_created = {window_created}\n"));
    for (class, outcome) in classes {
        text.push_str(&format!("input_class_{class} = \"{outcome}\"\n"));
        text.push_str(&format!("input_class_{class}_detail = \"observed\"\n"));
    }
    text.push_str(&format!("prerequisite = \"{}\"\n", escape(prerequisite)));
    text.push_str(&format!(
        "notes = [{}]\n",
        notes
            .iter()
            .map(|note| format!("\"{}\"", escape(note)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    text
}

fn temp_workspace(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "legion-native-product-acceptance-{tag}-{}-{nanos}",
        process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    dir
}

fn options() -> NativeProductAcceptanceOptions {
    NativeProductAcceptanceOptions {
        out_dir: "out".to_string(),
        package_dir: "package".to_string(),
        driver_path: Some("tools/native-input-driver/fixture-driver".to_string()),
    }
}

fn unavailable_probe() -> FixtureProbe {
    FixtureProbe {
        availability: DriverAvailability::Unavailable {
            prerequisite: npa::PREREQUISITE_DRIVER_MISSING.to_string(),
        },
    }
}

fn available_probe() -> FixtureProbe {
    FixtureProbe {
        availability: DriverAvailability::Available {
            driver_path: "tools/native-input-driver/fixture-driver".to_string(),
        },
    }
}

/// Create an empty file at `path`, parents included. Used to stand in for a
/// built or staged driver binary without building one.
fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("create {}: {err}", parent.display()));
    }
    fs::write(path, b"").unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

/// A workspace with a packaged product executable staged, so a run reaches the
/// driver's conformance launch instead of stopping at the package prerequisite.
fn staged_workspace(tag: &str) -> PathBuf {
    let workspace = temp_workspace(tag);
    touch(
        &workspace
            .join("package")
            .join(npa::packaged_executable_name()),
    );
    workspace
}

fn report_text(workspace: &Path) -> String {
    let path = workspace.join("out").join(REPORT_FILE_NAME);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// Value of a `key = "value"` line, unescaped only for the escapes this
/// harness emits.
fn toml_string_field(text: &str, key: &str) -> String {
    let prefix = format!("{key} = \"");
    let line = text
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("report has no `{key}` field:\n{text}"));
    let body = &line[prefix.len()..];
    let body = body
        .strip_suffix('"')
        .unwrap_or_else(|| panic!("unterminated `{key}` field: {line}"));
    body.replace("\\\\", "\\").replace("\\\"", "\"")
}

#[test]
fn driver_discovery_prefers_the_built_in_repo_driver_over_the_staged_path() {
    // The driver is a workspace crate now, so discovery looks first where cargo
    // puts it. The staged `tools/` path is retained, last, so a binary an owner
    // put there by hand still works — but a freshly built one must win over a
    // stale staged one, or a rebuild would silently not be what ran.
    let candidates = npa::default_driver_candidates();
    assert_eq!(
        candidates.len(),
        3,
        "discovery probes release, then debug, then the retained staged path: {candidates:?}"
    );
    assert!(
        candidates[0].starts_with("target/release/"),
        "the release build is probed first: {candidates:?}"
    );
    assert!(
        candidates[1].starts_with("target/debug/"),
        "the debug build is probed second: {candidates:?}"
    );
    assert!(
        candidates[2].starts_with("tools/native-input-driver/"),
        "the owner-staged path is retained, last: {candidates:?}"
    );
    for candidate in &candidates {
        assert!(
            candidate.ends_with(npa::driver_executable_name()),
            "every candidate names this host's driver binary: {candidate}"
        );
    }

    let workspace = temp_workspace("discovery");
    let resolved = candidates
        .iter()
        .map(|candidate| workspace.join(candidate))
        .collect::<Vec<_>>();

    assert_eq!(
        npa::first_existing_driver(&resolved),
        None,
        "with nothing built and nothing staged, discovery must find nothing rather than \
         invent a path"
    );

    touch(&resolved[2]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[2].as_path()),
        "a staged binary is still discovered when nothing has been built"
    );

    touch(&resolved[1]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[1].as_path()),
        "a debug build outranks the staged path"
    );

    touch(&resolved[0]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[0].as_path()),
        "the release build outranks both"
    );

    // The host probe answers from that same ordered list, and never builds
    // anything to make an answer come out.
    match npa::HostInputDriverProbe::new(resolved.clone()).probe() {
        DriverAvailability::Available { driver_path } => {
            assert_eq!(driver_path, resolved[0].display().to_string());
        }
        DriverAvailability::Unavailable { prerequisite } => {
            // A driver file exists at `resolved[0]`, so the only build that may
            // still answer unavailable is one ADR-0056 does not enable at all.
            assert_ne!(
                std::env::consts::OS,
                "windows",
                "on Windows a driver that exists on disk must be discovered; got {prerequisite:?}"
            );
            assert_eq!(
                prerequisite,
                npa::PREREQUISITE_UNSUPPORTED_HOST,
                "macOS and Linux stay blocked on BLK-2026-09-08-02 whatever is on disk"
            );
        }
    }

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn driver_missing_prerequisite_names_the_in_repo_build_command() {
    let prerequisite = npa::PREREQUISITE_DRIVER_MISSING;
    assert!(
        prerequisite.contains("cargo build -p legion-input-driver --release"),
        "the driver is built from this repository, so the prerequisite must name the build \
         command, not an installation the owner has to source: {prerequisite:?}"
    );
    assert!(
        prerequisite.contains("target/release/legion-input-driver.exe"),
        "the prerequisite must say where that build lands: {prerequisite:?}"
    );

    let build_at = prerequisite
        .find("cargo build -p legion-input-driver --release")
        .expect("build command present");
    let staged_at = prerequisite
        .find("tools/native-input-driver/")
        .expect("the retained staged path stays documented, because discovery still probes it");
    assert!(
        build_at < staged_at,
        "the in-repo build is the action to take; the staged path is the retained fallback \
         mentioned after it: {prerequisite:?}"
    );

    // And the blocked report carries that exact string, not a paraphrase.
    let workspace = temp_workspace("build-command");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "prerequisite"), prerequisite);
    assert_eq!(
        launcher.launch_count(),
        0,
        "naming a build command must not make the harness run one"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn unavailable_driver_returns_blocked_with_a_nonzero_exit() {
    let workspace = temp_workspace("unavailable");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);

    assert_ne!(
        code, 0,
        "a host with no input driver must never exit 0; that is how a blocked run gets read \
         as a pass"
    );
    assert_eq!(
        code, EXIT_BLOCKED,
        "an unavailable driver is blocked, not a conformance failure"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        text.contains("driver_available = false"),
        "report must record that discovery found no driver:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_result_carries_an_exact_actionable_prerequisite_string() {
    let workspace = temp_workspace("prerequisite");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let text = report_text(&workspace);
    let prerequisite = toml_string_field(&text, "prerequisite");

    assert!(
        prerequisite.len() > 80,
        "a prerequisite must be a sentence naming what the owner supplies, not a label; got \
         {prerequisite:?}"
    );
    let lowered = prerequisite.to_lowercase();
    for needle in ["host", "session", "driver", "install"] {
        assert!(
            lowered.contains(needle),
            "prerequisite must name the {needle} the owner has to supply; got {prerequisite:?}"
        );
    }
    assert!(
        lowered.contains("keyboard") && lowered.contains("clipboard") && lowered.contains("ime"),
        "prerequisite must say what the driver has to be able to do; got {prerequisite:?}"
    );
    assert_ne!(
        lowered.trim(),
        "driver unavailable",
        "a prerequisite that cannot be acted on is a dead end"
    );
    assert_eq!(
        prerequisite,
        npa::PREREQUISITE_DRIVER_MISSING,
        "the report must carry the published prerequisite verbatim"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_exit_code_is_distinct_from_the_conformance_failure_exit_code() {
    // Two concrete integers. If a later change ever makes them equal, CI can no
    // longer tell "the product is wrong" from "this machine cannot answer the
    // question", and this test fails.
    assert_eq!(EXIT_BLOCKED, 3);
    assert_eq!(EXIT_CONFORMANCE_FAILED, 1);
    assert_ne!(
        EXIT_BLOCKED, EXIT_CONFORMANCE_FAILED,
        "blocked and conformance-failed must never collapse into one nonzero code"
    );

    assert_eq!(EXIT_PASSED, 0);
    assert_eq!(EXIT_OPERATIONAL_ERROR, 2);
    let codes = [
        EXIT_PASSED,
        EXIT_CONFORMANCE_FAILED,
        EXIT_OPERATIONAL_ERROR,
        EXIT_BLOCKED,
    ];
    for (index, first) in codes.iter().enumerate() {
        for second in codes.iter().skip(index + 1) {
            assert_ne!(first, second, "exit codes must be pairwise distinct");
        }
    }
    assert!(
        !codes.iter().skip(1).any(|code| *code == 0),
        "no non-pass outcome may exit 0"
    );
}

#[test]
fn blocked_report_records_neither_passed_nor_skipped() {
    let workspace = temp_workspace("status");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let text = report_text(&workspace);
    let status = toml_string_field(&text, "status");
    assert_eq!(status, "blocked");
    assert!(
        !status.contains("passed"),
        "a blocked run must not report a pass; status was {status:?}"
    );
    assert!(
        !status.contains("skipped"),
        "a blocked run must not report a skip; status was {status:?}"
    );
    assert!(
        !text.contains("status = \"passed\"") && !text.contains("status = \"skipped\""),
        "no status line in a blocked report may read passed or skipped:\n{text}"
    );
    assert!(
        text.contains("window_created = false") && text.contains("input_classes_observed = []"),
        "a blocked run records no positive evidence:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn no_subprocess_is_spawned_when_the_driver_is_unavailable() {
    let workspace = temp_workspace("no-spawn");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    assert_eq!(
        launcher.launch_count(),
        0,
        "an unavailable driver must start no child process; spawning a windowed binary on a \
         headless machine hangs CI. Recorded: {:?}",
        launcher.launches.lock().expect("launch log")
    );
    let text = report_text(&workspace);
    assert!(
        text.contains("subprocess_launched = false"),
        "the report must state that nothing was launched:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn missing_packaged_binary_is_reported_blocked_rather_than_failed() {
    let workspace = temp_workspace("no-package");
    // Driver present, package absent: the host cannot answer the question and
    // the product is not implicated.
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);

    assert_eq!(
        code, EXIT_BLOCKED,
        "an absent packaged product is a missing prerequisite, not a product defect"
    );
    assert_ne!(
        code, EXIT_CONFORMANCE_FAILED,
        "blaming the product for a missing package would file a false defect"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        text.contains("driver_available = true")
            && text.contains("packaged_product_present = false"),
        "the report must distinguish which prerequisite was missing:\n{text}"
    );
    let prerequisite = toml_string_field(&text, "prerequisite").to_lowercase();
    assert!(
        prerequisite.contains("packaged") && prerequisite.contains("product"),
        "the prerequisite must name the packaged product; got {prerequisite:?}"
    );
    assert_eq!(
        launcher.launch_count(),
        0,
        "nothing may be launched when there is no packaged product to launch"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn report_is_written_and_readable_even_when_the_run_is_blocked() {
    let workspace = temp_workspace("report");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let path = workspace.join("out").join(REPORT_FILE_NAME);
    assert!(
        path.is_file(),
        "a blocked run that leaves no artifact is unauditable: {}",
        path.display()
    );
    let text = fs::read_to_string(&path).expect("blocked report must be readable");
    assert!(!text.trim().is_empty(), "blocked report must not be empty");
    assert!(
        text.contains("requirement_id = \"COMP-PLAT-002\""),
        "the report must name the requirement it is about:\n{text}"
    );
    assert!(
        text.contains("exit_code = 3"),
        "the report must record the exit code it returned:\n{text}"
    );

    // Rendering is pure, so the same report text is reproducible.
    let rendered = npa::render_report(&AcceptanceReport {
        status: "blocked".to_string(),
        exit_code: EXIT_BLOCKED,
        prerequisite: Some(npa::PREREQUISITE_DRIVER_MISSING.to_string()),
        driver_available: false,
        interactive_session: false,
        packaged_product_present: false,
        subprocess_launched: false,
        window_created: false,
        input_classes_observed: Vec::new(),
        input_classes_blocked: Vec::new(),
        input_classes_deviating: Vec::new(),
        driver_path: "tools/native-input-driver/fixture-driver".to_string(),
        package_dir: "package".to_string(),
        notes: Vec::new(),
    });
    assert!(rendered.contains("status = \"blocked\""));

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn command_is_not_referenced_by_any_pr_gate_workflow() {
    let workflows = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
    let entries = fs::read_dir(&workflows)
        .unwrap_or_else(|err| panic!("read {}: {err}", workflows.display()));

    let mut inspected = 0;
    for entry in entries {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "yml" || ext == "yaml");
        if !is_yaml {
            continue;
        }
        inspected += 1;
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        assert!(
            !text.contains("native-product-acceptance"),
            "{} references native-product-acceptance; this harness has never produced a real \
             result and must not gate merges. Promoting it is a deliberate change that has to \
             edit this test.",
            path.display()
        );
    }
    assert!(
        inspected > 0,
        "expected workflow files under {}",
        workflows.display()
    );
}

/// Items of a `key = ["a", "b"]` line.
fn toml_list_field(text: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key} = [");
    let line = text
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("report has no `{key}` field:\n{text}"));
    let body = line[prefix.len()..]
        .strip_suffix(']')
        .unwrap_or_else(|| panic!("unterminated `{key}` field: {line}"));
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.split(", ")
        .map(|item| item.trim().trim_matches('"').to_string())
        .collect()
}

/// The driver result this host is expected to produce, and the launcher that
/// plays it: a window, five conforming classes, `ime-cjk` blocked, exit 3.
fn this_hosts_blocked_run() -> ScriptedLauncher {
    ScriptedLauncher::conformance(
        EXIT_BLOCKED,
        driver_result_text(
            "blocked",
            Some(EXIT_BLOCKED),
            true,
            &five_conforms_one_blocked_ime(),
            &composed_ime_prerequisite(),
            &[],
        ),
    )
}

#[test]
fn driver_blocked_exit_is_propagated_as_blocked_with_its_own_exit_code() {
    let workspace = staged_workspace("driver-blocked");
    let launcher = this_hosts_blocked_run();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);

    assert_eq!(
        code, EXIT_BLOCKED,
        "the driver's own blocked outcome is the run's outcome; collapsing it into a conformance \
         failure blames the product for a host that cannot answer the question"
    );
    assert_ne!(code, EXIT_CONFORMANCE_FAILED);
    assert_ne!(code, EXIT_PASSED);
    assert_ne!(code, EXIT_OPERATIONAL_ERROR);

    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        text.contains("exit_code = 3"),
        "the report must record the blocked exit code it returned:\n{text}"
    );
    let launches = launcher.launches.lock().expect("launch log").clone();
    assert_eq!(
        launches.len(),
        2,
        "the session handshake and the conformance run are the only two launches: {launches:?}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn driver_blocked_result_carries_the_drivers_exact_prerequisite_string() {
    let workspace = staged_workspace("driver-prerequisite");
    let launcher = this_hosts_blocked_run();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let text = report_text(&workspace);
    let prerequisite = toml_string_field(&text, "prerequisite");
    assert_eq!(
        prerequisite,
        composed_ime_prerequisite(),
        "the harness must propagate the driver's prerequisite verbatim; a reworded prerequisite \
         is a fabricated one"
    );
    assert!(
        prerequisite.contains(IME_PREREQUISITE),
        "the propagated string must still carry the blocked class's own prerequisite sentence; \
         got {prerequisite:?}"
    );
    for own in [
        npa::PREREQUISITE_DRIVER_MISSING,
        npa::PREREQUISITE_SESSION_MISSING,
        npa::PREREQUISITE_PACKAGE_MISSING,
        npa::PREREQUISITE_UNSUPPORTED_HOST,
        npa::PREREQUISITE_DRIVER_STATED_NO_REASON,
    ] {
        assert!(
            !prerequisite.contains(own),
            "the driver's prerequisite must not be merged with one of the harness's own: \
             {prerequisite:?}"
        );
    }

    // The sentence this fixture quotes is the driver's, not this test's.
    let driver_source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/legion-input-driver/src/main.rs"),
    )
    .expect("read the driver source");
    assert!(
        driver_source.contains(
            "A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for "
        ),
        "the IME prerequisite this fixture propagates must be the one `observe_ime_cjk` writes"
    );
    assert!(
        driver_source.contains("which is not a CJK IME."),
        "the IME blocked detail this fixture propagates must be the one `observe_ime_cjk` writes"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn a_blocked_input_class_is_never_reported_as_a_conformance_failure() {
    // The artifact this host produces today, built exactly: a created window,
    // five classes observed to conform, `ime-cjk` blocked because no CJK input
    // layout is active, driver exit 3. The driver behaved perfectly and the
    // product did nothing wrong, so nothing here may be eligible to become a
    // defect against it.
    let workspace = staged_workspace("blocked-class");
    let launcher = this_hosts_blocked_run();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);

    assert_eq!(
        code, EXIT_BLOCKED,
        "a blocked class with no deviation anywhere is a blocked run"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert_eq!(
        toml_string_field(&text, "prerequisite"),
        composed_ime_prerequisite()
    );
    assert!(
        !text.contains("conformance-failed"),
        "the string `conformance-failed` must appear nowhere in this artifact; if it does, this \
         report is eligible to become a defect against a product that did nothing wrong:\n{text}"
    );
    assert_eq!(
        toml_list_field(&text, "input_classes_blocked"),
        vec!["ime-cjk".to_string()],
        "the artifact must name the class that blocked:\n{text}"
    );
    assert!(
        toml_list_field(&text, "input_classes_deviating").is_empty(),
        "no class deviated:\n{text}"
    );
    assert_eq!(
        toml_list_field(&text, "input_classes_observed").len(),
        5,
        "the five classes that did conform are still recorded:\n{text}"
    );
    assert!(
        text.contains("window_created = true"),
        "the window really was observed, and a blocked run does not erase that:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn conformance_failed_requires_a_driver_that_reported_a_deviation_itself() {
    // The one route in: a driver that ran to completion, observed a deviation,
    // and exited 1 saying so.
    let deviating = npa::INPUT_CLASSES
        .iter()
        .copied()
        .map(|class| {
            if class == "pointer" {
                (class, "deviates")
            } else {
                (class, "conforms")
            }
        })
        .collect::<Vec<_>>();
    let workspace = staged_workspace("deviation");
    let launcher = ScriptedLauncher::conformance(
        EXIT_CONFORMANCE_FAILED,
        driver_result_text(
            "conformance-failed",
            Some(EXIT_CONFORMANCE_FAILED),
            true,
            &deviating,
            "",
            &[],
        ),
    );
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(
        code, EXIT_CONFORMANCE_FAILED,
        "an observed deviation is a product finding and must still be reported as one"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "conformance-failed");
    assert_eq!(
        toml_list_field(&text, "input_classes_deviating"),
        vec!["pointer".to_string()]
    );
    let _ = fs::remove_dir_all(&workspace);

    // And no other driver outcome reaches it. Each of these is a driver that
    // did not report a deviation itself.
    let not_a_deviation: Vec<(&str, ScriptedLauncher)> = vec![
        ("blocked", this_hosts_blocked_run()),
        (
            "operational-error",
            ScriptedLauncher::conformance(
                EXIT_OPERATIONAL_ERROR,
                driver_result_text("blocked", Some(EXIT_OPERATIONAL_ERROR), false, &[], "", &[]),
            ),
        ),
        (
            "an unknown code",
            ScriptedLauncher::conformance(
                7,
                driver_result_text("blocked", Some(7), false, &[], "", &[]),
            ),
        ),
        (
            "exit 0 with a result that contradicts it",
            ScriptedLauncher::conformance(
                EXIT_PASSED,
                driver_result_text("passed", Some(0), false, &conforming_classes(), "", &[]),
            ),
        ),
        (
            "a result that is not parseable at all",
            ScriptedLauncher::conformance(EXIT_BLOCKED, "not: valid = toml [\n".to_string()),
        ),
        (
            "exit 1 with no class that deviates",
            ScriptedLauncher::conformance(
                EXIT_CONFORMANCE_FAILED,
                driver_result_text(
                    "conformance-failed",
                    Some(EXIT_CONFORMANCE_FAILED),
                    true,
                    &conforming_classes(),
                    "",
                    &[],
                ),
            ),
        ),
        (
            "exit 1 with a result whose own status denies a deviation",
            ScriptedLauncher::conformance(
                EXIT_CONFORMANCE_FAILED,
                driver_result_text(
                    "blocked",
                    Some(EXIT_CONFORMANCE_FAILED),
                    true,
                    &five_conforms_one_blocked_ime(),
                    &composed_ime_prerequisite(),
                    &[],
                ),
            ),
        ),
    ];
    for (label, launcher) in not_a_deviation {
        let workspace = staged_workspace("no-deviation");
        let code = npa::run_native_product_acceptance(
            &workspace,
            &options(),
            &available_probe(),
            &launcher,
        );
        assert_ne!(
            code, EXIT_CONFORMANCE_FAILED,
            "{label}: conformance-failed must be reachable only from a driver that reported a \
             deviation itself"
        );
        let text = report_text(&workspace);
        assert_ne!(
            toml_string_field(&text, "status"),
            "conformance-failed",
            "{label}: the artifact must not blame the product:\n{text}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }
}

#[test]
fn unknown_driver_exit_code_is_an_operational_error_not_a_conformance_failure() {
    for code in [7, 42, 255, -1] {
        let workspace = staged_workspace("unknown-code");
        let launcher = ScriptedLauncher::conformance(
            code,
            driver_result_text(
                "blocked",
                Some(code),
                true,
                &five_conforms_one_blocked_ime(),
                &composed_ime_prerequisite(),
                &[],
            ),
        );
        let returned = npa::run_native_product_acceptance(
            &workspace,
            &options(),
            &available_probe(),
            &launcher,
        );
        assert_eq!(
            returned, EXIT_OPERATIONAL_ERROR,
            "exit {code} is not one of the four outcome codes, so the harness cannot read a \
             product outcome from it"
        );
        assert_ne!(returned, EXIT_CONFORMANCE_FAILED);
        let text = report_text(&workspace);
        assert_eq!(toml_string_field(&text, "status"), "operational-error");
        assert!(
            text.contains("not one of the four outcome codes"),
            "the artifact must say why the code was unusable:\n{text}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }
}

#[test]
fn driver_exit_zero_contradicting_its_own_result_is_an_operational_error() {
    // `conformance_exit_code` in the driver returns 0 if and only if its report
    // holds a created window and six conforming classes. A driver that exits 0
    // while its own result says otherwise is a broken instrument, and calling
    // that a conformance failure would be the same bug in a new place.
    let contradictions: Vec<(&str, ScriptedLauncher, &str)> = vec![
        (
            "exit 0 with no window",
            ScriptedLauncher::conformance(
                EXIT_PASSED,
                driver_result_text("passed", Some(0), false, &conforming_classes(), "", &[]),
            ),
            "does not corroborate a pass",
        ),
        (
            "exit 0 with five of six classes",
            ScriptedLauncher::conformance(
                EXIT_PASSED,
                driver_result_text(
                    "passed",
                    Some(0),
                    true,
                    &five_conforms_one_blocked_ime(),
                    "",
                    &[],
                ),
            ),
            "does not corroborate a pass",
        ),
        (
            "exit 0 with a result whose own status denies it",
            ScriptedLauncher::conformance(
                EXIT_PASSED,
                driver_result_text("blocked", None, true, &conforming_classes(), "", &[]),
            ),
            "does not corroborate a pass",
        ),
        (
            "exit 0 with a result that reports exit_code = 3",
            ScriptedLauncher::conformance(
                EXIT_PASSED,
                driver_result_text("blocked", Some(3), true, &conforming_classes(), "", &[]),
            ),
            "exit_code = 3",
        ),
        (
            "exit 3 with a result that reports exit_code = 0",
            ScriptedLauncher::conformance(
                EXIT_BLOCKED,
                driver_result_text("passed", Some(0), true, &conforming_classes(), "", &[]),
            ),
            "exit_code = 0",
        ),
    ];

    for (label, launcher, expected_note) in contradictions {
        let workspace = staged_workspace("contradiction");
        let code = npa::run_native_product_acceptance(
            &workspace,
            &options(),
            &available_probe(),
            &launcher,
        );
        assert_eq!(
            code, EXIT_OPERATIONAL_ERROR,
            "{label}: an instrument that contradicts itself is an operational error"
        );
        assert_ne!(code, EXIT_PASSED, "{label}: it is certainly not a pass");
        assert_ne!(
            code, EXIT_CONFORMANCE_FAILED,
            "{label}: a broken instrument is not a broken product"
        );
        let text = report_text(&workspace);
        assert_eq!(toml_string_field(&text, "status"), "operational-error");
        assert!(
            text.contains(expected_note),
            "{label}: the artifact must name what contradicted what:\n{text}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    // Both numbers, named, on the disagreement path.
    let workspace = staged_workspace("both-numbers");
    let launcher = ScriptedLauncher::conformance(
        EXIT_BLOCKED,
        driver_result_text("passed", Some(0), true, &conforming_classes(), "", &[]),
    );
    npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    let text = report_text(&workspace);
    assert!(
        text.contains("exited 3") && text.contains("exit_code = 0"),
        "the note must name both the process exit code and the reported one:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_report_never_emits_an_empty_prerequisite() {
    // A driver that reports blocked without stating a reason. The harness does
    // not invent one, and it does not publish an empty one either.
    let workspace = staged_workspace("empty-prerequisite");
    let launcher = ScriptedLauncher::conformance(
        EXIT_BLOCKED,
        driver_result_text(
            "blocked",
            Some(EXIT_BLOCKED),
            true,
            &five_conforms_one_blocked_ime(),
            "",
            &[],
        ),
    );
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);
    let text = report_text(&workspace);
    assert!(
        !text.contains("prerequisite = \"\""),
        "a blocked result with an empty prerequisite cannot be acted on:\n{text}"
    );
    assert_eq!(
        toml_string_field(&text, "prerequisite"),
        npa::PREREQUISITE_DRIVER_STATED_NO_REASON,
        "what is missing is a driver result that states its own reason, and the harness says so \
         rather than inventing a host prerequisite"
    );
    let _ = fs::remove_dir_all(&workspace);

    // Every other blocked path this command has, for the same property.
    let session_refused = ScriptedLauncher {
        session_exit_code: EXIT_BLOCKED,
        ..ScriptedLauncher::default()
    };
    let session_silent = ScriptedLauncher {
        session_result: "status = \"blocked\"\ninteractive_session = false\n".to_string(),
        ..ScriptedLauncher::default()
    };

    for launcher in [session_refused, session_silent] {
        let workspace = staged_workspace("session-blocked");
        let code = npa::run_native_product_acceptance(
            &workspace,
            &options(),
            &available_probe(),
            &launcher,
        );
        assert_eq!(code, EXIT_BLOCKED);
        let text = report_text(&workspace);
        assert!(
            !text.contains("prerequisite = \"\""),
            "no blocked path may publish an empty prerequisite:\n{text}"
        );
        assert!(toml_string_field(&text, "prerequisite").len() > 80);
        let _ = fs::remove_dir_all(&workspace);
    }

    // An unavailable driver, and a driver with no packaged product staged.
    for probe in [unavailable_probe(), available_probe()] {
        let workspace = temp_workspace("blocked-paths");
        let launcher = RecordingLauncher::default();
        let code = npa::run_native_product_acceptance(&workspace, &options(), &probe, &launcher);
        assert_eq!(code, EXIT_BLOCKED);
        let text = report_text(&workspace);
        assert!(
            !text.contains("prerequisite = \"\""),
            "no blocked path may publish an empty prerequisite:\n{text}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }
}

#[test]
fn a_note_in_the_driver_result_cannot_forge_the_status_field() {
    // The driver's `toml_string` escapes quotes and newlines, so a note cannot
    // open a new line. Prove it end to end rather than trusting it: this result
    // is blocked, and its notes try to say otherwise.
    let forged = driver_result_text(
        "blocked",
        Some(EXIT_BLOCKED),
        true,
        &five_conforms_one_blocked_ime(),
        &composed_ime_prerequisite(),
        &[
            "status = \"passed\"",
            "exit_code = 0",
            "prerequisite = \"nothing is missing on this host\"",
        ],
    );

    let fields = npa::parse_driver_result_fields(&forged);
    assert_eq!(
        fields.status.as_deref(),
        Some("blocked"),
        "the top-level status is the one the driver wrote, not the one a note spells"
    );
    assert_eq!(fields.exit_code, Some(i64::from(EXIT_BLOCKED)));
    assert_eq!(
        fields.prerequisite.as_deref(),
        Some(composed_ime_prerequisite().as_str())
    );

    let workspace = staged_workspace("forged-note");
    let launcher = ScriptedLauncher::conformance(EXIT_BLOCKED, forged);
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(
        code, EXIT_BLOCKED,
        "a forged note must not turn a blocked run into a pass"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        !text.contains("status = \"passed\""),
        "no status line in this artifact may read passed:\n{text}"
    );
    assert_eq!(
        toml_string_field(&text, "prerequisite"),
        composed_ime_prerequisite(),
        "the forged prerequisite in the notes must not reach the artifact"
    );

    // A duplicated key states nothing at all, and nothing is never a value.
    let duplicated = npa::parse_driver_result_fields("status = \"passed\"\nstatus = \"blocked\"\n");
    assert_eq!(
        duplicated,
        npa::DriverResultFields::default(),
        "a duplicated key is not a value; the document states nothing"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_and_deviating_classes_are_named_in_the_harness_report() {
    // An operator reading a blocked artifact must be able to tell "five
    // conformed and `ime-cjk` had no oracle" from "nothing ran at all".
    let workspace = staged_workspace("named-blocked");
    let launcher = this_hosts_blocked_run();
    npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    let text = report_text(&workspace);
    assert_eq!(
        toml_list_field(&text, "input_classes_blocked"),
        vec!["ime-cjk".to_string()]
    );
    assert!(toml_list_field(&text, "input_classes_deviating").is_empty());
    assert_eq!(
        toml_list_field(&text, "required_input_classes").len(),
        npa::INPUT_CLASSES.len(),
        "the additive fields must not disturb the existing ones:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);

    // And on a run that both deviated and blocked, each class lands in the
    // field that describes what was actually observed of it.
    let mixed = npa::INPUT_CLASSES
        .iter()
        .copied()
        .map(|class| match class {
            "pointer" => (class, "deviates"),
            "text" => (class, "blocked"),
            other => (other, "conforms"),
        })
        .collect::<Vec<_>>();
    let workspace = staged_workspace("named-mixed");
    let launcher = ScriptedLauncher::conformance(
        EXIT_CONFORMANCE_FAILED,
        driver_result_text(
            "conformance-failed",
            Some(EXIT_CONFORMANCE_FAILED),
            true,
            &mixed,
            "",
            &[],
        ),
    );
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(
        code, EXIT_CONFORMANCE_FAILED,
        "a deviation the driver observed still outranks a blocked class"
    );
    let text = report_text(&workspace);
    assert_eq!(
        toml_list_field(&text, "input_classes_deviating"),
        vec!["pointer".to_string()]
    );
    assert_eq!(
        toml_list_field(&text, "input_classes_blocked"),
        vec!["text".to_string()]
    );
    assert_eq!(
        toml_list_field(&text, "input_classes_observed"),
        vec![
            "keyboard".to_string(),
            "clipboard".to_string(),
            "ime-cjk".to_string(),
            "command".to_string(),
        ]
    );
    let _ = fs::remove_dir_all(&workspace);
}

struct FailingLauncher;

impl SubprocessLauncher for FailingLauncher {
    fn launch(&self, _program: &Path, _args: &[String], _working_dir: &Path) -> io::Result<i32> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fixture driver is not executable",
        ))
    }
}

#[test]
fn handshake_spawn_failure_is_an_operational_error_and_records_no_subprocess() {
    let workspace = staged_workspace("handshake-spawn");
    let code = npa::run_native_product_acceptance(
        &workspace,
        &options(),
        &available_probe(),
        &FailingLauncher,
    );
    assert_eq!(code, EXIT_OPERATIONAL_ERROR);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "operational-error");
    assert!(
        text.contains("subprocess_launched = false"),
        "a spawn that never started a child must not claim one ran:\n{text}"
    );
    assert!(
        text.contains("could not start"),
        "the artifact must name the spawn failure:\n{text}"
    );
    assert!(
        !text.contains("interactive desktop session"),
        "a missing process is not a missing desktop:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn exit_one_without_a_recorded_deviation_is_an_operational_error() {
    let workspace = staged_workspace("exit-one-empty");
    let launcher = ScriptedLauncher::conformance(
        EXIT_CONFORMANCE_FAILED,
        driver_result_text(
            "conformance-failed",
            Some(EXIT_CONFORMANCE_FAILED),
            true,
            &conforming_classes(),
            "",
            &[],
        ),
    );
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(code, EXIT_OPERATIONAL_ERROR);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "operational-error");
    assert!(
        text.contains("does not corroborate a product deviation"),
        "the artifact must name the missing deviation:\n{text}"
    );
    assert!(
        toml_list_field(&text, "input_classes_deviating").is_empty(),
        "no class was observed to deviate:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn a_comment_cannot_forge_window_created_or_a_class_outcome() {
    let mut forged = String::from(
        "status = \"passed\"\nexit_code = 0\n# window_created = true\nwindow_created = false\n",
    );
    for class in npa::INPUT_CLASSES {
        forged.push_str(&format!(
            "input_class_{class}_detail = \"input_class_{class} = \\\"conforms\\\"\"\n"
        ));
    }
    let fields = npa::parse_driver_result_fields(&forged);
    assert_eq!(fields.window_created, Some(false));
    let workspace = staged_workspace("forged-window");
    let launcher = ScriptedLauncher::conformance(EXIT_PASSED, forged);
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(code, EXIT_OPERATIONAL_ERROR);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "operational-error");
    assert!(
        text.contains("window_created=false"),
        "a commented marker must not count as a created window:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn uncleared_prior_driver_result_is_an_operational_error() {
    let workspace = staged_workspace("stale-result");
    let result_path = workspace.join("out").join(npa::DRIVER_RESULT_FILE_NAME);
    fs::create_dir_all(&result_path).unwrap();
    let launcher = ScriptedLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);
    assert_eq!(code, EXIT_OPERATIONAL_ERROR);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "operational-error");
    assert!(
        text.contains("cannot clear prior driver result"),
        "the artifact must name the uncleared evidence:\n{text}"
    );
    let _ = fs::remove_dir_all(&workspace);
}
