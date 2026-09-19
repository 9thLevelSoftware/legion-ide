use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use legion_platform::{
    BoundedProcessRequest, MAX_BOUNDED_STDIN_BYTES, NativeProcessService, PlatformError,
    ProcessRequest, ProcessService,
};

/// Larger than the default anonymous-pipe buffer on every supported host
/// (roughly 4 KiB on Windows, 64 KiB on Linux, 16-64 KiB on macOS), so a child
/// that never reads is guaranteed to back-pressure the writer.
const OVER_PIPE_CAPACITY_BYTES: usize = 256 * 1024;

/// How long the `stdin_ignore` fixture stays alive without reading stdin before
/// writing its exit marker.
const IGNORE_FIXTURE_LIFETIME: Duration = Duration::from_secs(4);

/// Stderr progress interval used by the `echo_stdin` fixture.
const ECHO_PROGRESS_INTERVAL_BYTES: usize = 16 * 1024;

static MARKER_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn unique_marker(label: &str) -> PathBuf {
    let seq = MARKER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "legion-bounded-{label}-{}-{seq}-{nanos}",
        std::process::id()
    ))
}

fn exit_marker_path(marker: &Path) -> PathBuf {
    PathBuf::from(format!("{}.exit", marker.display()))
}

/// Blocks until the child's readiness marker appears and fails the test if it
/// never does. Callers anchor their timing on the instant this returns, so a
/// slow child start cannot make a later cleanup check vacuous.
fn wait_for_marker(marker: &Path, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if marker.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "child readiness marker {} never appeared within {limit:?}",
        marker.display()
    );
}

fn over_capacity_payload() -> Vec<u8> {
    vec![b'z'; OVER_PIPE_CAPACITY_BYTES]
}

/// Multi-byte UTF-8 payload larger than any host pipe buffer, so a byte-exact
/// echo also proves no chunk boundary corrupted the stream.
fn large_unicode_payload() -> String {
    let unit = "\u{03bb}\u{2192}\u{6f22}\u{5b57}\u{03c9}-0123456789";
    let mut payload = String::with_capacity(OVER_PIPE_CAPACITY_BYTES + unit.len());
    while payload.len() < OVER_PIPE_CAPACITY_BYTES {
        payload.push_str(unit);
    }
    payload
}

