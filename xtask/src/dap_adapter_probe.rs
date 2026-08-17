//! `dap-adapter-probe`: report which DAP adapter binaries a machine actually has.
//!
//! This exists because P2.F3.T2's dogfood tests were reporting `ok` through a
//! soft-skip branch on every runner, and a clean skip is not proof. Before a
//! dogfood run can mean anything, someone has to establish what each machine
//! ships. This command is that step: it looks for the binaries
//! `legion-debug`'s resolver looks for, prints what it found, and writes a TOML
//! report so a CI artifact records it.
//!
//! It is a **diagnostic**, not a resolver, and deliberately not a gate by
//! default. It never mints an [`legion_debug::AdapterResolutionGrant`]-style
//! authorization, never launches a debug session, and reports what is on disk
//! regardless of whether policy would permit launching it. `allowlisted_stem`
//! is reported as an observation so an operator can see the difference between
//! "the platform has an adapter" and "the shipped allowlist would accept it" —
//! the versioned `lldb-dap-18` that Debian/Ubuntu installs is exactly that gap.
//!
//! `xtask` may not depend on `legion-debug` (`plans/dependency-policy.md`), so
//! the name list below is duplicated rather than imported.
//! [`tests::probe_names_match_the_resolver_alias_list`] reads the resolver's
//! source and fails when the two drift.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Adapter binary names `legion-debug`'s `path_candidates` searches `PATH` for.
///
/// Must stay equal to the alias array in
/// `crates/legion-debug/src/adapter_resolve.rs`; the drift guard in this
/// module's tests enforces it.
pub const PROBE_NAMES: [&str; 3] = ["lldb-dap", "lldb-vscode", "codelldb"];

/// Where an adapter on this machine came from, as far as the caller knows.
///
/// The distinction matters for evidence: "the platform ships this" and "our
/// workflow installed this" support different claims, and a report that
/// conflates them cannot be read back later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// Nothing was installed by us before the probe ran: this is the image.
    Shipped,
    /// A workflow step installed an adapter before the probe ran.
    Installed,
    /// Caller did not say. Reported as-is rather than guessed.
    Unknown,
}

impl Provenance {
    /// Parse the `--provenance` flag value.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "shipped" | "preinstalled" => Ok(Self::Shipped),
            "installed" => Ok(Self::Installed),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(format!(
                "unknown provenance `{other}` (expected shipped|installed|unknown)"
            )),
        }
    }

    /// Stable string used in the report and in log lines.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shipped => "shipped",
            Self::Installed => "installed",
            Self::Unknown => "unknown",
        }
    }
}

/// One adapter binary found under a name the resolver searches for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdapterFinding {
    /// Name that was searched for (`lldb-dap`, …), not the on-disk file name.
    pub name: String,
    /// Absolute-or-`PATH`-relative location of the executable that was found.
    pub path: PathBuf,
    /// Whether the shipped default allowlist would accept this file stem.
    pub allowlisted_stem: bool,
    /// First line of `--version` output, when the binary answered in time.
    pub version: Option<String>,
}

/// A binary whose name is a versioned variant of a searched name.
///
/// `lldb-dap-18` is what `apt-get install lldb-18` leaves on `PATH`. The
/// resolver will not find it (it searches exact names) and the allowlist would
/// not accept its stem, so a runner can have a perfectly good adapter and still
/// resolve nothing. Reporting these separately is the difference between
/// "no adapter here" and "an adapter is here under a name we do not look for".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VariantFinding {
    /// On-disk file name, e.g. `lldb-dap-18`.
    pub file_name: String,
    /// Searched name it is a variant of.
    pub base_name: String,
    /// Where it was found.
    pub path: PathBuf,
}

/// Everything one probe run observed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProbeReport {
    /// `std::env::consts::OS` of the probing machine.
    pub os: String,
    /// `std::env::consts::ARCH` of the probing machine.
    pub arch: String,
    /// What the caller declared about how these binaries got here.
    pub provenance: Provenance,
    /// Adapters found under a name the resolver searches for.
    pub adapters: Vec<AdapterFinding>,
    /// Versioned variants found under names the resolver does *not* search for.
    pub variants: Vec<VariantFinding>,
}

