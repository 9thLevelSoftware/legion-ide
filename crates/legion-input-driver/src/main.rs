//! `legion-input-driver`: the external native input driver ADR-0056 specifies.
//!
//! `COMP-PLAT-002` asks whether the **packaged** product's native input path
//! carries keyboard, pointer, text, clipboard, IME/CJK and command input
//! through the authoritative UI/app route. This binary is the instrument that
//! could answer it. It is not the answer.
//!
//! # What this binary is allowed to know about the product
//!
//! Nothing, beyond a path to an executable. It launches the product as a
//! subprocess, injects at the OS level with `SendInput` and the Win32
//! clipboard API, and reads every observation back from **outside** that
//! process, through UI Automation and through the bytes the product wrote to
//! disk. There is no product-side test hook, feature flag, environment
//! variable or IPC channel, and adding one would defeat the point: a harness
//! that can see inside the product cannot tell a working native input path from
//! a harness talking to itself.
//!
//! # Outcomes
//!
//! Exit codes are [`cli::EXIT_PASSED`], [`cli::EXIT_CONFORMANCE_FAILED`],
//! [`cli::EXIT_OPERATIONAL_ERROR`] and [`cli::EXIT_BLOCKED`], matching the
//! harness. Blocked is nonzero, always, and a class the driver did not observe
//! is never written as conforming.
//!
//! # macOS and Linux
//!
//! Not enabled by ADR-0056. Both subcommands write a blocked report naming
//! `BLK-2026-09-08-02` and exit nonzero there; a Windows result never
//! substitutes for a macOS or Linux row.

mod cli;
#[cfg(windows)]
mod inject;
// `observe`, `report` and `session` hold the host-independent vocabulary that
// `tests/driver_contract.rs` includes by path and asserts against. Which half
// of it the *binary* reaches depends on the host it is built for — the
// unsupported-host reports are reachable only off Windows, the oracles only on
// it — so dead-code analysis of one build alone would flag the other host's
// half of a module both builds need to keep.
#[allow(dead_code)]
mod observe;
#[allow(dead_code)]
mod report;
#[allow(dead_code)]
mod session;

use std::{fs, path::Path};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    match cli::parse(args) {
        Err(error) => {
            eprintln!("legion-input-driver: {}", error.message);
            eprintln!("{}", cli::USAGE);
            cli::EXIT_OPERATIONAL_ERROR
        }
        Ok(cli::Command::ProbeSession { report }) => probe_session(&report),
        Ok(cli::Command::ConformanceRun { product, report }) => conformance_run(&product, &report),
    }
}

/// `--probe-session`: attach to the input desktop, and say so only if it
/// worked. The report is written in both cases; a blocked run that leaves no
/// artifact is unauditable.
fn probe_session(report_path: &Path) -> i32 {
    let attachment = session::probe_input_desktop();
    let code = report::session_exit_code(&attachment);
    let text = report::render_session_report(&attachment);
    match write_report(report_path, &text) {
        Ok(()) => code,
        Err(message) => {
            eprintln!("legion-input-driver: {message}");
            cli::EXIT_OPERATIONAL_ERROR
        }
    }
}

/// `--conformance-run`: drive the packaged product and record what was
/// observed from outside it.
fn conformance_run(product: &Path, report_path: &Path) -> i32 {
    let scratch_root = report_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let observed = observe_conformance(product, &scratch_root);
    let code = report::conformance_exit_code(&observed);
    let text = report::render_conformance_report(&observed);
    match write_report(report_path, &text) {
        Ok(()) => code,
        Err(message) => {
            eprintln!("legion-input-driver: {message}");
            cli::EXIT_OPERATIONAL_ERROR
        }
    }
}

fn write_report(report_path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(err) = fs::create_dir_all(parent)
    {
        return Err(format!(
            "cannot create report directory {}: {err}",
            parent.display()
        ));
    }
    fs::write(report_path, text)
        .map_err(|err| format!("cannot write report {}: {err}", report_path.display()))
}

