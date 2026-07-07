//! Consent-gated Rust panic capture — writes local crash bundles on opt-in.
//!
//! This module installs a [`std::panic::set_hook`] that writes crash bundles to a
//! configured local directory. The hook is **only** installed when consent is
//! explicitly enabled; when consent is off the call returns immediately
//! (fail-closed: nothing is registered).
//!
//! Crash bundles stay **local** — there is no upload path and none may be added.
//!
//! ## Bundle layout
//!
//! ```text
//! <bundle_dir>/
//!   <crash_id>/
//!     panic.txt    — panic message, location, and symbolicated backtrace
//!     summary.toml — metadata-only: crash_id, timestamp, version, os, arch,
//!                    panic_message (sanitized), panic_location, signer_status
//!     audit.json   — panic audit envelope (event name, retention/redaction labels,
//!                    crash_id, location, timestamp, consent state); satisfies the
//!                    audit-trail requirement without using EventEnvelopeBuilder
//!                    (see emit_panic_audit_envelope for why)
//! ```
//!
//! ## What is NOT written
//!
//! - Raw source file contents
//! - Full filesystem paths (paths are redacted to `<crate>/src/file.rs` form)
//! - Any data transmitted over a network

use std::path::{Path, PathBuf};

use legion_protocol::WorkbenchTelemetryConsent;
use thiserror::Error;

/// Configuration for consent-gated panic capture.
pub struct CrashCaptureConfig {
    /// Directory under which per-crash subdirectories are written.
    pub bundle_dir: PathBuf,
    /// Consent payload that gates whether the panic hook is installed.
    pub consent: WorkbenchTelemetryConsent,
}

/// Errors from panic hook installation or bundle I/O.
#[derive(Debug, Error)]
pub enum CrashCaptureError {
    /// I/O failure creating the bundle directory or writing bundle files.
    #[error("crash capture I/O error: {0}")]
    IoError(#[from] std::io::Error),
    /// Hook was not installed because crash reporting consent is disabled.
    #[error("crash reporting consent is disabled")]
    ConsentDenied,
}

/// Install a consent-gated panic hook that writes crash bundles to `config.bundle_dir`.
///
/// Returns `Ok(())` immediately **without installing a hook** if
/// `config.consent.crash_reports_enabled` is `false` — fail-closed.
///
/// When consent is enabled:
/// 1. Creates `config.bundle_dir` if it does not exist.
/// 2. Installs a `std::panic::set_hook` that writes `panic.txt` and
///    `summary.toml` under `<bundle_dir>/<crash_id>/`.
pub fn install_panic_hook(config: CrashCaptureConfig) -> Result<(), CrashCaptureError> {
    if !config.consent.crash_reports_enabled {
        // Fail-closed: return Ok without installing anything.
        return Ok(());
    }

    std::fs::create_dir_all(&config.bundle_dir)?;

    let bundle_dir = config.bundle_dir.clone();
    let consent = config.consent.clone();

    std::panic::set_hook(Box::new(move |info| {
        write_crash_bundle(&bundle_dir, info, &consent);
    }));

    Ok(())
}

/// Restore the default panic hook by taking and dropping the current hook.
///
/// After this call the process reverts to the built-in Rust panic handler.
/// Call this in tests after each hook installation to restore clean state.
pub fn uninstall_panic_hook() {
    let _ = std::panic::take_hook();
}

// ---------------------------------------------------------------------------
// Internal implementation — not part of the public API
// ---------------------------------------------------------------------------

fn write_crash_bundle(
    bundle_dir: &Path,
    info: &std::panic::PanicHookInfo<'_>,
    consent: &WorkbenchTelemetryConsent,
) {
    let crash_id = uuid::Uuid::new_v4().to_string();
    let crash_dir = bundle_dir.join(&crash_id);

    if std::fs::create_dir_all(&crash_dir).is_err() {
        return;
    }

    let backtrace = std::backtrace::Backtrace::force_capture();

    // Extract the panic message from the payload.
    let message: String = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    };

    // Build a redacted location (file:line, path redacted).
    let location = info
        .location()
        .map(|loc| format!("{}:{}", redact_path(loc.file()), loc.line()))
        .unwrap_or_else(|| "unknown:0".to_string());

    // --- panic.txt: full local detail for offline debugging ---
    let panic_txt =
        format!("panic: {message}\nlocation: {location}\n\nstack backtrace:\n{backtrace}");
    let _ = std::fs::write(crash_dir.join("panic.txt"), panic_txt);

    // --- summary.toml: metadata-only, no raw source ---
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let safe_message = sanitize_message(&message);

