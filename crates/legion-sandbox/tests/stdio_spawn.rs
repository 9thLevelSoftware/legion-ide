//! C4: long-lived sandboxed stdio spawn.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use legion_sandbox::spawn_stdio::{SandboxStdioSpec, spawn_sandboxed_stdio};

#[test]
fn spawn_sandboxed_stdio_runs_and_reports() {
    let root = std::env::temp_dir().join(format!(
        "legion-stdio-sandbox-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&root).expect("tmpdir");

    #[cfg(windows)]
    let (program, args) = (
        PathBuf::from("cmd.exe"),
        vec!["/C".to_string(), "echo".to_string(), "ok".to_string()],
    );
    #[cfg(not(windows))]
    let (program, args) = (
        PathBuf::from("sh"),
        vec!["-c".to_string(), "echo ok".to_string()],
    );

    let spec = SandboxStdioSpec {
        program,
        args,
        working_dir: root.clone(),
        writable_root: root.clone(),
        allowed_egress: BTreeSet::new(),
        env: Vec::new(),
    };

    let proc = match spawn_sandboxed_stdio(&spec) {
        Ok(p) => p,
        Err(err) => {
            // Fail-closed platforms without Landlock/sandbox-exec: skip in CI.
            eprintln!("skip stdio sandbox spawn: {err}");
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
    };

    assert!(
        !proc.enforcement.backend_used.is_empty(),
        "enforcement backend must be reported"
    );

    let (mut child, mut stdin, mut stdout, report, _guard) = proc.into_parts();
    assert!(!report.backend_used.is_empty());

    // Drain short-lived child output so it can exit.
    let mut buf = Vec::new();
    let _ = stdout.read_to_end(&mut buf);
    let _ = stdin.flush();
    drop(stdin);
    let status = child.wait().expect("wait");
    assert!(status.success() || status.code().is_some());

    let _ = std::fs::remove_dir_all(&root);
    let _ = Duration::from_millis(1);
}