/// macOS and Linux stay blocked on `BLK-2026-09-08-02`. This path never
/// reaches exit 0.
#[cfg(not(windows))]
fn observe_conformance(product: &Path, _scratch_root: &Path) -> report::ConformanceReport {
    report::unsupported_host_conformance_report(&product.display().to_string())
}

#[cfg(windows)]
fn observe_conformance(product: &Path, scratch_root: &Path) -> report::ConformanceReport {
    use std::{process::Command, time::Duration};

    use report::ConformanceReport;

    let product_label = product.display().to_string();
    if !product.is_file() {
        return report::missing_product_conformance_report(&product_label);
    }

    let mut observed = ConformanceReport {
        product: product_label.clone(),
        ..ConformanceReport::default()
    };

    // The driver must be on the input desktop before it injects anything.
    match session::probe_input_desktop() {
        session::DesktopAttachment::Attached { .. } => {}
        session::DesktopAttachment::NotAttached { detail }
        | session::DesktopAttachment::UnsupportedHost { detail } => {
            observed.prerequisite = Some(report::PREREQUISITE_NO_INPUT_DESKTOP.to_string());
            observed.notes.push(format!(
                "the driver could not attach to an input desktop, so nothing was launched and \
                 no class was observed: {detail}"
            ));
            return observed;
        }
    }

    let workspace = scratch_root.join("native-input-driver-workspace");
    let target = workspace.join("native_input_probe.txt");
    if let Err(err) = fs::create_dir_all(&workspace) {
        observed.prerequisite = Some(format!(
            "A writable scratch directory at `{}` for the driver's probe workspace.",
            workspace.display()
        ));
        observed
            .notes
            .push(format!("cannot create the driver scratch workspace: {err}"));
        return observed;
    }
    if let Err(err) = fs::write(&target, b"seed\n") {
        observed.prerequisite = Some(format!(
            "A writable scratch file at `{}` for the driver's probe document.",
            target.display()
        ));
        observed
            .notes
            .push(format!("cannot seed the driver probe document: {err}"));
        return observed;
    }

    let mut child = match Command::new(product)
        .arg("--workspace")
        .arg(&workspace)
        .arg("--file")
        .arg(&target)
        .current_dir(&workspace)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            observed.prerequisite = Some(report::PREREQUISITE_NO_PRODUCT.to_string());
            observed
                .notes
                .push(format!("cannot launch the packaged product: {err}"));
            return observed;
        }
    };

    let window = observe::wait_for_window(child.id(), Duration::from_secs(90), || {
        child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.code().unwrap_or(1))
    });
    let window = match window {
        observe::WindowWait::Found(window) => window,
        observe::WindowWait::ProcessExited { exit_code } => {
            observed.prerequisite = Some(format!(
                "The packaged native Legion product process stayed alive long enough to open a \
                 visible top-level window. It exited with code {exit_code} before any such \
                 window appeared, so the out-of-process oracles had nothing to read and the \
                 90-second host window-wait was not reached."
            ));
            observed.notes.push(format!(
                "product process exited with code {exit_code} before a visible top-level window \
                 appeared"
            ));
            return observed;
        }
        observe::WindowWait::TimedOut => {
            let _ = child.kill();
            let _ = child.wait();
            observed.prerequisite = Some(
                "A packaged native Legion product that opens a visible top-level window within 90 \
                 seconds of launch on this host, so the out-of-process oracles have a window to \
                 read. Without one the driver observed nothing and the product is not implicated."
                    .to_string(),
            );
            observed.notes.push(
                "no visible top-level window owned by the product process appeared".to_string(),
            );
            return observed;
        }
    };
    observed.window_created = true;

    let outcome = drive_classes(window, &target);
    let _ = child.kill();
    let _ = child.wait();

    match outcome {
        Ok(observations) => observed.observations = observations,
        Err(message) => {
            observed.prerequisite = Some(format!(
                "A Windows 11 x64 host on which the driver can establish its out-of-process \
                 oracles against the packaged product window. The driver reached the window but \
                 could not read it: {message}"
            ));
            observed.notes.push(message);
        }
    }

    // A blocked run must state why. The expected shape on a host with no CJK
    // input layout is *not* the empty-observation one: five classes observed,
    // `ime-cjk` blocked, no `Err` from the oracles — which left `prerequisite`
    // unset and wrote `prerequisite = ""` onto a blocked report. The reason
    // lives in the blocked classes' own details, so compose it from them and
    // name the classes. This only ever fills a gap: a prerequisite already
    // stated above wins, and a run that is not blocked gets none.
    if observed.prerequisite.is_none() {
        let composed = report::effective_prerequisite(&observed);
        observed.prerequisite = composed;
    }
    observed
}