impl ProbeReport {
    /// Adapters the resolver could actually return on this machine.
    ///
    /// A finding whose stem is not allowlisted does not count: the resolver
    /// filters every hit through the grant, so such a binary is visible to this
    /// probe and unreachable to the product.
    pub fn resolvable(&self) -> impl Iterator<Item = &AdapterFinding> {
        self.adapters.iter().filter(|found| found.allowlisted_stem)
    }

    /// Whether a dogfood run with `LEGION_DAP_DOGFOOD=1` could resolve anything.
    pub fn has_resolvable_adapter(&self) -> bool {
        self.resolvable().next().is_some()
    }
}

/// Look for every [`PROBE_NAMES`] entry on `PATH` and describe what is there.
///
/// `capture_versions` runs each hit with `--version`; skip it when the caller
/// cannot afford to execute unknown binaries.
pub fn probe(provenance: Provenance, capture_versions: bool) -> ProbeReport {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();

    let mut adapters = Vec::new();
    for name in PROBE_NAMES {
        let Some(path) = find_on_path(&dirs, name) else {
            continue;
        };
        let version = if capture_versions {
            capture_version(&path)
        } else {
            None
        };
        adapters.push(AdapterFinding {
            name: name.to_string(),
            allowlisted_stem: stem_is_allowlisted(&path),
            path,
            version,
        });
    }

    ProbeReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        provenance,
        adapters,
        variants: find_versioned_variants(&dirs),
    }
}

/// Whether the shipped default allowlist would accept this program's stem.
///
/// Mirrors `AdapterResolutionGrant::permits_program`: case-insensitive match on
/// the file stem, so the extension is not part of the name.
fn stem_is_allowlisted(program: &Path) -> bool {
    let Some(stem) = program.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    PROBE_NAMES
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(stem))
}

fn find_on_path(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    for dir in dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn find_versioned_variants(dirs: &[PathBuf]) -> Vec<VariantFinding> {
    let mut variants: Vec<VariantFinding> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(base) = versioned_variant_base(&file_name) else {
                continue;
            };
            // A directory named `lldb-dap-18` is not an adapter. Follow symlinks
            // (`is_file` does) because that is how distributions ship these.
            if !entry.path().is_file() {
                continue;
            }
            if variants
                .iter()
                .any(|existing| existing.file_name.eq_ignore_ascii_case(&file_name))
            {
                continue;
            }
            variants.push(VariantFinding {
                file_name,
                base_name: base,
                path: entry.path(),
            });
        }
    }
    variants.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    variants
}

/// `lldb-dap-18` -> `Some("lldb-dap")`; `lldb-dap` and `lldb-dap-foo` -> `None`.
///
/// The trailing segment must start with a digit, so this recognizes the
/// distribution-versioned names and not arbitrary hyphenated neighbours.
fn versioned_variant_base(file_name: &str) -> Option<String> {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);
    for name in PROBE_NAMES {
        let prefix = format!("{name}-");
        if stem.len() > prefix.len() && stem.to_ascii_lowercase().starts_with(&prefix) {
            let suffix = &stem[prefix.len()..];
            if suffix.starts_with(|c: char| c.is_ascii_digit()) {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// How long a probed binary gets to answer `--version` before it is killed.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// Run `<program> --version` and return its first output line.
///
/// stdin is `/dev/null` so an adapter that mistakes this for a DAP session
/// reads EOF instead of blocking, and the child is killed at
/// [`VERSION_TIMEOUT`] so a probe can never hang a CI job.
fn capture_version(program: &Path) -> Option<String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let started = Instant::now();
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) => {}
            // Overwhelmingly `EINTR` from a signal arriving, not a vanished
            // child. Bailing here would make a probe that spawned the adapter
            // successfully report no version at all, on the first signal the
            // OS happens to deliver. The 10s budget below is the real bound.
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
        if started.elapsed() >= VERSION_TIMEOUT {
            break true;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if timed_out {
        let _ = child.kill();
        let _ = child.wait();
        return Some("(no --version answer within 10s; killed)".to_string());
    }

    // Output is a version banner, so reading after exit cannot deadlock on a
    // full pipe buffer the way a large stream would.
    let mut text = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut text);
    }
    if text.trim().is_empty()
        && let Some(mut err) = child.stderr.take()
    {
        let _ = err.read_to_string(&mut text);
    }
    let first = text.lines().find(|line| !line.trim().is_empty())?;
    Some(first.trim().to_string())
}