fn bounded_with(
    command: String,
    args: Vec<String>,
    cancellation: Arc<AtomicBool>,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> BoundedProcessRequest {
    BoundedProcessRequest::new(
        ProcessRequest {
            command,
            args,
            cwd: None,
            env: Vec::new(),
            stdin: None,
            timeout: None,
            cancelled: false,
        },
        stdout_limit,
        stderr_limit,
        timeout,
        cancellation,
    )
}

fn bounded(
    command: String,
    args: Vec<String>,
    cancellation: Arc<AtomicBool>,
) -> BoundedProcessRequest {
    bounded_with(command, args, cancellation, 64, 64, Duration::from_secs(2))
}

fn fixture_request(mode: &str) -> BoundedProcessRequest {
    let mut request = ProcessRequest::new(
        std::env::current_exe()
            .expect("test executable")
            .to_string_lossy()
            .to_string(),
    );
    request.args = vec![
        "--exact".to_string(),
        "bounded_child_fixture".to_string(),
        "--nocapture".to_string(),
    ];
    request.env = vec![("LEGION_BOUNDED_FIXTURE".to_string(), mode.to_string())];
    BoundedProcessRequest::new(
        request,
        64,
        64,
        Duration::from_millis(500),
        Arc::new(AtomicBool::new(false)),
    )
}

fn fixture_request_with_marker(mode: &str, marker: &Path) -> BoundedProcessRequest {
    let mut request = fixture_request(mode);
    request.process.env.push((
        "LEGION_BOUNDED_READY_MARKER".to_string(),
        marker.to_string_lossy().into_owned(),
    ));
    request.max_stdout_bytes = 4096;
    request.max_stderr_bytes = 4096;
    request
}

#[test]
// This fixture intentionally leaves the pipe-holding descendant alive so the
// outer bounded runner must clean it up through its process group/job.
#[allow(clippy::zombie_processes)]
fn bounded_child_fixture() {
    let Ok(mode) = std::env::var("LEGION_BOUNDED_FIXTURE") else {
        return;
    };
    if mode == "hold" {
        if let Ok(marker) = std::env::var("LEGION_BOUNDED_READY_MARKER") {
            let _ = std::fs::write(marker, b"ready");
        }
        thread::sleep(Duration::from_secs(10));
    } else if mode == "descendant" {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command.args(["--exact", "bounded_child_fixture", "--nocapture"]);
        command.env("LEGION_BOUNDED_FIXTURE", "hold");
        let _child = command.spawn().expect("spawn pipe holder");
        if let Ok(marker) = std::env::var("LEGION_BOUNDED_READY_MARKER") {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline && !std::path::Path::new(&marker).exists() {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                std::path::Path::new(&marker).exists(),
                "descendant did not signal readiness"
            );
            println!("descendant-ready");
        }
    } else if mode == "echo_stdin" {
        // Reads stdin incrementally, echoes the exact bytes back on stdout, and
        // emits live stderr at the same time, so both directions are in flight
        // together and an echo alone cannot be mistaken for a deadlock-free
        // write.
        eprintln!("stdin-live");
        let mut out = std::io::stdout().lock();
        // A request that carries no stdin leaves this child's standard input in
        // the operating system's hands. On Windows a NULL `hStdInput` under
        // `CREATE_NO_WINDOW` resolves to the freshly created console's input
        // buffer: a live character device that no test can write to or close,
        // so reading it blocks until the bounded deadline instead of reporting
        // "no input". Report the kind and stop. Every request that does carry
        // stdin hands this child a pipe, and a pipe is never a terminal, so
        // this branch cannot mask a delivery, chunking, or EOF bug.
        if std::io::stdin().is_terminal() {
            let _ = write!(out, "stdin-console");
            let _ = out.flush();
            return;
        }
        let mut input = std::io::stdin().lock();
        let mut chunk = vec![0u8; 8192];
        let mut total = 0usize;
        let mut next_mark = ECHO_PROGRESS_INTERVAL_BYTES;
        loop {
            match input.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    out.write_all(&chunk[..read]).expect("echo stdin to stdout");
                    out.flush().expect("flush echoed stdin");
                    total += read;
                    while total >= next_mark {
                        eprintln!("stdin-chunk:{next_mark}");
                        next_mark += ECHO_PROGRESS_INTERVAL_BYTES;
                    }
                }
                Err(err) => {
                    let _ = write!(out, "stdin-unavailable:{:?}", err.kind());
                    let _ = out.flush();
                    return;
                }
            }
        }
        let _ = write!(out, "echo-done:{total}");
        let _ = out.flush();
    } else if mode == "stdin_ignore" {
        // Proves it started, then deliberately never reads stdin, so the parent
        // is genuinely back-pressured rather than merely slow.
        let Ok(marker) = std::env::var("LEGION_BOUNDED_READY_MARKER") else {
            return;
        };
        let _ = std::fs::write(&marker, b"ready");
        thread::sleep(IGNORE_FIXTURE_LIFETIME);
        let _ = std::fs::write(format!("{marker}.exit"), b"exit");
    } else if mode == "stdin_exit_early" {
        // Exits without consuming stdin, so the parent is still holding
        // undelivered input when the child is gone.
        println!("exiting-before-stdin");
    }
}

fn emit_command() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "[Console]::Out.Write('ok')".to_string(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        (
            "sh".to_string(),
            vec!["-c".to_string(), "printf ok".to_string()],
        )
    }
}

#[test]
fn bounded_process_returns_normal_output() {
    let (command, args) = emit_command();
    let service = NativeProcessService;
    let result = service
        .execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))))
        .expect("bounded process");
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "ok");
}

#[test]
fn bounded_process_terminates_on_stdout_overflow() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Out.Write('x' * 4096)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "head -c 4096 /dev/zero".to_string()],
            )
        }
    };
    let service = NativeProcessService;
    let result = service.execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit {
            stream: "stdout",
            ..
        })
    ));
}

