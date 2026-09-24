//! Stdio teardown fixture for `legion-lsp` contract tests.
//!
//! Modes:
//! - `hold-tree`: spawn a silent grandchild that inherits stdout, then sleep.
//! - `hold-child`: the silent descendant used by `hold-tree`.
//! - `flood`: emit valid JSON-RPC notifications forever so the reader mailbox fills.

use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("hold-tree") => hold_tree(),
        Some("hold-child") => sleep_forever(),
        Some("flood") => flood(),
        other => {
            eprintln!("unknown lsp_stdio_fixture mode: {other:?}");
            std::process::exit(2);
        }
    }
}

fn hold_tree() {
    let exe = std::env::current_exe().expect("current exe");
    // The grandchild must outlive this helper until the test's process-tree
    // kill. Reaping here would close the inherited stdout write end.
    #[allow(clippy::zombie_processes)]
    let _child = Command::new(exe)
        .arg("hold-child")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn hold-child");
    // Do not write to stdout: the grandchild must keep the LSP pipe open
    // without producing frames that would let the reader exit on its own.
    eprintln!("ready");
    let _ = io::stderr().flush();
    sleep_forever();
}

fn flood() {
    let payload =
        br#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"x"}}"#;
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    let mut stdout = io::stdout().lock();
    loop {
        if stdout.write_all(header.as_bytes()).is_err()
            || stdout.write_all(payload).is_err()
            || stdout.flush().is_err()
        {
            return;
        }
    }
}

fn sleep_forever() {
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