/// Run the six classes against a live product window.
///
/// Returns `Err` only when the oracles themselves could not be established;
/// every per-class result, including a blocked one, comes back inside `Ok`.
#[cfg(windows)]
fn drive_classes(
    window: windows::Win32::Foundation::HWND,
    target: &Path,
) -> Result<Vec<report::ClassObservation>, String> {
    use std::{thread, time::Duration};

    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    let oracle = observe::UiaOracle::open()?;
    let root = oracle.element_from_window(window)?;
    let (editor, text_pattern) = oracle.text_element(&root)?;

    // SAFETY: `window` was produced by the desktop enumeration.
    let mut foregrounded = false;
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
    if !foregrounded {
        return Err(
            "Windows refused to foreground the product window; injected input would not be \
             guaranteed to reach it"
                .to_string(),
        );
    }
    thread::sleep(Duration::from_millis(500));

    // Focus the editor once, through the OS pointer, at a coordinate derived
    // from the UI Automation bounding rectangle. No product geometry API is
    // consulted here or anywhere else in this file.
    let rect = oracle.bounding_rectangle(&editor)?;
    let centre_x = rect.left + (rect.right - rect.left) / 2;
    let centre_y = rect.top + (rect.bottom - rect.top) / 2;
    inject::click_at(centre_x, centre_y)?;
    thread::sleep(Duration::from_millis(300));

    let context = ClassContext {
        oracle: &oracle,
        root: &root,
        text_pattern: &text_pattern,
        window,
        target,
        centre: (centre_x, centre_y),
    };

    Ok(vec![
        observe_keyboard(&context),
        observe_pointer(&context),
        observe_text(&context),
        observe_clipboard(&context),
        observe_ime_cjk(&context),
        observe_command(&context),
    ])
}

#[cfg(windows)]
struct ClassContext<'a> {
    oracle: &'a observe::UiaOracle,
    root: &'a windows::Win32::UI::Accessibility::IUIAutomationElement,
    text_pattern: &'a windows::Win32::UI::Accessibility::IUIAutomationTextPattern,
    window: windows::Win32::Foundation::HWND,
    target: &'a Path,
    centre: (i32, i32),
}

#[cfg(windows)]
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(400));
}

/// Press the authoritative save chord and let the product finish writing.
#[cfg(windows)]
fn save_chord() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_S};

    inject::chord(&[VK_CONTROL], VK_S)?;
    std::thread::sleep(std::time::Duration::from_millis(900));
    Ok(())
}

/// **Keyboard.** Re-observes the exact behaviour `DEF-2026-09-05-01` is about:
/// whether `Home` moves the caret to the start of the line. `AB`, then `Home`,
/// then `Q` must leave `QAB` in the document; `ABQ` means the caret never
/// moved.
#[cfg(windows)]
fn observe_keyboard(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_END, VK_HOME, VK_RETURN};

    let injected = (|| -> Result<String, String> {
        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::unicode_text("AB")?;
        inject::extended_key_press(VK_HOME)?;
        inject::unicode_text("Q")?;
        settle();
        context.oracle.document_text(context.text_pattern)
    })();

    match injected {
        Err(message) => ClassObservation::new("keyboard", ClassOutcome::Blocked, message),
        Ok(text) if text.contains("QAB") => ClassObservation::new(
            "keyboard",
            ClassOutcome::Conforms,
            "injected AB, Home, Q; the UI Automation TextPattern reported QAB, so Home moved \
             the caret to the start of the line",
        ),
        Ok(text) if text.contains("ABQ") => ClassObservation::new(
            "keyboard",
            ClassOutcome::Deviates,
            "injected AB, Home, Q; the UI Automation TextPattern reported ABQ, so the injected \
             Home did not move the caret (DEF-2026-09-05-01)",
        ),
        Ok(_) => ClassObservation::new(
            "keyboard",
            ClassOutcome::Blocked,
            "the UI Automation TextPattern reported neither QAB nor ABQ after the injected \
             keys, so the caret oracle could not be read",
        ),
    }
}

