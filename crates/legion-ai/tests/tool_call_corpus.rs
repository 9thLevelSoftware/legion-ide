//! Corpus test: the tolerant tool-call normalizer against the frozen
//! SmallCode-derived vector set.
//!
//! The corpus is the acceptance gate for ADR-0049's tool-call recovery port.
//! Every vector either names the exact calls that must be recovered, or is
//! marked `unrepairable` and must produce nothing at all. A vector that
//! fabricates a call is a worse failure than one that recovers nothing, so
//! both directions are asserted.
//!
//! Vectors are derived from SmallCode
//! (<https://github.com/Doorman11991/smallcode>, MIT) — see
//! `THIRD_PARTY_NOTICES.md`.

use std::path::PathBuf;

use legion_ai::normalize::{ExtractionInput, extract_tool_calls, normalize_alias};
use serde_json::Value;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/smallcode_vectors/tool_call_vectors.jsonl")
}

fn load_vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(corpus_path()).expect("tool-call vector corpus is readable");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("each corpus line is valid JSON"))
        .filter(|value| value.get("name").is_some())
        .collect()
}

/// Outcome of running one vector through the normalizer.
struct Outcome {
    calls: Vec<Value>,
    residual: Option<String>,
}

fn run_vector(vector: &Value) -> Outcome {
    let raw = &vector["raw_input"];
    let known: Vec<String> = vector
        .get("known_tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    // Alias-shaped vectors exercise name/argument remapping directly rather
    // than extraction from prose.
    if let Some(tool_call) = raw.get("tool_call") {
        let name = tool_call["name"].as_str().unwrap_or_default();
        let arguments = match tool_call.get("arguments") {
            Some(Value::String(text)) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
            Some(value) => value.clone(),
            None => match tool_call.get("arguments_raw").and_then(Value::as_str) {
                Some(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
                None => Value::Null,
            },
        };
        let (name, arguments) = normalize_alias(name, &arguments);
        return Outcome {
            calls: vec![serde_json::json!({"name": name, "arguments": arguments})],
            residual: None,
        };
    }

    let (content, reasoning, has_existing) = match raw {
        Value::String(text) => (text.clone(), None, false),
        Value::Object(object) => (
            object
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            object
                .get("reasoning_content")
                .and_then(Value::as_str)
                .map(str::to_string),
            object
                .get("existing_tool_calls")
                .and_then(Value::as_array)
                .map(|calls| !calls.is_empty())
                .unwrap_or(false),
        ),
        other => panic!("unsupported raw_input shape: {other}"),
    };

    let extraction = extract_tool_calls(&ExtractionInput {
        content: &content,
        reasoning_content: reasoning.as_deref(),
        has_existing_tool_calls: has_existing,
        known_tools: &known,
    });

    Outcome {
        calls: extraction
            .calls
            .iter()
            .map(|call| serde_json::json!({"name": call.name, "arguments": call.arguments}))
            .collect(),
        residual: Some(extraction.residual_content),
    }
}

#[test]
fn every_corpus_vector_is_recovered_or_safely_rejected() {
    let vectors = load_vectors();
    assert!(
        vectors.len() >= 50,
        "corpus should carry the full extracted vector set, found {}",
        vectors.len()
    );

    let mut failures = Vec::new();
    for vector in &vectors {
        let name = vector["name"].as_str().unwrap_or("<unnamed>");
        let outcome = run_vector(vector);

        if vector.get("expected_outcome").and_then(Value::as_str) == Some("unrepairable") {
            if !outcome.calls.is_empty() {
                failures.push(format!(
                    "{name}: expected no calls (unrepairable) but recovered {}",
                    serde_json::to_string(&outcome.calls).unwrap_or_default()
                ));
            }
            continue;
        }

        let Some(expected) = vector.get("expected") else {
            continue;
        };

        if let Some(expected_calls) = expected.get("calls").and_then(Value::as_array) {
            let actual: Vec<Value> = outcome.calls.clone();
            let expected_normalized: Vec<Value> = expected_calls
                .iter()
                .map(|call| {
                    serde_json::json!({
                        "name": call["name"],
                        "arguments": call.get("arguments").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect();
            if actual != expected_normalized {
                failures.push(format!(
                    "{name}: calls mismatch\n     expected: {}\n     actual:   {}",
                    serde_json::to_string(&expected_normalized).unwrap_or_default(),
                    serde_json::to_string(&actual).unwrap_or_default()
                ));
                continue;
            }
        }

        if let (Some(expected_residual), Some(actual_residual)) = (
            expected.get("residual_content").and_then(Value::as_str),
            outcome.residual.as_deref(),
        ) && expected_residual != actual_residual
        {
            failures.push(format!(
                "{name}: residual_content mismatch\n     expected: {expected_residual:?}\n     actual:   {actual_residual:?}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} corpus vectors failed:\n  - {}",
        failures.len(),
        vectors.len(),
        failures.join("\n  - ")
    );
}

#[test]
fn recovery_never_panics_on_adversarial_input() {
    // Structural fuzzing: truncations and delimiter soup must fail closed
    // rather than crash the provider thread.
    let seeds = [
        "<tool_call>",
        "</tool_call>",
        "<tool_call>{",
        "<tool_call>{\"name\":",
        "```json",
        "```json\n{",
        "<|tool_call_start|>",
        "<|tool_call_start|>[",
        "<|tool_call_start|>[f(",
        "<|tool_call_start|>[f(a=",
        "<|tool_call_start|>[f(a='",
        "<|tool_call_start|>[f(a='\\",
        "{\"function\":{}}",
        "{\"function\":{\"name\":\"\"}}",
        "[[[[[[",
        "{{{{{{",
        "\"\"\"",
        "\\",
    ];
    for seed in seeds {
        // Every prefix of every seed, to catch mid-token truncation.
        for cut in 0..=seed.len() {
            if !seed.is_char_boundary(cut) {
                continue;
            }
            let input = &seed[..cut];
            let known = vec!["bash".to_string(), "read_file".to_string()];
            let out = extract_tool_calls(&ExtractionInput {
                content: input,
                reasoning_content: None,
                has_existing_tool_calls: false,
                known_tools: &known,
            });
            for call in out.calls {
                assert!(
                    !call.name.is_empty(),
                    "recovered call from {input:?} must never carry an empty name"
                );
            }
        }
    }
}

#[test]
fn unicode_input_is_handled_on_character_boundaries() {
    let known = vec!["write_file".to_string()];
    for content in [
        "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"content\":\"héllo 🌍\"}}</tool_call>",
        "日本語のテキスト<tool_call>{\"name\":\"write_file\",\"arguments\":{\"content\":\"café\"}}</tool_call>",
        "<|tool_call_start|>[write_file(path='é.txt', content='🌍')]<|tool_call_end|>",
    ] {
        let out = extract_tool_calls(&ExtractionInput {
            content,
            reasoning_content: None,
            has_existing_tool_calls: false,
            known_tools: &known,
        });
        assert_eq!(out.calls.len(), 1, "unicode content: {content}");
    }
}
