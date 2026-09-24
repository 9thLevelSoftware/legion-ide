//! Measures the secret ruleset's false-positive rate against a real directory tree.
//!
//! Run it over this repository to reproduce the figures in
//! `plans/evidence/production/2026-08-17-secret-scanning-ruleset.md`:
//!
//! ```text
//! cargo run --release -p legion-security --example corpus_scan -- .
//! ```
//!
//! This exists because the rule table's precision cannot be established by
//! choosing negative fixtures by inspection. The fixture suite passed while the
//! entropy rule was producing 5042 false positives on this tree; only scanning
//! the tree found that. Re-run this after any change to the entropy heuristic or
//! its shape predicates.
//!
//! # This tool prints matched text, and nothing else in the crate does
//!
//! [`legion_security::secrets::SecretFinding`] deliberately carries only a rule
//! id, a confidence, a severity, and a byte span — never the matched bytes —
//! because findings are copied into audit records. This example re-slices the
//! source text by that span to show what fired, which is the opposite choice.
//! That is correct for a developer triaging false positives against a local
//! checkout and wrong for anything else: do not route its output into a log, an
//! artifact, an audit record, or CI output.

use std::{collections::BTreeMap, fs, path::Path};

use legion_security::secrets::{ScanPosture, SecretConfidence, scan_text_for_secrets};

fn main() {
    let root = std::env::args().nth(1).expect("usage: corpus_scan <root>");
    let mut by_rule: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut files = 0usize;
    let mut bytes = 0usize;
    walk(Path::new(&root), &mut |path: &Path| {
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        files += 1;
        bytes += text.len();
        let report = scan_text_for_secrets(&text);
        for finding in &report.findings {
            let excerpt = text
                .get(finding.span.start..finding.span.end)
                .unwrap_or("<non-utf8-boundary>");
            let line = text[..finding.span.start].matches('\n').count() + 1;
            let posture = if finding.confidence == SecretConfidence::Heuristic {
                "EGRESS-ONLY"
            } else {
                "BOTH-POSTURES"
            };
            by_rule
                .entry(format!("{} [{posture}]", finding.rule_id.stable_id()))
                .or_default()
                .push(format!("{}:{line}: {excerpt}", path.display()));
        }
        let _ = report.requires_redaction(ScanPosture::DisplayPrecision);
    });
    println!("FILES={files} BYTES={bytes}");
    let mut total = 0usize;
    for (rule, hits) in &by_rule {
        total += hits.len();
        println!("\n=== {rule}: {} hit(s) ===", hits.len());
        for hit in hits.iter().take(40) {
            println!("  {hit}");
        }
        if hits.len() > 40 {
            println!("  ... {} more", hits.len() - 40);
        }
    }
    println!("\nTOTAL_FINDINGS={total}");
}

fn walk(dir: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            "target" | ".git" | "node_modules" | ".claude" | ".worktrees"
        ) {
            continue;
        }
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}