    let summary_toml = format!(
        "crash_id = \"{crash_id}\"\n\
         timestamp = {timestamp}\n\
         version = \"{version}\"\n\
         os = \"{os}\"\n\
         arch = \"{arch}\"\n\
         panic_message = \"{panic_message}\"\n\
         panic_location = \"{panic_location}\"\n\
         signer_status = \"unsigned-beta\"\n",
        crash_id = toml_escape(&crash_id),
        timestamp = timestamp,
        version = toml_escape(env!("CARGO_PKG_VERSION")),
        os = toml_escape(std::env::consts::OS),
        arch = toml_escape(std::env::consts::ARCH),
        panic_message = toml_escape(&safe_message),
        panic_location = toml_escape(&location),
    );
    let _ = std::fs::write(crash_dir.join("summary.toml"), summary_toml);

    // Emit the panic audit envelope last — infallible, must not prevent
    // panic.txt / summary.toml from being written even if it fails.
    emit_panic_audit_envelope(&crash_dir, &crash_id, &location, consent);
}

/// Write a minimal panic audit record to `<crash_dir>/audit.json`.
///
/// # Why not `EventEnvelopeBuilder`
///
/// `EventEnvelopeBuilder` (from `crate::`) is wired through `validate_envelope`,
/// which allocates and may itself panic. This function runs *inside a panic hook*
/// where the allocator may be in an inconsistent state after the original panic.
/// Additionally, the hook has no access to session-level `CausalityId` /
/// `CorrelationId` values (they are not carried through `std::panic::set_hook`
/// closures without introducing shared-state machinery that creates its own hazards).
///
/// Writing a plain JSON file via a single `fs::write` call is the safest path
/// that still produces a durable, auditable artifact — satisfying the spec
/// requirement that "the crash event produces an audit-trail artifact."
///
/// Fields mirror the envelope schema (event name, retention, redaction, consent
/// state). The record is local-only — never transmitted.
///
/// This function is **infallible**: all errors are silently dropped so they cannot
/// prevent `panic.txt` or `summary.toml` from being written.
fn emit_panic_audit_envelope(
    crash_dir: &Path,
    crash_id: &str,
    panic_location: &str,
    consent: &WorkbenchTelemetryConsent,
) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let audit = serde_json::json!({
        "event": "crash_capture.panic_audit_recorded",
        "retention": "Audit",
        "redaction": "MetadataOnly",
        "payload_class": "metadata_only",
        "crash_id": crash_id,
        "panic_location": panic_location,
        "timestamp": timestamp,
        "telemetry_enabled": consent.enabled,
        "crash_reports_enabled": consent.crash_reports_enabled,
        "consent_label": consent.consent_label,
    });

    if let Ok(json_str) = serde_json::to_string_pretty(&audit) {
        let _ = std::fs::write(crash_dir.join("audit.json"), json_str);
    }
}

/// Redact an absolute or workspace-relative filesystem path to a safe form.
///
/// Keeps only the `<crate-name>/src/<relative>` tail, or just the filename, so
/// that absolute machine paths are never written into audit records.
fn redact_path(path: &str) -> String {
    let components: Vec<&str> = path.split(['/', '\\']).collect();

    // Look for a "src" component: keep everything from the crate name before it.
    if let Some(src_idx) = components.iter().rposition(|&c| c == "src")
        && src_idx >= 1
    {
        let crate_name = components[src_idx - 1];
        let rest = &components[src_idx..];
        return format!("{}/{}", crate_name, rest.join("/"));
    }

    // Fallback: just the filename.
    components.last().copied().unwrap_or(path).to_string()
}

/// Strip absolute-path-looking tokens from a panic message.
///
/// Tokens that look like absolute or workspace-relative paths are replaced with
/// `<path>` so the summary.toml field contains no filesystem artifacts.
fn sanitize_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(|word| {
            if word.starts_with('/')
                || word.len() > 2 && word.as_bytes()[1] == b':'
                || word.contains("/src/")
                || word.contains("\\src\\")
            {
                "<path>"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape a string for inclusion as a TOML basic string value.
fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn redact_path_strips_absolute_prefix() {
        let result = redact_path("/home/user/.cargo/registry/src/github.com-x/foo/src/lib.rs");
        assert!(result.contains("src/lib.rs"), "got: {result}");
        assert!(!result.contains("/home"), "got: {result}");
    }

    #[test]
    fn redact_path_keeps_crate_src_form() {
        let result = redact_path("crates/legion-observability/src/crash_capture.rs");
        assert_eq!(result, "legion-observability/src/crash_capture.rs");
    }

    #[test]
    fn sanitize_message_strips_absolute_path() {
        let msg = "file /home/user/project/src/main.rs not found";
        let sanitized = sanitize_message(msg);
        assert!(!sanitized.contains("/home"), "got: {sanitized}");
        assert!(sanitized.contains("<path>"), "got: {sanitized}");
    }

    #[test]
    fn toml_escape_handles_quotes_and_backslashes() {
        assert_eq!(toml_escape(r#"say "hello""#), r#"say \"hello\""#);
        assert_eq!(toml_escape("C:\\Users\\foo"), "C:\\\\Users\\\\foo");
    }
}
