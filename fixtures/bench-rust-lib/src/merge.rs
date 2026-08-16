//! Layered configuration merging.

use std::collections::BTreeMap;

/// Merge configuration layers into one map.
///
/// Layers are ordered from lowest precedence to highest, so a key present in
/// a later layer replaces the same key from an earlier one. Keys absent from
/// the later layers keep their earlier value.
pub fn merge_layers(layers: &[BTreeMap<String, String>]) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    for layer in layers {
        for (key, value) in layer {
            merged.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    merged
}

/// Keys whose value differs between two layers.
///
/// A key present in only one of them counts as differing. The result is
/// sorted and free of duplicates.
pub fn differing_keys(
    left: &BTreeMap<String, String>,
    right: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for (key, value) in left {
        if right.get(key) != Some(value) {
            keys.push(key.clone());
        }
    }
    for key in right.keys() {
        if !left.contains_key(key) {
            keys.push(key.clone());
        }
    }
    keys.sort();
    keys.dedup();
    keys
}