#[test]
fn bounded_process_terminates_on_stderr_overflow() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Error.Write('x' * 4096)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "head -c 4096 /dev/zero >&2".to_string()],
            )
        }
    };
    let service = NativeProcessService;
    let result = service.execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit {
            stream: "stderr",
            ..
        })
    ));
}

#[test]
fn bounded_process_enforces_timeout_and_kills_descendants_holding_pipes() {
    let marker = std::env::temp_dir().join(format!(
        "legion-bounded-ready-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_file(&marker);
    let request = fixture_request_with_marker("descendant", &marker);
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    let _ = std::fs::remove_file(&marker);
    assert!(
        result.is_ok(),
        "parent exit should allow bounded drain: {result:?}"
    );
    assert!(result.unwrap().stdout.contains("descendant-ready"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "pipe-holding child was not reaped"
    );
}

#[test]
fn bounded_process_enforces_actual_timeout() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "Start-Sleep -Seconds 10".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 10".to_string()],
            )
        }
    };
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&bounded_with(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
        64,
        64,
        Duration::from_millis(100),
    ));
    assert!(matches!(result, Err(PlatformError::Timeout { .. })));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn bounded_process_honors_prelaunch_cancellation() {
    let (command, args) = emit_command();
    let cancellation = Arc::new(AtomicBool::new(true));
    let result = NativeProcessService.execute_bounded(&bounded(command, args, cancellation));
    assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
}

#[test]
fn bounded_process_rejects_invalid_utf8() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::OpenStandardOutput().WriteByte(255)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "printf '\\377'".to_string()],
            )
        }
    };
    let result = NativeProcessService.execute_bounded(&bounded(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
    ));
    assert!(matches!(result, Err(PlatformError::Encoding { .. })));
}

#[test]
fn bounded_process_caps_both_streams_in_one_child() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Out.Write('x' * 4096); [Console]::Error.Write('y' * 4096)"
                        .to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec![
                    "-c".to_string(),
                    "head -c 4096 /dev/zero & head -c 4096 /dev/zero >&2; wait".to_string(),
                ],
            )
        }
    };
    let result = NativeProcessService.execute_bounded(&bounded(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
    ));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit { .. })
    ));
}

#[test]
fn bounded_process_honors_live_cancellation() {
    let cancellation = Arc::new(AtomicBool::new(false));
    let request = {
        #[cfg(windows)]
        {
            bounded(
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "Start-Sleep -Seconds 10".to_string(),
                ],
                cancellation.clone(),
            )
        }
        #[cfg(not(windows))]
        {
            bounded(
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 10".to_string()],
                cancellation.clone(),
            )
        }
    };
    let flag = cancellation.clone();
    let join = thread::spawn(move || NativeProcessService.execute_bounded(&request));
    thread::sleep(Duration::from_millis(50));
    flag.store(true, Ordering::Release);
    let result = join.join().expect("bounded worker");
    assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
}

#[test]
fn bounded_process_requires_finite_timeout() {
    let (command, args) = emit_command();
    let mut request = bounded(command, args, Arc::new(AtomicBool::new(false)));
    request.timeout = Duration::ZERO;
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    assert!(matches!(
        result,
        Err(PlatformError::UnsupportedOperation { .. })
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn bounded_process_streams_large_stdin_without_deadlock() {
    let payload = large_unicode_payload();
    assert!(
        payload.len() > 64 * 1024,
        "payload must exceed any host pipe buffer, got {} bytes",
        payload.len()
    );
    let mut request = fixture_request("echo_stdin");
    request.process.stdin = Some(payload.as_bytes().to_vec());
    request.max_stdout_bytes = 4 * 1024 * 1024;
    request.max_stderr_bytes = 64 * 1024;
    request.timeout = Duration::from_secs(20);

    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    // Asserted before `expect`, and well below the request timeout, so a stalled
    // writer fails here with a diagnostic instead of surfacing as a `Timeout`.
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "streaming stdin larger than the pipe buffer did not complete promptly \
         (took {elapsed:?} against a 20s request timeout)"
    );

    let result = result.expect("bounded stdin echo");
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains(payload.as_str()),
        "child stdout did not contain the exact payload as a contiguous run"
    );
    assert!(
        result
            .stdout
            .contains(&format!("echo-done:{}", payload.len())),
        "child did not report consuming every stdin byte (stdout is {} bytes)",
        result.stdout.len()
    );
    assert!(
        result.stderr.contains("stdin-live"),
        "child stderr was not live while stdin was being written: {}",
        result.stderr
    );
    assert!(
        result.stderr.matches("stdin-chunk:").count() > 1,
        "child emitted no incremental stderr while consuming stdin: {}",
        result.stderr
    );
}

#[test]
fn bounded_process_delivers_empty_stdin_as_eof() {
    let mut request = fixture_request("echo_stdin");
    request.process.stdin = Some(Vec::new());
    request.max_stdout_bytes = 4096;
    request.max_stderr_bytes = 4096;
    request.timeout = Duration::from_secs(10);

    let started = Instant::now();
    let result = NativeProcessService
        .execute_bounded(&request)
        .expect("bounded empty stdin");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "an empty stdin payload must still close the child's stdin, took {elapsed:?}"
    );
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("echo-done:0"),
        "child did not read its stdin to EOF: {}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("stdin-unavailable"),
        "Some(Vec::new()) must still hand the child a readable stdin: {}",
        result.stdout
    );
}

