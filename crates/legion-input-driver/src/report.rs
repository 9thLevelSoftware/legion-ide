//! TOML rendering of the two report files the harness reads.
//!
//! Host-independent by construction, so every marker in the wire protocol can
//! be asserted headlessly.
//!
//! # The wire protocol is not this crate's to change
//!
//! `xtask/src/native_product_acceptance.rs` reads these files as parsed TOML
//! and looks up `interactive_session`, `window_created`, and
//! `input_class_<class>` for each of the six classes. Those field names are
//! the contract. This module writes them, and `tests/driver_contract.rs`
//! asserts them as literals written out in the test rather than as constants
//! imported from here — if both sides read one constant, a rename breaks
//! nothing and the guard is worthless.
//!
//! # Why free-form text is scrubbed
//!
//! Every operator-facing string that reaches these files goes through
//! [`toml_string`], which rewrites the three marker prefixes before escaping.
//! A driver that put an OS error message straight into a note could otherwise
//! forge the session marker inside a *blocked* report and turn a host that
//! cannot answer the question into a pass.

use crate::cli::{EXIT_BLOCKED, EXIT_CONFORMANCE_FAILED, EXIT_PASSED};
use crate::session::DesktopAttachment;

/// The six input classes named by `COMP-PLAT-002`, in the harness's order.
pub const INPUT_CLASSES: [&str; 6] = [
    "keyboard",
    "pointer",
    "text",
    "clipboard",
    "ime-cjk",
    "command",
];

/// Exact, actionable prerequisite for a host with no interactive input
/// desktop.
pub const PREREQUISITE_NO_INPUT_DESKTOP: &str = concat!(
    "A Windows 11 x64 host running this driver inside an interactive ",
    "logged-in desktop session (an attended session, or an autologon runner ",
    "with a real window station), not a service or headless session, so the ",
    "driver can open the input desktop and attach its thread to it."
);

/// Exact, actionable prerequisite for macOS and Linux, which stay blocked on
/// `BLK-2026-09-08-02`. A Windows result never substitutes for either row.
pub const PREREQUISITE_UNSUPPORTED_HOST: &str = concat!(
    "A macOS host and a Linux host with the packaged native product installed ",
    "and a real display session, able to run the windowed GUI e2e suite ",
    "(BLK-2026-09-08-02). This driver injects OS-level input and reads UI ",
    "Automation on Windows only; a Windows result never substitutes for a ",
    "macOS or Linux row."
);

/// Exact, actionable prerequisite when `--product` names nothing on disk.
pub const PREREQUISITE_NO_PRODUCT: &str = concat!(
    "A packaged native Legion product executable at the path passed to ",
    "`--product`, so the driver can launch the packaged product as a ",
    "subprocess rather than a development build (BLK-2026-09-08-04)."
);

/// Last-resort prerequisite for a blocked run that states no reason of its own.
///
/// [`blocked_reasons`] comes back empty only for a report that is blocked while
/// a window was observed, every one of [`INPUT_CLASSES`] carries an observation
/// and none of those is blocked — a shape [`conformance_exit_code`] cannot
/// produce. The constant exists so [`render_conformance_report`] can never
/// write an unactionable `prerequisite = ""` onto a blocked report, whatever
/// value it is handed.
pub const PREREQUISITE_UNSTATED_BLOCKED_REASON: &str = concat!(
    "A `--conformance-run` result that states why it was blocked. This result ",
    "reports a blocked run while naming no blocked class, no unobserved class ",
    "and no missing window, so the driver published no reason an owner can act ",
    "on. Rebuild the driver with `cargo build -p legion-input-driver --release` ",
    "and re-run; the product is not implicated."
);

/// How one input class came out. There is no fourth value, and in particular
/// no "assumed" and no "skipped": an unobserved class is simply absent from the
/// report, which already reads as not-observed on the harness side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassOutcome {
    /// Observed, from outside the product process, to conform.
    Conforms,
    /// Observed, from outside the product process, to deviate.
    Deviates,
    /// The oracle could not be established on this host.
    Blocked,
}

impl ClassOutcome {
    /// The literal written into the report.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conforms => "conforms",
            Self::Deviates => "deviates",
            Self::Blocked => "blocked",
        }
    }
}

