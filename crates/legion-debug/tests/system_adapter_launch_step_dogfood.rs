//! B13: optional system adapter launch + step against a tiny debugee.
//!
//! Default CI: soft-skip when no system adapter is present **or** when spawn /
//! handshake / launch / step fails (broken host LLDB installs are common).
//!
//! Intentional dogfood: `LEGION_DAP_DOGFOOD=1` fails closed on any step.
//!
//! Scope: build a tiny Rust binary, initialize, setBreakpoints, launch with
//! stopOnEntry, one step over, disconnect. Not a claim of full product UX.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use legion_debug::{LiveDapSession, dogfood_requires_system_adapter, resolve_system_adapter};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn soft_or_fail(require: bool, context: &str, err: impl std::fmt::Display) {
    if require {
        panic!("{context}: {err}");
    }
    eprintln!("skip: {context} ({err}); set LEGION_DAP_DOGFOOD=1 to fail closed");
}

fn build_tiny_debugee() -> Result<(PathBuf, PathBuf, String), String> {
    let root = std::env::temp_dir().join(format!(
        "legion-dap-dogfood-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "dap_dogfood_probe"
version = "0.1.0"
edition = "2021"
"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n",
    )
    .map_err(|e| e.to_string())?;

    let status = Command::new("cargo")
        .args([
            "build",
            "--package",
            "dap_dogfood_probe",
            "--bin",
            "dap_dogfood_probe",
        ])
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("cargo spawn: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build failed: {status}"));
    }

    let bin = if cfg!(windows) {
        root.join("target/debug/dap_dogfood_probe.exe")
    } else {
        root.join("target/debug/dap_dogfood_probe")
    };
    if !bin.is_file() {
        return Err(format!("missing binary {}", bin.display()));
    }
    let source = root.join("src/main.rs");
    let program = bin.to_string_lossy().replace('\\', "/");
    Ok((root, source, program))
}

#[test]
fn system_adapter_launch_step_dogfood() {
    let require = dogfood_requires_system_adapter();

    let Some(adapter) = resolve_system_adapter("lldb-dap") else {
        if require {
            panic!(
                "LEGION_DAP_DOGFOOD=1 requires a system adapter \
                 (set LEGION_DAP_ADAPTER or install lldb-dap/codelldb on PATH)"
            );
        }
        eprintln!(
            "skip system_adapter_launch_step_dogfood: no system adapter; \
             set LEGION_DAP_DOGFOOD=1 to fail closed"
        );
        return;
    };

    assert!(!adapter.is_fake);

    let (root, source, program) = match build_tiny_debugee() {
        Ok(v) => v,
        Err(err) => {
            soft_or_fail(require, "build tiny debugee", err);
            return;
        }
    };
    let source_path = source.to_string_lossy().replace('\\', "/");
    let cwd = root.to_string_lossy().replace('\\', "/");

    eprintln!(
        "system DAP launch dogfood: adapter={} type={} program={program} require={require}",
        adapter.program.display(),
        adapter.adapter_type
    );

    let mut session = match LiveDapSession::spawn(
        &adapter.program,
        &adapter.args,
        adapter.adapter_type.clone(),
    ) {
        Ok(s) => s,
        Err(err) => {
            soft_or_fail(require, "spawn system adapter", err);
            let _ = fs::remove_dir_all(&root);
            return;
        }
    };

    if let Err(err) = session.initialize_handshake(Duration::from_secs(10)) {
        let _ = session.disconnect_and_wait(Duration::from_secs(2));
        soft_or_fail(require, "initialize handshake", err);
        let _ = fs::remove_dir_all(&root);
        return;
    }

    if let Err(err) = session.set_breakpoints(&source_path, &[2], Duration::from_secs(5)) {
        let _ = session.disconnect_and_wait(Duration::from_secs(2));
        soft_or_fail(require, "setBreakpoints", err);
        let _ = fs::remove_dir_all(&root);
        return;
    }

    let stop = match session.launch_until_stopped_with(
        &program,
        Some(cwd.as_str()),
        true, // stopOnEntry for deterministic first stop
        Duration::from_secs(15),
    ) {
        Ok(stop) => stop,
        Err(err) => {
            let _ = session.disconnect_and_wait(Duration::from_secs(2));
            soft_or_fail(require, "launch until stopped", err);
            let _ = fs::remove_dir_all(&root);
            return;
        }
    };

    assert!(
        !stop.stack_frames.is_empty() || stop.thread_id > 0,
        "expected stop frames or thread: {:?}",
        stop
    );
    eprintln!(
        "system DAP launch dogfood: stopped reason={} thread={} frames={}",
        stop.reason,
        stop.thread_id,
        stop.stack_frames.len()
    );

    match session.step_over_until_stopped(stop.thread_id, Duration::from_secs(10)) {
        Ok(stepped) => {
            eprintln!(
                "system DAP launch dogfood: step reason={} frames={}",
                stepped.reason,
                stepped.stack_frames.len()
            );
        }
        Err(err) => {
            let _ = session.disconnect_and_wait(Duration::from_secs(2));
            soft_or_fail(require, "step over", err);
            let _ = fs::remove_dir_all(&root);
            return;
        }
    }

    if let Err(err) = session.disconnect_and_wait(Duration::from_secs(3)) {
        soft_or_fail(require, "disconnect", err);
    }

    let _ = fs::remove_dir_all(&root);
}
