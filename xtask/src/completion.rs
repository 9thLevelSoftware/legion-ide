pub mod artifact_files;
pub mod candidate;
pub mod hash;
pub mod identity_receipts;
pub mod links;
pub mod outcomes;
pub mod ratification;
pub mod run_artifacts;
pub mod schema;
pub mod structure;

/// Returns whether a tuple is eligible for product evidence consideration.
///
/// This is an eligibility primitive only. It does not establish evidence
/// authenticity and does not produce a release verdict.
pub fn qualifies_as_product_evidence(
    layer: &str,
    input_route: &str,
    result: &str,
    required_dependency_substituted: bool,
) -> bool {
    layer == "product"
        && input_route == "native-input"
        && result == "passed"
        && !required_dependency_substituted
}

/// Validate the canonical completion registers and selected evidence.
pub fn validate_completion(
    root: &std::path::Path,
    candidate: &str,
    release: bool,
) -> Result<Vec<String>, String> {
    crate::completion_command::validate_completion(root, candidate, release)
}