/// One class's observation. Constructing this requires naming an outcome, so
/// there is no code path that emits a class line without one.
#[derive(Debug, Clone)]
pub struct ClassObservation {
    /// One of [`INPUT_CLASSES`].
    pub class: &'static str,
    /// What was observed.
    pub outcome: ClassOutcome,
    /// What the oracle actually saw, for the operator reading the artifact.
    pub detail: String,
}

impl ClassObservation {
    /// Record an observation for `class`.
    pub fn new(class: &'static str, outcome: ClassOutcome, detail: impl Into<String>) -> Self {
        Self {
            class,
            outcome,
            detail: detail.into(),
        }
    }
}

/// The `--conformance-run` result.
#[derive(Debug, Clone, Default)]
pub struct ConformanceReport {
    /// The product executable this run was pointed at.
    pub product: String,
    /// Whether a native top-level window of the product process was actually
    /// observed from outside it.
    pub window_created: bool,
    /// Only classes that were really observed. Never pre-populated.
    pub observations: Vec<ClassObservation>,
    /// Exact prerequisite; present when the host could not answer.
    pub prerequisite: Option<String>,
    /// Free-form notes. Never a substitute for the fields above.
    pub notes: Vec<String>,
}

/// Exit code for a session probe. Attaching is the only zero.
pub fn session_exit_code(attachment: &DesktopAttachment) -> i32 {
    match attachment {
        DesktopAttachment::Attached { .. } => EXIT_PASSED,
        DesktopAttachment::NotAttached { .. } | DesktopAttachment::UnsupportedHost { .. } => {
            EXIT_BLOCKED
        }
    }
}

/// Exit code for a conformance run.
///
/// A deviation the driver actually observed outranks a blocked class: a real
/// product finding must not be hidden behind an oracle that happened to be
/// unavailable for some other class. Everything else that is not a complete,
/// windowed, six-of-six pass is blocked, because the driver has no positive
/// evidence to report and the product is not implicated by the absence of it.
pub fn conformance_exit_code(report: &ConformanceReport) -> i32 {
    if report
        .observations
        .iter()
        .any(|observation| observation.outcome == ClassOutcome::Deviates)
    {
        return EXIT_CONFORMANCE_FAILED;
    }

    let every_class_conforms = INPUT_CLASSES.iter().all(|class| {
        report.observations.iter().any(|observation| {
            observation.class == *class && observation.outcome == ClassOutcome::Conforms
        })
    });

    if report.window_created && every_class_conforms {
        EXIT_PASSED
    } else {
        EXIT_BLOCKED
    }
}

/// Status string matching [`conformance_exit_code`].
pub fn conformance_status(report: &ConformanceReport) -> &'static str {
    match conformance_exit_code(report) {
        EXIT_PASSED => "passed",
        EXIT_CONFORMANCE_FAILED => "conformance-failed",
        _ => "blocked",
    }
}

/// The classes named by [`INPUT_CLASSES`] that this report carries no
/// observation for at all.
///
/// An unobserved class is not the same thing as a blocked one: the blocked
/// class has a reason, and this one does not even have that. Both are reasons a
/// run is blocked, and neither is ever rendered as conforming.
pub fn unobserved_classes(report: &ConformanceReport) -> Vec<&'static str> {
    INPUT_CLASSES
        .iter()
        .copied()
        .filter(|class| {
            !report
                .observations
                .iter()
                .any(|observation| observation.class == *class)
        })
        .collect()
}

