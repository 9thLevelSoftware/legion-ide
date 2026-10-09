//! Bounded SC-MANUAL-OPEN-TYPE-SAVE steps 1–6, using the existing external
//! driver only. Never a six-class conformance or complete scenario result.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub struct JourneyOutput {
    pub session_state: PathBuf,
    report_file: fs::File,
}

/// Reserve new evidence and an empty session directory outside the clone.
/// Never restore, overwrite or reset the workspace's default session metadata.
pub fn reserve_journey_output(workspace: &Path, report: &Path) -> Result<JourneyOutput, String> {
    let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
    let parent = report
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("report parent must exist: {e}"))?;
    if parent.starts_with(&workspace) {
        return Err("journey report and session must be outside the disposable workspace".into());
    }
    let name = report.file_name().ok_or("report filename is required")?;
    let report = parent.join(name);
    match fs::symlink_metadata(&report) {
        Ok(_) => return Err("journey report already exists; refusing to overwrite".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let mut directory_name = name.to_os_string();
    directory_name.push(".session");
    let directory = parent.join(directory_name);
    fs::create_dir(&directory)
        .map_err(|e| format!("cannot reserve fresh session directory: {e}"))?;
    let report_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report)
        .map_err(|e| format!("cannot reserve new journey report: {e}"))?;
    Ok(JourneyOutput {
        session_state: directory.join("session.json"),
        report_file,
    })
}

/// Use the normal public windowed CLI with an isolated, initially absent session.
pub fn product_command(product: &Path, workspace: &Path, session_state: &Path) -> Command {
    let mut command = Command::new(product);
    command
        .arg("--workspace")
        .arg(workspace)
        .arg("--session-state")
        .arg(session_state)
        .current_dir(workspace);
    command
}

#[cfg(windows)]
fn guarded_input(
    window: windows::Win32::Foundation::HWND,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), (i32, String)> {
    let foreground = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    crate::focus::guarded_batch(window, foreground, send).map_err(|error| (3, error))
}

#[cfg(windows)]
fn guarded_text(window: windows::Win32::Foundation::HWND, text: &str) -> Result<(), (i32, String)> {
    for character in text.chars() {
        guarded_input(window, || {
            crate::inject::unicode_text(&character.to_string())
        })?;
    }
    Ok(())
}