/// **Pointer.** Clicks at a coordinate derived from the UI Automation bounding
/// rectangle and reads focus back through UI Automation.
#[cfg(windows)]
fn observe_pointer(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};

    let observation = (|| -> Result<(bool, bool), String> {
        inject::click_at(context.centre.0, context.centre.1)?;
        settle();
        let focused = context.oracle.focused_element()?;
        let focused_rect = context.oracle.bounding_rectangle(&focused)?;
        let inside = context.centre.0 >= focused_rect.left
            && context.centre.0 <= focused_rect.right
            && context.centre.1 >= focused_rect.top
            && context.centre.1 <= focused_rect.bottom;
        let caret_readable = context.oracle.selection_text(context.text_pattern).is_ok();
        Ok((inside, caret_readable))
    })();

    match observation {
        Err(message) => ClassObservation::new("pointer", ClassOutcome::Blocked, message),
        Ok((true, true)) => ClassObservation::new(
            "pointer",
            ClassOutcome::Conforms,
            "clicked the UI Automation bounding-rectangle centre; the focused element's \
             rectangle contains the clicked point and the caret was readable through \
             TextPattern",
        ),
        Ok((false, _)) => ClassObservation::new(
            "pointer",
            ClassOutcome::Deviates,
            "clicked the UI Automation bounding-rectangle centre, but the element that took \
             focus does not contain the clicked point",
        ),
        Ok((true, false)) => ClassObservation::new(
            "pointer",
            ClassOutcome::Blocked,
            "focus moved to the clicked element, but the caret could not be read back through \
             the UI Automation TextPattern",
        ),
    }
}

/// **Text.** Injects non-ASCII and a multi-code-point grapheme, then compares
/// the UI Automation text against the bytes on disk after an authoritative
/// save. The two oracles disagreeing is the finding, not a retry.
#[cfg(windows)]
fn observe_text(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_END, VK_RETURN};

    // `caf\u{e9}` is non-ASCII; the regional-indicator pair is one grapheme
    // made of two code points, which is the case a UTF-16 round trip breaks.
    const MARKER: &str = "LEGION-TEXT-caf\u{e9}\u{1f1ef}\u{1f1f5}";

    let observation = (|| -> Result<(bool, bool, String), String> {
        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::unicode_text(MARKER)?;
        settle();
        let in_ui = context
            .oracle
            .document_text(context.text_pattern)?
            .contains(MARKER);
        save_chord()?;
        let digest = observe::file_digest(context.target)?;
        let on_disk = window_contains(&digest.bytes, MARKER.as_bytes());
        Ok((in_ui, on_disk, digest.sha256))
    })();

    match observation {
        Err(message) => ClassObservation::new("text", ClassOutcome::Blocked, message),
        Ok((true, true, sha256)) => ClassObservation::new(
            "text",
            ClassOutcome::Conforms,
            format!(
                "the injected non-ASCII multi-code-point marker is present in the UI Automation \
                 text and in the saved UTF-8 bytes; file SHA-256 {sha256}"
            ),
        ),
        Ok((in_ui, on_disk, sha256)) => ClassObservation::new(
            "text",
            ClassOutcome::Deviates,
            format!(
                "the injected marker was in the UI Automation text: {in_ui}; in the saved bytes: \
                 {on_disk}; file SHA-256 {sha256}"
            ),
        ),
    }
}