#[test]
fn bounded_process_cancels_while_blocked_writing_stdin() {
    let marker = unique_marker("cancel-stdin");
    let exit_marker = exit_marker_path(&marker);
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_file(&exit_marker);

    let cancellation = Arc::new(AtomicBool::new(false));
    let mut request = fixture_request_with_marker("stdin_ignore", &marker);
    request.process.stdin = Some(over_capacity_payload());
    // Far above any plausible cancellation latency, so a deadline can never
    // masquerade as a cancellation.
    request.timeout = Duration::from_secs(30);
    request.cancellation = cancellation.clone();

    let worker = thread::spawn(move || NativeProcessService.execute_bounded(&request));
    wait_for_marker(&marker, Duration::from_secs(10));
    let ready_at = Instant::now();
    // The child provably started and provably never reads, so by now the pipe
    // buffer is full and the supervisor is genuinely back-pressured.
    thread::sleep(Duration::from_millis(200));
    cancellation.store(true, Ordering::Release);
    let result = worker.join().expect("bounded worker");
    let ran_for = ready_at.elapsed();

    assert!(
        matches!(result, Err(PlatformError::Cancelled { .. })),
        "expected cancellation while stdin was back-pressured, got {result:?}"
    );
    assert!(
        ran_for < Duration::from_secs(5),
        "cancellation was not honoured while stdin was back-pressured \
         (ran {ran_for:?} past observed child readiness)"
    );

    while ready_at.elapsed() < IGNORE_FIXTURE_LIFETIME + Duration::from_millis(800) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !exit_marker.exists(),
        "cancelled child outlived the bounded run and wrote {}",
        exit_marker.display()
    );
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_file(&exit_marker);
}

#[test]
fn bounded_process_times_out_while_blocked_writing_stdin() {
    let marker = unique_marker("timeout-stdin");
    let exit_marker = exit_marker_path(&marker);
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_file(&exit_marker);

    let mut request = fixture_request_with_marker("stdin_ignore", &marker);
    request.process.stdin = Some(over_capacity_payload());
    request.timeout = Duration::from_millis(1_500);

    let worker = thread::spawn(move || NativeProcessService.execute_bounded(&request));
    wait_for_marker(&marker, Duration::from_secs(10));
    let ready_at = Instant::now();
    let result = worker.join().expect("bounded worker");
    let ran_for = ready_at.elapsed();

    assert!(
        matches!(result, Err(PlatformError::Timeout { .. })),
        "expected a deadline while stdin was back-pressured, got {result:?}"
    );
    assert!(
        ran_for < Duration::from_secs(5),
        "deadline was not enforced while stdin was back-pressured \
         (bounded run took {ran_for:?} past observed child readiness for a 1.5s timeout)"
    );

    while ready_at.elapsed() < IGNORE_FIXTURE_LIFETIME + Duration::from_millis(800) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !exit_marker.exists(),
        "timed-out child outlived the bounded run and wrote {}",
        exit_marker.display()
    );
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_file(&exit_marker);
}

