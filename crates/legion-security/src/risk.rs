//! Deterministic risk-classification helpers for proposal auto-approval.

/// Coarse risk level emitted by deterministic risk rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe to consider for auto-approval.
    Low,
    /// Requires manual review; must not be auto-approved.
    Elevated,
    /// Must be denied outright.
    Deny,
}

/// Returns true when `path` names a dependency manifest or lockfile for a
/// Legion-supported workspace ecosystem.
///
/// Dependency/manifest edits change the trust surface of a project, so they must
/// not be silently classified as low-risk. The comparison is performed on the
/// lowercased file name so both `/` and `\` path separators are handled.
pub fn is_dependency_or_lockfile(path: &str) -> bool {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();

    matches!(
        name.as_str(),
        // Rust
        "cargo.toml"
            | "cargo.lock"
            // JavaScript / TypeScript
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "deno.json"
            | "deno.jsonc"
            | "deno.lock"
            | "bun.lockb"
            // Go
            | "go.mod"
            | "go.sum"
            // Python
            | "pyproject.toml"
            | "requirements.txt"
            | "pipfile"
            | "pipfile.lock"
            | "poetry.lock"
            // Ruby
            | "gemfile"
            | "gemfile.lock"
            // PHP
            | "composer.json"
            | "composer.lock"
            // JVM
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.lockfile"
    )
}

/// Evaluates the path-scope risk for a proposal.
///
/// When `workspace_root` is absent, path containment cannot be evaluated, so the
/// proposal must be treated as non-low (deny for auto-approval purposes) rather
/// than emitting an informational allow finding. Missing scope metadata is never
/// low-risk evidence.
pub fn path_scope_risk(workspace_root: Option<&str>, target_paths: &[String]) -> RiskLevel {
    let Some(root) = workspace_root.map(str::trim).filter(|root| !root.is_empty()) else {
        // Scope could not be evaluated; refuse to classify as low-risk.
        return RiskLevel::Deny;
    };

    let normalized_root = root.replace('\\', "/");
    let escapes_scope = target_paths.iter().any(|path| {
        let normalized = path.replace('\\', "/");
        !normalized.starts_with(&normalized_root) || normalized.contains("..")
    });

    if escapes_scope {
        RiskLevel::Elevated
    } else {
        RiskLevel::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_lockfiles_are_detected() {
        assert!(is_dependency_or_lockfile("Cargo.toml"));
        assert!(is_dependency_or_lockfile("crates/foo/Cargo.toml"));
        assert!(is_dependency_or_lockfile("frontend\\package.json"));
        assert!(is_dependency_or_lockfile("Cargo.lock"));
        assert!(is_dependency_or_lockfile("pnpm-lock.yaml"));
        assert!(!is_dependency_or_lockfile("src/main.rs"));
        assert!(!is_dependency_or_lockfile("README.md"));
    }

    #[test]
    fn missing_workspace_root_is_not_low_risk() {
        assert_eq!(
            path_scope_risk(None, &["src/main.rs".to_string()]),
            RiskLevel::Deny
        );
        assert_eq!(
            path_scope_risk(Some("   "), &["src/main.rs".to_string()]),
            RiskLevel::Deny
        );
    }

    #[test]
    fn contained_paths_are_low_risk() {
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/repo/src/main.rs".to_string()]),
            RiskLevel::Low
        );
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/repo/../etc/passwd".to_string()]),
            RiskLevel::Elevated
        );
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/outside/file".to_string()]),
            RiskLevel::Elevated
        );
    }
}