/// **Clipboard.** The driver owns both ends: it sets the system clipboard
/// before the paste and reads the system clipboard after the copy. The
/// product's clipboard abstraction is never consulted.
#[cfg(windows)]
fn observe_clipboard(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_C, VK_CONTROL, VK_END, VK_RETURN, VK_V,
    };

    const MARKER: &str = "LEGION-CLIP-\u{e9}\u{4e2d}";

    let observation = (|| -> Result<(bool, bool), String> {
        inject::set_clipboard_text(MARKER)?;
        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::chord(&[VK_CONTROL], VK_V)?;
        save_chord()?;
        let pasted = window_contains(
            &observe::file_digest(context.target)?.bytes,
            MARKER.as_bytes(),
        );

        // Wipe the clipboard first, so a copy that does nothing cannot be read
        // as a success from the value the driver put there a moment ago.
        inject::set_clipboard_text("LEGION-CLIP-CLEARED")?;
        inject::chord(&[VK_CONTROL], VK_A)?;
        inject::chord(&[VK_CONTROL], VK_C)?;
        settle();
        let copied = inject::read_clipboard_text()?.contains(MARKER);
        inject::chord(&[VK_CONTROL], VK_END)?;
        Ok((pasted, copied))
    })();

    match observation {
        Err(message) => ClassObservation::new("clipboard", ClassOutcome::Blocked, message),
        Ok((true, true)) => ClassObservation::new(
            "clipboard",
            ClassOutcome::Conforms,
            "the OS clipboard value the driver set reached the saved bytes, and the driver read \
             its marker back off the OS clipboard after the copy gesture",
        ),
        Ok((pasted, copied)) => ClassObservation::new(
            "clipboard",
            ClassOutcome::Deviates,
            format!(
                "paste reached the saved bytes: {pasted}; the OS clipboard carried the marker \
                 after the copy gesture: {copied}"
            ),
        ),
    }
}

/// **IME/CJK.** Requires a real installed IME driven by key injection.
///
/// With no CJK input layout active for the product window there is nothing to
/// drive, and this class is `blocked` with an exact prerequisite. It is never
/// `conforms` on such a host, and never silently omitted.
#[cfg(windows)]
fn observe_ime_cjk(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, VK_CONTROL, VK_CONVERT, VK_END, VK_RETURN,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    const IME_PREREQUISITE: &str = concat!(
        "A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for ",
        "Japanese) and active as the input layout of the packaged product's window, so the ",
        "driver can drive a real composition by key injection rather than synthesizing a ",
        "commit."
    );

    // SAFETY: `window` is a live handle produced by the desktop enumeration.
    let layout = unsafe {
        let thread = GetWindowThreadProcessId(context.window, None);
        GetKeyboardLayout(thread)
    };
    let language_id = (layout.0 as usize & 0xffff) as u16;
    let primary_language = language_id & 0x3ff;
    // Japanese, Chinese and Korean primary language identifiers.
    let cjk = matches!(primary_language, 0x11 | 0x04 | 0x12);
    if !cjk {
        return ClassObservation::new(
            "ime-cjk",
            ClassOutcome::Blocked,
            format!(
                "{IME_PREREQUISITE} The product window's input layout reports language id \
                 0x{language_id:04x}, which is not a CJK IME."
            ),
        );
    }

    let observation = (|| -> Result<(bool, bool), String> {
        let baseline_ui = context.oracle.document_text(context.text_pattern)?;
        let baseline_file =
            String::from_utf8_lossy(&observe::file_digest(context.target)?.bytes).into_owned();
        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::unicode_text("nihongo")?;
        inject::key_press(VK_CONVERT)?;
        settle();
        // First oracle: a composition region visible through UI Automation
        // before anything is committed, compared against the pre-IME snapshot
        // so earlier text/clipboard CJK markers cannot count as this class.
        let composing = observe::has_new_cjk(
            &baseline_ui,
            &context.oracle.document_text(context.text_pattern)?,
        );
        inject::key_press(VK_RETURN)?;
        save_chord()?;
        // Second oracle: newly committed CJK in the saved bytes versus the
        // same pre-IME snapshot of the file.
        let committed = observe::has_new_cjk(
            &baseline_file,
            &String::from_utf8_lossy(&observe::file_digest(context.target)?.bytes),
        );
        Ok((composing, committed))
    })();

    match observation {
        Err(message) => ClassObservation::new("ime-cjk", ClassOutcome::Blocked, message),
        Ok((true, true)) => ClassObservation::new(
            "ime-cjk",
            ClassOutcome::Conforms,
            "a real IME composition was visible through the UI Automation TextPattern and the \
             committed code points reached the saved bytes",
        ),
        Ok((composing, committed)) => ClassObservation::new(
            "ime-cjk",
            ClassOutcome::Deviates,
            format!(
                "composition visible through UI Automation: {composing}; committed code points \
                 in the saved bytes: {committed}"
            ),
        ),
    }
}

