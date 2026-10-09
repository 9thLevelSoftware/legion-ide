//! Bounded SC-MANUAL-OPEN-TYPE-SAVE steps 1–6, using the existing external
//! driver only. Never a six-class conformance or complete scenario result.
use std::{fs, path::Path};

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
    let mut observations = Vec::new();
    let result = if !product.is_file() {
        Err((3, "packaged product executable is unavailable".to_string()))
    } else {
        observe(
            product,
            workspace,
            target,
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
    if let Some(parent) = report.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("{error}");
            return 2;
        }
    }
    match fs::write(report, text) {
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
    notes: &mut Vec<String>,
    await_foreground: bool,
) -> Result<(), (i32, String)> {
    use crate::{inject, observe, session};
    use std::{
        process::Command,
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
    let mut child = Command::new(product)
        .arg("--workspace")
        .arg(&workspace)
        .current_dir(&workspace)
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
                "Awaiting user foreground of packaged Legion window {window:?}, pid {}, for at most 60 seconds; no input sent while waiting",
                child.id()
            );
            notes.push(
                "foreground_mode=user-attended; timeout_seconds=60; automatic_activation=false"
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
                AwaitForegroundOutcome::WindowExited => return Err((3, "product window/process became unavailable during attended foreground wait; no input injected".into())),
                AwaitForegroundOutcome::TimedOut => return Err((3, "user did not foreground the exact product window within 60 seconds; no input injected".into())),
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
                let (x, y) = oracle.clickable_center(&drawer)?;
                guarded_input(window, || inject::click_at(x, y)).map_err(|(_, error)| error)?;
                notes.push("Explorer drawer clicked once through guarded atomic OS pointer; waiting up to 3 seconds for exact visible target".into());
                Ok(())
            },
            || navigation_started.elapsed(),
            || thread::sleep(Duration::from_millis(100)),
        ).map_err(blocked)?;
        let (x, y) = oracle.clickable_center(&file).map_err(blocked)?;
        guarded_input(window, || inject::click_at(x, y))?;
        thread::sleep(Duration::from_millis(900));
        notes.push("Explorer file selected through OS pointer".into());
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
        let rect = oracle.bounding_rectangle(&editor).map_err(blocked)?;
        guarded_input(window, || {
            inject::click_at((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
        })?;
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
