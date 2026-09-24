//! The intent-reachability gate: a capability nobody can reach is not shipped.
//!
//! `CommandDispatchIntent` is the whole vocabulary of things the product can be
//! asked to do. An intent reaches a user only if some gesture produces it — a
//! rendered control, a keybinding, a `:` command, a Vim mapping, or the command
//! palette. Nothing checked that, and on 2026-08-17 four separate capabilities
//! turned out to be complete, tested, and unreachable:
//!
//! * clicking a file in the explorer dispatched select-and-reveal and never
//!   opened the buffer;
//! * session state had no default path, so `save_session_state` returned
//!   immediately and every restart lost the layout;
//! * persisted dock splitter fractions were reloaded and read by no renderer;
//! * multi-cursor had intents, app handling and eight passing tests, and no
//!   `DesktopAction`, no bridge translation and no keybinding.
//!
//! Each was found by a person running the app, one at a time. Every suite
//! stayed green throughout, because in every case the app layer was correct and
//! the route to it did not exist. This gate closes that class: an intent with
//! no route fails the build unless it is allowlisted with a written reason.
//!
//! The check is deliberately textual. Tracing real reachability would mean
//! following control flow through the renderer, the palette's string-keyed
//! command table, and the Vim parser — and a gate nobody can predict is a gate
//! people route around. "Some route-carrying file names this variant" is
//! coarse, but it is exactly the property that was missing in all four cases,
//! and it cannot be satisfied by accident.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

/// Configuration: where intents are declared, which files carry routes, and
/// which variants are deliberately unreachable.
#[derive(Debug, Clone, Deserialize)]
pub struct IntentReachabilityConfig {
    /// File declaring the intent enum.
    pub intent_source: String,
    /// Name of the enum to read variants from.
    pub intent_enum: String,
    /// Files where a user gesture may produce an intent.
    ///
    /// A directory is read one level deep, which covers the renderer's `view/`
    /// submodules without the config having to list every file.
    pub route_sources: Vec<String>,
    /// Variants that are knowingly unreachable, each with a reason.
    #[serde(default)]
    pub allowed: Vec<AllowedIntent>,
}

/// One deliberately unreachable intent.
#[derive(Debug, Clone, Deserialize)]
pub struct AllowedIntent {
    /// Variant name.
    pub intent: String,
    /// Why it has no route. Required and required to be non-empty: an
    /// allowlist without reasons becomes a list nobody remembers the case for.
    pub reason: String,
}

/// One intent with no route from any user gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreachableIntent {
    /// Variant name.
    pub intent: String,
}

/// Why the gate could not reach a verdict.
#[derive(Debug)]
pub enum GateError {
    /// The intent source could not be read, or declared no such enum.
    IntentSource(String),
    /// An allowlist entry carries no reason.
    ReasonMissing(String),
    /// Allowlist entries that are now reachable, or name nothing.
    StaleAllowlist(Vec<String>),
}

impl IntentReachabilityConfig {
    /// Read the config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let body = std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        toml::from_str(&body).map_err(|err| format!("cannot parse {}: {err}", path.display()))
    }
}