#[test]
fn bounded_process_reports_write_failure_when_child_exits_early() {
    let mut request = fixture_request("stdin_exit_early");
    request.process.stdin = Some(over_capacity_payload());
    request.max_stdout_bytes = 4096;
    request.max_stderr_bytes = 4096;
    request.timeout = Duration::from_secs(15);

    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "a child that exits before consuming stdin must fail fast, took {elapsed:?}"
    );
    match result {
        Err(PlatformError::Io { operation, .. }) => assert!(
            operation.starts_with("write bounded process stdin"),
            "unexpected structured error operation: {operation}"
        ),
        other => {
            panic!("undelivered stdin must never produce Ok, even at exit code 0; got {other:?}")
        }
    }
}

#[test]
fn bounded_process_rejects_oversize_stdin_before_spawn() {
    let marker = unique_marker("oversize-stdin");
    let _ = std::fs::remove_file(&marker);
    let mut request = fixture_request_with_marker("hold", &marker);
    request.process.stdin = Some(vec![b'x'; MAX_BOUNDED_STDIN_BYTES + 1]);

    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    let elapsed = started.elapsed();

    assert!(
        matches!(result, Err(PlatformError::UnsupportedOperation { .. })),
        "over-limit stdin must be a structured rejection, got {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "over-limit stdin was not rejected before spawn, took {elapsed:?}"
    );
    thread::sleep(Duration::from_millis(250));
    assert!(
        !marker.exists(),
        "over-limit stdin spawned a child that reached readiness at {}",
        marker.display()
    );
    let _ = std::fs::remove_file(&marker);
}

#[test]
fn bounded_process_none_stdin_behaves_as_before() {
    let (command, args) = emit_command();
    let result = NativeProcessService
        .execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))))
        .expect("bounded process without stdin");
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "ok");
    assert!(
        result.stderr.is_empty(),
        "unexpected stderr on the None-stdin path: {}",
        result.stderr
    );

    let mut request = fixture_request("echo_stdin");
    request.max_stdout_bytes = 4096;
    request.max_stderr_bytes = 4096;
    request.timeout = Duration::from_secs(10);
    assert!(
        request.process.stdin.is_none(),
        "this case must exercise the untouched None path"
    );
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    // Asserted before `expect`, and well below the request timeout: a None
    // request that acquired a runner-owned stdin pipe would leave the child
    // reading a pipe nobody closes until the 10s deadline, and that regression
    // must fail here with a diagnostic instead of inside `expect`.
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "a None-stdin child did not finish promptly (took {elapsed:?} against a 10s request timeout)"
    );

    let result = result.expect("bounded fixture without stdin");
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stderr.contains("stdin-live"),
        "the None-stdin case did not reach the echo fixture at all: {}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("stdin-chunk:"),
        "a None-stdin child must consume no input bytes: {}",
        result.stderr
    );
    // The None path is byte-identical to the pre-packet code on both platforms,
    // so this pins what each platform actually hands the child.
    //
    // Windows: `hStdInput` stays NULL. Under `CREATE_NO_WINDOW` that resolves
    // to the new console's input buffer (`stdin-console`), or to no handle at
    // all where no console can be created (`stdin-unavailable`). It must never
    // be a readable-then-closed stream: `echo-done:` would mean the runner had
    // started handing this path a stdin of its own.
    #[cfg(windows)]
    {
        assert!(
            result.stdout.contains("stdin-console") || result.stdout.contains("stdin-unavailable"),
            "a None-stdin child must report an OS-owned stdin it was never fed: {}",
            result.stdout
        );
        assert!(
            !result.stdout.contains("echo-done:"),
            "the None path must not hand the child a runner-owned stdin: {}",
            result.stdout
        );
    }
    // Unix: `Stdio::null()`, so the child reads immediate EOF and reports zero
    // bytes. Unassessed on this host; see the blocked Unix row.
    #[cfg(not(windows))]
    {
        assert!(
            result.stdout.contains("echo-done:0"),
            "a None-stdin child must observe no input bytes: {}",
            result.stdout
        );
    }
}