pub fn run(
    product: &Path,
    workspace: &Path,
    target: &Path,
    report: &Path,
    await_foreground: bool,
) -> i32 {
    let mut output = match reserve_journey_output(workspace, report) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let mut observations = vec![format!(
        "isolated_session_state={}; initially_absent=true; default_session_untouched=true",
        output.session_state.display()
    )];
    let result = if !product.is_file() {
        Err((3, "packaged product executable is unavailable".to_string()))
    } else {
        observe(
            product,
            workspace,
            target,
            &output.session_state,
            &mut observations,
            await_foreground,
        )
    };
    let (code, detail) = result
        .err()
        .unwrap_or((0, "bounded journey completed".to_string()));
    let status = match code {
        0 => "passed",
        1 => "conformance-failed",
        2 => "operational-error",
        _ => "blocked",
    };
    let escape = |text: &str| {
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    };
    let mut text = format!(
        "schema_version = 1\nscenario = \"SC-MANUAL-OPEN-TYPE-SAVE\"\nplanned_coverage = \"steps-1-through-6-only\"\ncomplete_scenario = false\nfull_input_conformance = false\nstatus = \"{status}\"\nexit_code = {code}\ndetail = \"{}\"\n",
        escape(&detail)
    );
    for observation in observations {
        text.push_str(&format!("observation = \"{}\"\n", escape(&observation)));
    }
    // Store observations as an array, not duplicate TOML keys.
    text = text.replace("\nobservation = ", "\n[[observations]]\ndetail = ");
    match output.report_file.write_all(text.as_bytes()) {
        Ok(()) => code,
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

#[cfg(not(windows))]
fn observe(
    _: &Path,
    _: &Path,
    _: &Path,
    _: &Path,
    _: &mut Vec<String>,
    _: bool,
) -> Result<(), (i32, String)> {
    Err((
        3,
        "Windows interactive desktop and external UIA input oracle required".to_string(),
    ))
}

#[cfg(windows)]
fn observe(
    product: &Path,
    workspace: &Path,
    target: &Path,
    session_state: &Path,
    notes: &mut Vec<String>,
    await_foreground: bool,
) -> Result<(), (i32, String)> {
    use crate::{inject, observe, session};
    use std::{
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use windows::Win32::UI::{
        Input::KeyboardAndMouse::{VK_CONTROL, VK_HOME, VK_P, VK_RETURN, VK_S, VK_SHIFT},
        WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow},
    };
    let blocked = |error: String| (3, error);
    let workspace = workspace.canonicalize().map_err(|e| (2, e.to_string()))?;
    let target = workspace
        .join(target)
        .canonicalize()
        .map_err(|e| (2, e.to_string()))?;
    if !target.starts_with(&workspace) {
        return Err((2, "target escapes disposable workspace".into()));
    }
    let relative = target.strip_prefix(&workspace).unwrap();
    let git = |args: &[&str]| -> Result<String, (i32, String)> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .output()
            .map_err(|e| (2, e.to_string()))?;
        if !output.status.success() {
            return Err((2, "external Git oracle failed".into()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    if !git(&["status", "--porcelain"])?.is_empty() {
        return Err((2, "disposable repository must start clean".into()));
    }
    git(&["ls-files", "--error-unmatch", &relative.to_string_lossy()])?;
    let before = observe::file_digest(&target).map_err(blocked)?;
    let executable = observe::file_digest(product).map_err(blocked)?;
    notes.push(format!(
        "workspace_head={} executable_sha256={} target_before_sha256={}",
        git(&["rev-parse", "HEAD"])?,
        executable.sha256,
        before.sha256
    ));
    match session::probe_input_desktop() {
        session::DesktopAttachment::Attached { .. } => {}
        _ => {
            return Err((
                3,
                "interactive input desktop unavailable; no product launched".into(),
            ));
        }
    }
    let mut child = product_command(product, &workspace, session_state)
        .spawn()
        .map_err(|e| (3, e.to_string()))?;
    let result = (|| {
        let window = match observe::wait_for_window(child.id(), Duration::from_secs(90), || {
            child
                .try_wait()
                .ok()
                .flatten()
                .map(|s| s.code().unwrap_or(1))
        }) {
            observe::WindowWait::Found(window) => window,
            _ => return Err((3, "packaged process did not expose a native window".into())),
        };
        notes.push("native_window_created=true; no --file shortcut used".into());
        // Match the existing conformance driver's bounded foreground handshake.
        let mut foregrounded = false;
        if await_foreground {
            use crate::focus::{
                AwaitForegroundOutcome, ForegroundObservation, await_user_foreground,
            };
            let started = std::time::Instant::now();
            eprintln!(
                "Awaiting exact packaged Legion window {window:?}, pid {}, to become foreground for at most 60 seconds; no input sent while waiting",
                child.id()
            );
            notes.push(
                "foreground_mode=external-foreground-wait; timeout_seconds=60; driver_automatic_activation=false"
                    .into(),
            );
            let outcome = await_user_foreground(
                || {
                    if !matches!(child.try_wait(), Ok(None))
                        || !unsafe { IsWindow(Some(window)).as_bool() }
                    {
                        ForegroundObservation::WindowExited
                    } else if unsafe { GetForegroundWindow() } == window {
                        ForegroundObservation::Target
                    } else {
                        ForegroundObservation::Other
                    }
                },
                || started.elapsed(),
                || thread::sleep(Duration::from_millis(100)),
            );
            match outcome {
                AwaitForegroundOutcome::Ready => foregrounded = true,
                AwaitForegroundOutcome::WindowExited => return Err((3, "product window/process became unavailable during external foreground wait; no input injected".into())),
                AwaitForegroundOutcome::TimedOut => return Err((3, "exact product window did not become foreground within 60 seconds; no input injected".into())),
            }
        } else {
            for _ in 0..10 {
                unsafe {
                    let _ = SetForegroundWindow(window);
                    if GetForegroundWindow() == window {
                        foregrounded = true;
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        if !foregrounded {
            return Err((
                3,
                "product cannot be foregrounded; no input injected".into(),
            ));
        }
        notes.push(
            "exact_product_foreground_observed=true; per-input foreground guards remain required"
                .into(),
        );
        thread::sleep(Duration::from_millis(500));
        let oracle = observe::UiaOracle::open().map_err(blocked)?;
        let root = oracle.element_from_window(window).map_err(blocked)?;
        notes.push("uia_bootstrap_and_product_root_observed=true".into());
        let filename = target.file_name().unwrap().to_string_lossy();
        let navigation_started = std::time::Instant::now();
        let file = observe::navigate_explorer_target(
            &filename,
            || oracle.explorer_navigation_elements(&root, &filename),
            |drawer| {
                let (x, y) = oracle.clickable_center(window, &drawer.element, drawer.scope.as_ref())?;
                guarded_input(window, || inject::click_at(x, y)).map_err(|(_, error)| error)?;
                notes.push("Explorer drawer clicked once through guarded atomic OS pointer; waiting up to 3 seconds for exact visible target".into());
                Ok(())
            },
            || navigation_started.elapsed(),
            || thread::sleep(Duration::from_millis(100)),
        ).map_err(blocked)?;
        let file_point = oracle
            .clickable_center_with_observation(window, &file.element, file.scope.as_ref())
            .map_err(blocked)?;
        if file_point.explorer_scope_geometry_unavailable {
            notes.push("Explorer file scope geometry unavailable (verified named Dialog reports all-zero bounds); exact subtree, positive target/client bounds and exact point hit verified".into());
        }
        let (x, y) = file_point.center;
        guarded_input(window, || inject::click_at(x, y))?;
        thread::sleep(Duration::from_millis(900));
        notes.push("Explorer file selected through OS pointer".into());
        let close_started = std::time::Instant::now();
        observe::close_explorer_drawer(
            || oracle.explorer_navigation_elements(&root, &filename),
            |close| {
                let close_point =
                    oracle.clickable_center_with_observation(window, &close.element, close.scope.as_ref())?;
                if close_point.explorer_scope_geometry_unavailable {
                    notes.push("Explorer close scope geometry unavailable (verified named Dialog reports all-zero bounds); exact subtree, positive target/client bounds and exact point hit verified".into());
                }
                let (x, y) = close_point.center;
                guarded_input(window, || inject::click_at(x, y)).map_err(|(_, error)| error)?;
                notes.push(
                    "scoped Close Explorer drawer clicked through guarded atomic OS pointer".into(),
                );
                Ok(())
            },
            || close_started.elapsed(),
            || thread::sleep(Duration::from_millis(100)),
        )
        .map_err(blocked)?;
        notes.push("Explorer drawer absent before editor pointer/focus/text".into());
        if oracle
            .selected_tab_state(&root, &filename)
            .map_err(blocked)?
            != observe::TabState::Clean
        {
            return Err((
                3,
                "clean target tab title is not observable through UIA".into(),
            ));
        }
        let baseline = std::str::from_utf8(&before.bytes)
            .map_err(|_| (3, "selected journey target is not UTF-8".to_string()))?;
        if baseline.is_empty() {
            return Err((
                3,
                "empty document cannot uniquely identify target editor; no text injected".into(),
            ));
        }
        let (editor, pattern) = oracle
            .exact_document_element(&root, baseline)
            .map_err(blocked)?;
        let (x, y) = oracle
            .clickable_center(window, &editor, None)
            .map_err(blocked)?;
        guarded_input(window, || inject::click_at(x, y))?;
        let focus_started = std::time::Instant::now();
        observe::wait_for_editor_focus(
            || oracle.element_is_focused(&editor),
            || focus_started.elapsed(),
            || thread::sleep(Duration::from_millis(50)),
        )
        .map_err(blocked)?;
        notes.push("exact document UIA element owns keyboard focus before text input".into());
        guarded_input(window, || inject::chord(&[VK_CONTROL], VK_HOME))?;
        let marker = format!(
            "LEGION-NATIVE-RUN-{} ",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        guarded_text(window, &marker)?;
        thread::sleep(Duration::from_millis(500));
        let expected_document = format!("{marker}{baseline}");
        if !observe::document_text_matches(
            &expected_document,
            &oracle.document_text(&pattern).map_err(blocked)?,
        ) {
            return Err((
                3,
                "exact complete marker-plus-baseline UIA text unavailable; save not attempted"
                    .into(),
            ));
        }
        let dirty = oracle
            .selected_tab_state(&root, &filename)
            .map_err(blocked)?
            == observe::TabState::Dirty;
        notes.push(format!("marker_visible=true dirty_tab_observed={dirty}"));
        if !dirty {
            return Err((
                3,
                "dirty target tab affordance is not observable through UIA".into(),
            ));
        }
        guarded_input(window, || inject::chord(&[VK_CONTROL], VK_S))?;
        thread::sleep(Duration::from_millis(900));
        let mut after = observe::file_digest(&target).map_err(blocked)?;
        let mut expected = marker.as_bytes().to_vec();
        expected.extend_from_slice(&before.bytes);
        let primary_exact = after.bytes == expected;
        let primary_clean = oracle
            .selected_tab_state(&root, &filename)
            .map_err(blocked)?
            == observe::TabState::Clean;
        notes.push(format!(
            "ctrl_s_exact_disk={primary_exact} ctrl_s_dirty_cleared={primary_clean}"
        ));
        if !primary_exact || !primary_clean {
            notes.push("save shortcut did not establish clean exact disk result; trying palette Save Active Buffer".into());
            guarded_input(window, || inject::chord(&[VK_CONTROL, VK_SHIFT], VK_P))?;
            thread::sleep(Duration::from_millis(400));
            guarded_text(window, "Save Active Buffer")?;
            thread::sleep(Duration::from_millis(400));
            guarded_input(window, || inject::key_press(VK_RETURN))?;
            thread::sleep(Duration::from_millis(900));
            after = observe::file_digest(&target).map_err(blocked)?;
            notes.push("save_path=command-palette; primary_shortcut_not_passed".into());
        } else {
            notes.push("save_path=ctrl-s; fallback_used=false".into());
        }
        if after.bytes != expected {
            return Err((
                1,
                "external disk bytes differ from exact marker plus original bytes".into(),
            ));
        }
        if oracle
            .selected_tab_state(&root, &filename)
            .map_err(blocked)?
            != observe::TabState::Clean
        {
            return Err((3, "saved clean tab is not observable".into()));
        }
        let changed = git(&["diff", "--name-only"])?;
        let path = relative.to_string_lossy().replace('\\', "/");
        if changed != path {
            return Err((1, format!("unexpected tracked changes: {changed}")));
        }
        let status = git(&["status", "--porcelain"])?;
        if status != format!("M {path}") {
            return Err((1, format!("unexpected tracked Git status: {status}")));
        }
        notes.push(format!("exact_disk_bytes=true target_after_sha256={} git_diff_only={path}; no_unrelated_git_status=true", after.sha256));
        notes.push(
            "steps 7–8, all recovery cases and clipboard/IME conformance not performed".into(),
        );
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}
