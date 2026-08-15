//! Corpus test: exact-match patch resolution against the frozen
//! SmallCode-derived vector set.
//!
//! Each vector either names the file content an edit must produce, or is
//! marked with the refusal it must produce. Both directions matter: an edit
//! that applies when it should have been refused is how a patch lands in the
//! wrong place, which is worse than one that refuses when it could have
//! applied.
//!
//! Vectors are derived from SmallCode
//! (<https://github.com/Doorman11991/smallcode>, MIT) — see
//! `THIRD_PARTY_NOTICES.md`.

use std::path::PathBuf;

use legion_ai::patch::{PatchResolution, apply_edit_from_arguments, parse_edit_blocks};
use serde_json::Value;

fn load_vectors() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/smallcode_vectors/patch_vectors.jsonl");
    let text = std::fs::read_to_string(path).expect("patch vector corpus is readable");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("each corpus line is valid JSON"))
        .filter(|value| value.get("name").is_some())
        .collect()
}

/// Map a corpus `expected_outcome` label onto a resolution.
fn matches_expected_outcome(resolution: &PatchResolution, expected: &str) -> bool {
    match expected {
        "no_match" => matches!(resolution, PatchResolution::NoMatch(_)),
        "ambiguous" => matches!(resolution, PatchResolution::Ambiguous { .. }),
        "validation_error" => matches!(resolution, PatchResolution::ValidationError { .. }),
        // Block-format vectors: "unrepairable" means no edit is extracted.
        "unrepairable" => false,
        other => panic!("unknown expected_outcome: {other}"),
    }
}

#[test]
fn every_patch_vector_applies_exactly_or_refuses() {
    let vectors = load_vectors();
    assert!(
        vectors.len() >= 15,
        "corpus should carry the full extracted vector set, found {}",
        vectors.len()
    );

    let mut failures = Vec::new();
    for vector in &vectors {
        let name = vector["name"].as_str().unwrap_or("<unnamed>");
        let raw = &vector["raw_input"];
        let expected_outcome = vector.get("expected_outcome").and_then(Value::as_str);

        // Block-format vectors carry prose; apply-format vectors carry an object.
        let Some(object) = raw.as_object() else {
            let text = raw.as_str().unwrap_or_default();
            let blocks = parse_edit_blocks(text);
            if expected_outcome == Some("unrepairable") {
                if !blocks.is_empty() {
                    failures.push(format!(
                        "{name}: expected no edits (unrepairable) but parsed {}",
                        blocks.len()
                    ));
                }
                continue;
            }
            let expected_edits = vector["expected"]["edits"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if blocks.len() != expected_edits.len() {
                failures.push(format!(
                    "{name}: expected {} edit(s), parsed {}",
                    expected_edits.len(),
                    blocks.len()
                ));
                continue;
            }
            for (parsed, expected) in blocks.iter().zip(expected_edits.iter()) {
                if parsed.path != expected["path"].as_str().unwrap_or_default()
                    || parsed.old_str != expected["old_str"].as_str().unwrap_or_default()
                    || parsed.new_str != expected["new_str"].as_str().unwrap_or_default()
                {
                    failures.push(format!(
                        "{name}: edit mismatch\n     expected: {}\n     actual:   path={:?} old={:?} new={:?}",
                        serde_json::to_string(expected).unwrap_or_default(),
                        parsed.path,
                        parsed.old_str,
                        parsed.new_str
                    ));
                }
            }
            continue;
        };

        let file_content = object
            .get("file_content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let arguments = Value::Object(
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "file_content")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        let resolution = apply_edit_from_arguments(file_content, &arguments);

        if let Some(expected) = expected_outcome {
            if !matches_expected_outcome(&resolution, expected) {
                failures.push(format!("{name}: expected {expected}, got {resolution:?}"));
            }
            continue;
        }

        let expected_content = vector["expected"]["result_content"]
            .as_str()
            .unwrap_or_default();
        match &resolution {
            PatchResolution::Applied { content, .. } if content == expected_content => {}
            PatchResolution::Applied { content, .. } => failures.push(format!(
                "{name}: content mismatch\n     expected: {expected_content:?}\n     actual:   {content:?}"
            )),
            other => failures.push(format!("{name}: expected an applied edit, got {other:?}")),
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} patch vectors failed:\n  - {}",
        failures.len(),
        vectors.len(),
        failures.join("\n  - ")
    );
}

#[test]
fn resolution_never_panics_on_adversarial_input() {
    let long_content = "x".repeat(4096);
    let long_fragment = "x".repeat(5000);
    let contents = ["", "a", "a\nb\n", "\r\n\r\n", "é🌍", long_content.as_str()];
    let fragments = [
        "",
        "a",
        "\n",
        "\r\n",
        "é",
        long_fragment.as_str(),
        "<<<<<<< SEARCH",
    ];
    for content in contents {
        for fragment in fragments {
            let _ = legion_ai::patch::apply_edit(content, fragment, "replacement");
        }
    }

    // Every prefix of a block-format seed, to catch mid-marker truncation.
    let seed = "src/a.rs\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n```diff\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n```";
    for cut in 0..=seed.len() {
        if seed.is_char_boundary(cut) {
            let _ = parse_edit_blocks(&seed[..cut]);
        }
    }
}

/// A refusal has to tell the model where to look, or it cannot retry — it can
/// only escalate to rewriting the file, which is the outcome this layer exists
/// to prevent.
#[test]
fn a_refusal_points_at_the_nearest_candidate() {
    let content = "fn alpha() {}\nfn beta() {}\nfn gamma() {}\n";
    match legion_ai::patch::apply_edit(content, "fn beta( ) {}", "fn beta(x: u8) {}") {
        PatchResolution::NoMatch(diagnostic) => {
            assert_eq!(
                diagnostic.nearest_line,
                Some(2),
                "should point at the line the model probably meant"
            );
            assert!(diagnostic.similarity_percent > 50);
            assert!(
                diagnostic.message.contains("Re-read"),
                "should tell the model what to do next: {}",
                diagnostic.message
            );
        }
        other => panic!("expected NoMatch, got {other:?}"),
    }
}