/// Render a report as TOML for a CI artifact.
///
/// Through `toml` rather than by building the string by hand. The artifact
/// exists to be read back, and a hand-rolled escaper is only as complete as
/// its author remembered to be — the first one here covered `\n`, `\r`,
/// `\t`, `"` and `\` and silently emitted invalid TOML for `\0`, `\b`,
/// `\f` and control characters, any of which a `--version` banner may carry.
/// `toml` is already a dependency of this package, so the escaping question
/// simply stops being one.
///
/// Serialization of a plain data struct cannot realistically fail, but the
/// probe is a diagnostic and must not take a CI job down with it, so an
/// error is reported in-band rather than panicked.
pub fn render_toml(report: &ProbeReport) -> String {
    /// The report plus the one number that is derived rather than stored.
    ///
    /// `resolvable_adapter_count` is the headline a maintainer reads, and it is
    /// computed from policy — so a plain derive on `ProbeReport` silently drops
    /// it from the artifact, which is what happened when this function stopped
    /// building its TOML by hand. Keeping it in a view preserves `resolvable()`
    /// as the single source of truth rather than duplicating the count into a
    /// field that can drift from it.
    #[derive(serde::Serialize)]
    struct ReportView<'a> {
        os: &'a str,
        arch: &'a str,
        provenance: Provenance,
        resolvable_adapter_count: usize,
        adapters: &'a [AdapterFinding],
        variants: &'a [VariantFinding],
    }

    let view = ReportView {
        os: &report.os,
        arch: &report.arch,
        provenance: report.provenance,
        resolvable_adapter_count: report.resolvable().count(),
        adapters: &report.adapters,
        variants: &report.variants,
    };
    match toml::to_string_pretty(&view) {
        Ok(rendered) => rendered,
        Err(err) => format!("# dap-adapter-probe: report serialization failed: {err}\n"),
    }
}