/// **Command.** The `DEF-2026-09-05-02` contrast pair: the `Ctrl+S` chord and
/// the command palette route must agree, and both must move the bytes.
///
/// A route that moves the UI Automation state but not the bytes, or the
/// reverse, is a conformance failure — not a retry and not a blocked result.
#[cfg(windows)]
fn observe_command(context: &ClassContext<'_>) -> report::ClassObservation {
    use report::{ClassObservation, ClassOutcome};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_CONTROL, VK_END, VK_P, VK_RETURN, VK_SHIFT,
    };

    const CHORD_MARKER: &str = "LEGION-CMD-CHORD";
    const PALETTE_MARKER: &str = "LEGION-CMD-PALETTE";

    let observation = (|| -> Result<(bool, bool, bool), String> {
        let before = observe::file_digest(context.target)?;
        let clean_names = context.oracle.name_snapshot(context.root)?;

        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::unicode_text(CHORD_MARKER)?;
        settle();
        let dirty_names = context.oracle.name_snapshot(context.root)?;

        save_chord()?;
        let after_chord = observe::file_digest(context.target)?;
        let saved_names = context.oracle.name_snapshot(context.root)?;
        let chord_saved = after_chord.sha256 != before.sha256
            && window_contains(&after_chord.bytes, CHORD_MARKER.as_bytes());
        // The dirty affordance has to move when the buffer is edited and move
        // back when it is saved. If the accessible names never change, the
        // affordance is not observable from outside the process.
        let dirty_observable = dirty_names != clean_names && saved_names != dirty_names;

        inject::chord(&[VK_CONTROL], VK_END)?;
        inject::key_press(VK_RETURN)?;
        inject::unicode_text(PALETTE_MARKER)?;
        settle();
        inject::chord(&[VK_CONTROL, VK_SHIFT], VK_P)?;
        settle();
        inject::unicode_text("save")?;
        settle();
        inject::key_press(VK_RETURN)?;
        std::thread::sleep(std::time::Duration::from_millis(900));
        let after_palette = observe::file_digest(context.target)?;
        let palette_saved = after_palette.sha256 != after_chord.sha256
            && window_contains(&after_palette.bytes, PALETTE_MARKER.as_bytes());

        Ok((chord_saved, palette_saved, dirty_observable))
    })();

    match observation {
        Err(message) => ClassObservation::new("command", ClassOutcome::Blocked, message),
        Ok((chord_saved, palette_saved, _)) if chord_saved != palette_saved => {
            ClassObservation::new(
                "command",
                ClassOutcome::Deviates,
                format!(
                    "the injected Ctrl+S chord moved the file bytes: {chord_saved}; the command \
                     palette route moved them: {palette_saved}. The two routes disagree \
                     (DEF-2026-09-05-02)"
                ),
            )
        }
        Ok((false, false, _)) => ClassObservation::new(
            "command",
            ClassOutcome::Deviates,
            "neither the injected Ctrl+S chord nor the command palette route changed the file \
             bytes on disk",
        ),
        Ok((_, _, false)) => ClassObservation::new(
            "command",
            ClassOutcome::Blocked,
            "both save routes moved the file bytes, but the product publishes no UI Automation \
             name that changes between edited and saved, so the dirty affordance could not be \
             read from outside the process and the contrast pair is only half observable",
        ),
        Ok((_, _, true)) => ClassObservation::new(
            "command",
            ClassOutcome::Conforms,
            "the injected Ctrl+S chord and the command palette route both changed the file \
             bytes, and the UI Automation dirty affordance moved on edit and returned on save",
        ),
    }
}

/// Substring search over raw bytes, so the disk oracle never has to assume the
/// file is valid UTF-8 before it can look for a marker.
#[cfg(windows)]
fn window_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|slice| slice == needle)
}
