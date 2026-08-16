//! Tests for layered configuration merging.

use std::collections::BTreeMap;

use miniconf::merge::{differing_keys, merge_layers};

fn layer(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn a_later_layer_overrides_an_earlier_one() {
    let base = layer(&[("host", "localhost"), ("port", "80")]);
    let override_layer = layer(&[("port", "8080")]);
    let merged = merge_layers(&[base, override_layer]);
    assert_eq!(merged.get("host").map(String::as_str), Some("localhost"));
    assert_eq!(
        merged.get("port").map(String::as_str),
        Some("8080"),
        "the later layer has higher precedence"
    );
}

#[test]
fn the_last_of_three_layers_wins() {
    let merged = merge_layers(&[
        layer(&[("mode", "a")]),
        layer(&[("mode", "b")]),
        layer(&[("mode", "c")]),
    ]);
    assert_eq!(merged.get("mode").map(String::as_str), Some("c"));
}

#[test]
fn keys_absent_later_keep_their_earlier_value() {
    let merged = merge_layers(&[layer(&[("only", "kept")]), layer(&[("other", "x")])]);
    assert_eq!(merged.get("only").map(String::as_str), Some("kept"));
}

#[test]
fn differing_keys_reports_both_sides() {
    let left = layer(&[("same", "1"), ("changed", "a"), ("left_only", "l")]);
    let right = layer(&[("same", "1"), ("changed", "b"), ("right_only", "r")]);
    assert_eq!(
        differing_keys(&left, &right),
        vec![
            "changed".to_string(),
            "left_only".to_string(),
            "right_only".to_string()
        ]
    );
}