pub fn render_summary(report: &ProbeReport) -> String {
    let mut out = format!(
        "dap-adapter-probe: os={} arch={} provenance={}\n",
        report.os,
        report.arch,
        report.provenance.as_str()
    );
    if report.adapters.is_empty() {
        out.push_str("  no adapter found under any of: ");
        out.push_str(&PROBE_NAMES.join(", "));
        out.push('\n');
    }
    for found in &report.adapters {
        out.push_str(&format!(
            "  found {} at {} (allowlisted_stem={}) version={}\n",
            found.name,
            found.path.display(),
            found.allowlisted_stem,
            found.version.as_deref().unwrap_or("(none)")
        ));
    }
    for variant in &report.variants {
        out.push_str(&format!(
            "  variant {} at {} — not resolvable (resolver searches `{}` exactly)\n",
            variant.file_name,
            variant.path.display(),
            variant.base_name
        ));
    }
    out.push_str(&format!(
        "  resolvable_adapter_count={}\n",
        report.resolvable().count()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_round_trips_and_rejects_garbage() {
        assert_eq!(Provenance::parse("shipped"), Ok(Provenance::Shipped));
        assert_eq!(Provenance::parse(" Installed "), Ok(Provenance::Installed));
        assert_eq!(Provenance::parse(""), Ok(Provenance::Unknown));
        assert!(Provenance::parse("probably").is_err());
        assert_eq!(Provenance::Shipped.as_str(), "shipped");
    }

    #[test]
    fn versioned_variant_base_recognizes_distro_names_only() {
        assert_eq!(
            versioned_variant_base("lldb-dap-18"),
            Some("lldb-dap".to_string())
        );
        assert_eq!(
            versioned_variant_base("lldb-vscode-14"),
            Some("lldb-vscode".to_string())
        );
        // Exact names are found by the normal search, not reported as variants.
        assert_eq!(versioned_variant_base("lldb-dap"), None);
        // A hyphenated neighbour is not a version.
        assert_eq!(versioned_variant_base("lldb-dap-helper"), None);
        assert_eq!(versioned_variant_base("clang-18"), None);
    }

    #[test]
    fn allowlisted_stem_ignores_extension_and_case() {
        assert!(stem_is_allowlisted(Path::new("CodeLLDB.exe")));
        assert!(stem_is_allowlisted(Path::new("lldb-dap")));
        // The gap this probe exists to make visible.
        assert!(!stem_is_allowlisted(Path::new("lldb-dap-18")));
        assert!(!stem_is_allowlisted(Path::new("sh")));
    }

    #[test]
    fn render_toml_escapes_windows_paths() {
        let report = ProbeReport {
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
            provenance: Provenance::Shipped,
            adapters: vec![AdapterFinding {
                name: "lldb-dap".to_string(),
                path: PathBuf::from(r"C:\Program Files\LLVM\bin\lldb-dap.exe"),
                allowlisted_stem: true,
                version: Some(r#"lldb version 18.1.8 "quoted""#.to_string()),
            }],
            variants: Vec::new(),
        };
        let rendered = render_toml(&report);
        assert!(rendered.contains("resolvable_adapter_count = 1"));
        // Parses as TOML, which is the whole point of writing an artifact.
        let parsed: toml::Value = rendered.parse().expect("probe report must be valid TOML");
        assert_eq!(parsed["provenance"].as_str(), Some("shipped"));
        // The property, not the encoding: whichever string form the serializer
        // picks, a Windows path and an embedded quote must come back
        // byte-identical. The earlier assertions pinned one hand-rolled escape
        // spelling, which is precisely what makes a serializer unreplaceable.
        assert_eq!(
            parsed["adapters"][0]["path"].as_str(),
            Some(r"C:\Program Files\LLVM\bin\lldb-dap.exe")
        );
        assert_eq!(
            parsed["adapters"][0]["version"].as_str(),
            Some(r#"lldb version 18.1.8 "quoted""#)
        );
    }

    #[test]
    fn a_present_but_unallowlisted_adapter_is_not_counted_as_resolvable() {
        let report = ProbeReport {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            provenance: Provenance::Installed,
            adapters: vec![AdapterFinding {
                // Contrived: the searcher only looks for exact names, but the
                // report type can hold anything, and `resolvable()` must key on
                // policy rather than on presence.
                name: "lldb-dap".to_string(),
                path: PathBuf::from("/usr/bin/lldb-dap-18"),
                allowlisted_stem: false,
                version: None,
            }],
            variants: Vec::new(),
        };
        assert!(!report.has_resolvable_adapter());
        assert!(render_toml(&report).contains("resolvable_adapter_count = 0"));
    }

    /// Drift guard: `xtask` cannot import `legion-debug`, so the probe's name
    /// list is a copy. If the resolver learns a new alias and this list does
    /// not, the probe will report "nothing here" on a machine that can in fact
    /// resolve an adapter — which is the exact failure mode this whole command
    /// exists to eliminate. Read the resolver's source and compare.
    #[test]
    fn probe_names_match_the_resolver_alias_list() {
        let resolver = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("crates/legion-debug/src/adapter_resolve.rs");
        let source = std::fs::read_to_string(&resolver)
            .unwrap_or_else(|err| panic!("read {}: {err}", resolver.display()));

        let marker = "for alias in [";
        let start = source
            .find(marker)
            .expect("resolver must still declare its aliases as `for alias in [...]`");
        let rest = &source[start + marker.len()..];
        let end = rest.find(']').expect("unterminated alias array");
        let aliases: Vec<String> = rest[..end]
            .split(',')
            .filter_map(|piece| {
                let piece = piece.trim().trim_matches('"').trim();
                (!piece.is_empty()).then(|| piece.to_string())
            })
            .collect();

        assert_eq!(
            aliases,
            PROBE_NAMES.to_vec(),
            "PROBE_NAMES has drifted from the resolver aliases in {}; \
             update xtask::dap_adapter_probe::PROBE_NAMES to match",
            resolver.display()
        );
    }
}