/// Check every intent for a route.
///
/// `Ok(Ok(count))` means every one of `count` intents is reachable or
/// allowlisted. `Ok(Err(list))` names the ones that are neither.
pub fn run_intent_reachability(
    workspace_root: &Path,
    config: &IntentReachabilityConfig,
) -> Result<Result<usize, Vec<UnreachableIntent>>, GateError> {
    for allowed in &config.allowed {
        if allowed.reason.trim().is_empty() {
            return Err(GateError::ReasonMissing(allowed.intent.clone()));
        }
    }

    let source = std::fs::read_to_string(workspace_root.join(&config.intent_source))
        .map_err(|err| GateError::IntentSource(format!("{}: {err}", config.intent_source)))?;
    let variants = enum_variants(&source, &config.intent_enum).ok_or_else(|| {
        GateError::IntentSource(format!(
            "{} does not declare `enum {}`",
            config.intent_source, config.intent_enum
        ))
    })?;

    let routes = read_route_sources(workspace_root, &config.route_sources);
    let allowed: BTreeSet<&str> = config
        .allowed
        .iter()
        .map(|entry| entry.intent.as_str())
        .collect();

    let mut unreachable = Vec::new();
    let mut reachable = BTreeSet::new();
    for variant in &variants {
        if routes.contains(&format!("{}::{variant}", config.intent_enum)) {
            reachable.insert(variant.clone());
        } else if !allowed.contains(variant.as_str()) {
            unreachable.push(UnreachableIntent {
                intent: variant.clone(),
            });
        }
    }

    // An allowlist that outlives its reason is worse than none: it hides the
    // very thing the gate exists to surface. Entries that became reachable, or
    // that no longer name a variant, are an error in their own right.
    let stale: Vec<String> = config
        .allowed
        .iter()
        .filter(|entry| reachable.contains(&entry.intent) || !variants.contains(&entry.intent))
        .map(|entry| entry.intent.clone())
        .collect();
    if !stale.is_empty() {
        return Err(GateError::StaleAllowlist(stale));
    }

    if unreachable.is_empty() {
        Ok(Ok(variants.len()))
    } else {
        Ok(Err(unreachable))
    }
}

/// Variant names declared by `enum name` in `source`.
///
/// Reads a brace-balanced body rather than stopping at the first `}`, so a
/// variant carrying a struct body cannot truncate the list — which would
/// silently shrink what the gate checks, in the direction of passing.
fn enum_variants(source: &str, name: &str) -> Option<Vec<String>> {
    let marker = format!("enum {name} {{");
    let start = source.find(&marker)? + marker.len();
    let mut depth = 1usize;
    let mut end = source.len();
    for (index, character) in source[start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + index;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &source[start..end];

    let mut variants = Vec::new();
    let mut depth = 0i64;
    for line in body.lines() {
        let trimmed = line.trim();
        // Only lines at the enum's own nesting level declare variants; deeper
        // ones are the fields of a struct-bodied variant.
        if depth == 0 && trimmed.starts_with(|c: char| c.is_ascii_uppercase()) {
            let ident: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() {
                variants.push(ident);
            }
        }
        depth += trimmed.matches('{').count() as i64;
        depth -= trimmed.matches('}').count() as i64;
        depth = depth.max(0);
    }
    Some(variants)
}

/// Concatenate every route source, so one search covers them all.
fn read_route_sources(workspace_root: &Path, sources: &[String]) -> String {
    let mut blob = String::new();
    for source in sources {
        let path = workspace_root.join(source);
        if path.is_dir() {
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "rs")
                    && let Ok(body) = std::fs::read_to_string(entry.path())
                {
                    blob.push_str(&body);
                    blob.push('\n');
                }
            }
        } else if let Ok(body) = std::fs::read_to_string(&path) {
            blob.push_str(&body);
            blob.push('\n');
        }
    }
    blob
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "
pub enum CommandDispatchIntent {
    /// Doc comment.
    Quit,
    Save {
        buffer_id: BufferId,
    },
    OpenPath { path: String },
}
";

    #[test]
    fn variants_survive_a_struct_bodied_member() {
        // A reader that stopped at the first `}` would report only `Quit` and
        // silently shrink what the gate checks, in the direction of passing.
        let variants = enum_variants(SAMPLE, "CommandDispatchIntent").expect("enum found");
        assert_eq!(variants, vec!["Quit", "Save", "OpenPath"]);
    }

    #[test]
    fn a_field_is_not_mistaken_for_a_variant() {
        let variants = enum_variants(SAMPLE, "CommandDispatchIntent").expect("enum found");
        assert!(
            !variants.iter().any(|variant| variant == "buffer_id"),
            "lowercase fields are not variants: {variants:?}"
        );
    }

    #[test]
    fn a_missing_enum_is_reported_rather_than_read_as_empty() {
        // Returning an empty list here would make the gate pass loudly while
        // checking nothing at all.
        assert!(enum_variants(SAMPLE, "SomeOtherEnum").is_none());
    }
}