/// Why this run could not answer the question, in the run's own words.
///
/// Each blocked class contributes its own `detail`, which is where the exact
/// prerequisite text already lives (`observe_ime_cjk` in `main.rs` is the
/// worked example: its detail opens with the CJK IME prerequisite verbatim).
/// Nothing here is reworded, and nothing here is invented.
pub fn blocked_reasons(report: &ConformanceReport) -> Vec<String> {
    let mut reasons = Vec::new();
    if !report.window_created {
        reasons.push(
            "no native top-level window of the product process was observed from outside it"
                .to_string(),
        );
    }
    for observation in &report.observations {
        if observation.outcome == ClassOutcome::Blocked {
            reasons.push(format!(
                "input class `{}` was blocked: {}",
                observation.class, observation.detail
            ));
        }
    }
    let unobserved = unobserved_classes(report);
    if !unobserved.is_empty() {
        reasons.push(format!(
            "no observation at all was recorded for input {}: {}",
            if unobserved.len() == 1 {
                "class"
            } else {
                "classes"
            },
            unobserved
                .iter()
                .map(|class| format!("`{class}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    reasons
}

/// The top-level prerequisite a blocked run composes from its own observations.
///
/// This is what closes the gap where a run with five conforming classes and one
/// blocked class reported `prerequisite = ""`: the reason lives in the blocked
/// class's `detail`, so the top-level prerequisite is composed from it and
/// names the class it came from.
pub fn blocked_run_prerequisite(report: &ConformanceReport) -> String {
    let reasons = blocked_reasons(report);
    if reasons.is_empty() {
        return PREREQUISITE_UNSTATED_BLOCKED_REASON.to_string();
    }
    let mut composed = format!(
        "This host could not answer COMP-PLAT-002 for the packaged product, and the product is \
         not implicated. Supply what these observations name and re-run: {}",
        reasons.join("; ")
    );
    // A blocked class's detail usually ends its own sentence; do not double the
    // full stop, and do not leave one missing when it does not.
    if !composed.ends_with('.') {
        composed.push('.');
    }
    composed
}

/// The prerequisite this report actually renders.
///
/// Precedence, and there is no fourth case:
///
/// 1. a stated, non-empty top-level `prerequisite` wins, verbatim — the caller
///    already knew a more specific reason than the observations carry;
/// 2. otherwise a **blocked** run composes one with [`blocked_run_prerequisite`];
/// 3. otherwise there is none. A passing run has no prerequisite, and neither
///    does a `conformance-failed` one: a deviation the driver observed is a
///    product finding, not a missing host capability, and dressing it as a
///    prerequisite would hide it.
pub fn effective_prerequisite(report: &ConformanceReport) -> Option<String> {
    if let Some(stated) = report.prerequisite.as_deref()
        && !stated.trim().is_empty()
    {
        return Some(stated.to_string());
    }
    if conformance_exit_code(report) == EXIT_BLOCKED {
        return Some(blocked_run_prerequisite(report));
    }
    None
}

/// Render the `--probe-session` report.
///
/// The session success marker is written in exactly one arm of this match: the
/// one reached only after the thread really attached to the input desktop.
pub fn render_session_report(attachment: &DesktopAttachment) -> String {
    let exit_code = session_exit_code(attachment);
    let mut text = String::new();
    text.push_str("# legion-input-driver --probe-session (ADR-0056)\n");
    text.push_str("# COMP-PLAT-002 interactive-desktop handshake.\n");
    text.push_str("schema_version = 1\n");
    text.push_str("driver = \"legion-input-driver\"\n");
    text.push_str("mode = \"probe-session\"\n");
    text.push_str(&format!("host_os = \"{}\"\n", host_os_label()));
    text.push_str(&format!("exit_code = {exit_code}\n"));

    match attachment {
        DesktopAttachment::Attached { desktop } => {
            text.push_str("status = \"attached\"\n");
            text.push_str("interactive_session = true\n");
            text.push_str(&format!("input_desktop = \"{}\"\n", toml_string(desktop)));
            text.push_str("prerequisite = \"\"\n");
        }
        DesktopAttachment::NotAttached { detail } => {
            text.push_str("status = \"blocked\"\n");
            text.push_str("interactive_session = false\n");
            text.push_str("input_desktop = \"\"\n");
            text.push_str(&format!("detail = \"{}\"\n", toml_string(detail)));
            text.push_str(&format!(
                "prerequisite = \"{}\"\n",
                toml_string(PREREQUISITE_NO_INPUT_DESKTOP)
            ));
        }
        DesktopAttachment::UnsupportedHost { detail } => {
            text.push_str("status = \"blocked\"\n");
            text.push_str("interactive_session = false\n");
            text.push_str("input_desktop = \"\"\n");
            text.push_str(&format!("detail = \"{}\"\n", toml_string(detail)));
            text.push_str(&format!(
                "prerequisite = \"{}\"\n",
                toml_string(PREREQUISITE_UNSUPPORTED_HOST)
            ));
        }
    }
    text
}

/// Render the `--conformance-run` report.
///
/// The per-class loop walks [`ConformanceReport::observations`], never
/// [`INPUT_CLASSES`], so a class with no observation produces no line at all.
/// There is no default, no fallback and no `unwrap_or` that could turn a class
/// nobody looked at into a conforming one.
pub fn render_conformance_report(report: &ConformanceReport) -> String {
    let mut text = String::new();
    text.push_str("# legion-input-driver --conformance-run (ADR-0056)\n");
    text.push_str("# COMP-PLAT-002 per-class native input observations.\n");
    text.push_str("schema_version = 1\n");
    text.push_str("driver = \"legion-input-driver\"\n");
    text.push_str("mode = \"conformance-run\"\n");
    text.push_str(&format!("host_os = \"{}\"\n", host_os_label()));
    text.push_str(&format!(
        "status = \"{}\"\n",
        toml_string(conformance_status(report))
    ));
    text.push_str(&format!("exit_code = {}\n", conformance_exit_code(report)));
    text.push_str(&format!("product = \"{}\"\n", toml_string(&report.product)));
    text.push_str(&format!(
        "window_created = {}\n",
        if report.window_created {
            "true"
        } else {
            "false"
        }
    ));
    text.push_str(&format!(
        "required_input_classes = [{}]\n",
        INPUT_CLASSES
            .iter()
            .map(|class| format!("\"{}\"", toml_string(class)))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    for observation in &report.observations {
        text.push_str(&format!(
            "input_class_{} = \"{}\"\n",
            observation.class,
            observation.outcome.as_str()
        ));
        text.push_str(&format!(
            "input_class_{}_detail = \"{}\"\n",
            observation.class,
            toml_string(&observation.detail)
        ));
    }

    // A blocked run renders the reason it actually has: its own stated
    // prerequisite, or one composed from the blocked classes' details. A
    // blocked result with an empty prerequisite is unactionable, and the
    // harness would have nothing to propagate.
    text.push_str(&format!(
        "prerequisite = \"{}\"\n",
        toml_string(&effective_prerequisite(report).unwrap_or_default())
    ));
    text.push_str(&format!(
        "notes = [{}]\n",
        report
            .notes
            .iter()
            .map(|note| format!("\"{}\"", toml_string(note)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    text
}

/// The blocked `--conformance-run` report for a host ADR-0056 does not enable.
pub fn unsupported_host_conformance_report(product: &str) -> ConformanceReport {
    ConformanceReport {
        product: product.to_string(),
        window_created: false,
        observations: Vec::new(),
        prerequisite: Some(PREREQUISITE_UNSUPPORTED_HOST.to_string()),
        notes: vec![format!(
            "`{}` is not a native input host enabled by ADR-0056; no product was launched and \
             no class was observed",
            host_os_label()
        )],
    }
}

/// The blocked `--conformance-run` report for an absent packaged product.
pub fn missing_product_conformance_report(product: &str) -> ConformanceReport {
    ConformanceReport {
        product: product.to_string(),
        window_created: false,
        observations: Vec::new(),
        prerequisite: Some(PREREQUISITE_NO_PRODUCT.to_string()),
        notes: vec![
            "no packaged product executable at the path passed to --product; nothing was \
             launched, no class was observed, and the product is not implicated"
                .to_string(),
        ],
    }
}

fn host_os_label() -> &'static str {
    std::env::consts::OS
}

/// Scrub the wire-protocol marker prefixes out of operator-facing text, then
/// escape it for a TOML basic string.
///
/// The scrub is not cosmetic. Without it an OS error message, a file path or a
/// product-supplied UI Automation string could carry a marker prefix into a
/// report the harness reads by substring, and a blocked run would be read as a
/// pass.
pub fn toml_string(value: &str) -> String {
    let scrubbed = value
        .replace("interactive_session", "interactive-session")
        .replace("window_created", "window-created")
        .replace("input_class_", "input-class-");

    let mut escaped = String::with_capacity(scrubbed.len());
    for ch in scrubbed.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}
