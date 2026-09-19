//! Argument parsing and the four outcome codes for `legion-input-driver`.
//!
//! Host-independent by construction: nothing in this module touches the OS, so
//! the outcome vocabulary can be tested headlessly on any host.
//!
//! The argument shape is **not** this crate's to choose. It is fixed by the
//! harness in `xtask/src/native_product_acceptance.rs`, which spawns the driver
//! with `--probe-session --report <path>` and then
//! `--conformance-run --product <exe> --report <path>`.

use std::path::PathBuf;

/// Every input class was observed to conform, from outside the product
/// process, on a real window. Never the mere absence of a failure.
pub const EXIT_PASSED: i32 = 0;
/// The driver reached the product and the product's behaviour deviated.
/// **The product is wrong.**
pub const EXIT_CONFORMANCE_FAILED: i32 = 1;
/// The driver itself could not operate: unreadable arguments, or a report that
/// could not be written.
pub const EXIT_OPERATIONAL_ERROR: i32 = 2;
/// This host cannot answer the question. Never a pass, never a skip, never
/// `0`.
pub const EXIT_BLOCKED: i32 = 3;

/// The two subcommands the harness invokes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `--probe-session --report <path>`: attach to the input desktop.
    ProbeSession {
        /// Where to write the session handshake report.
        report: PathBuf,
    },
    /// `--conformance-run --product <exe> --report <path>`: drive the product.
    ConformanceRun {
        /// The packaged product executable to launch as a subprocess.
        product: PathBuf,
        /// Where to write the per-class result report.
        report: PathBuf,
    },
}

/// A command line this driver cannot act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    /// Human-readable reason, printed to stderr before exiting.
    pub message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Usage string, printed on a parse error.
pub const USAGE: &str = concat!(
    "usage: legion-input-driver --probe-session --report <path>\n",
    "       legion-input-driver --conformance-run --product <exe> --report <path>"
);

/// Parse the driver's arguments (already stripped of `argv[0]`).
///
/// Unknown flags are rejected rather than ignored: a driver that silently
/// discards an argument it does not understand would happily run the wrong
/// mode and report a result for a question nobody asked.
pub fn parse(args: &[String]) -> Result<Command, CliError> {
    let mut probe_session = false;
    let mut conformance_run = false;
    let mut product: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--probe-session" => probe_session = true,
            "--conformance-run" => conformance_run = true,
            "--product" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| CliError::new("--product requires a path"))?;
                product = Some(PathBuf::from(value));
            }
            "--report" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| CliError::new("--report requires a path"))?;
                report = Some(PathBuf::from(value));
            }
            other => {
                return Err(CliError::new(format!("unrecognised argument `{other}`")));
            }
        }
        index += 1;
    }

    let report = report.ok_or_else(|| CliError::new("--report <path> is required"))?;

    match (probe_session, conformance_run) {
        (true, true) => Err(CliError::new(
            "--probe-session and --conformance-run are mutually exclusive",
        )),
        (false, false) => Err(CliError::new(
            "one of --probe-session or --conformance-run is required",
        )),
        (true, false) => Ok(Command::ProbeSession { report }),
        (false, true) => {
            let product =
                product.ok_or_else(|| CliError::new("--conformance-run requires --product"))?;
            Ok(Command::ConformanceRun { product, report })
        }
    }
}
