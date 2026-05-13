use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::Path,
    process,
};

use cargo_metadata::{Metadata, MetadataCommand};
use clap::{Parser, Subcommand};

const DEFAULT_POLICY_PATH: &str = "plans/dependency-policy.md";
const DEFAULT_PROTOCOL_PATH: &str = "crates/devil-protocol/src/lib.rs";

#[derive(Parser)]
#[command(author, version, about = "Repository maintenance and validation tasks")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate workspace crate dependencies against architecture policy
    CheckDeps {
        /// Path to the markdown policy document.
        #[arg(long, default_value = DEFAULT_POLICY_PATH)]
        policy: String,
    },
}

fn main() {
    let args = Args::parse();

    let code = match args.command {
        Commands::CheckDeps { policy } => {
            if let Err(err) = run_check_deps(&policy) {
                eprintln!("dependency check failed: {err}");
                1
            } else {
                println!("dependency policy checks passed");
                0
            }
        }
    };

    process::exit(code);
}

fn run_check_deps(policy_path: &str) -> Result<(), String> {
    let workspace_root =
        env::current_dir().map_err(|err| format!("unable to resolve current directory: {err}"))?;

    let policy_text = fs::read_to_string(policy_path)
        .map_err(|err| format!("unable to read policy at `{policy_path}`: {err}"))?;
    let policy = Policy::from_markdown(&policy_text)
        .map_err(|err| format!("unable to parse policy: {err}"))?;

    let metadata = load_workspace_metadata(&workspace_root)?;
    let packages = workspace_packages(&metadata);
    let violations = validate_dependency_policy(&packages, &policy);

    let protocol_violations = validate_protocol_contracts(
        &workspace_root.join(DEFAULT_PROTOCOL_PATH),
        policy.protocol_symbols(),
    )?;

    let mut all = violations;
    all.extend(protocol_violations);

    if !all.is_empty() {
        let mut output = String::new();
        output.push_str("dependency policy violations:\n");
        for item in all {
            output.push_str(&format!("- {item}\n"));
        }
        return Err(output);
    }

    Ok(())
}

fn load_workspace_metadata(workspace_root: &Path) -> Result<Metadata, String> {
    MetadataCommand::new()
        .current_dir(workspace_root)
        .manifest_path(workspace_root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .map_err(|err| format!("cargo metadata failed: {err}"))
}

fn workspace_packages(metadata: &Metadata) -> HashMap<String, HashSet<String>> {
    let internal = metadata
        .packages
        .iter()
        .filter(|package| package.source.is_none())
        .map(|package| package.name.clone())
        .collect::<HashSet<_>>();

    metadata
        .packages
        .iter()
        .filter(|package| internal.contains(&package.name))
        .map(|package| {
            let package_deps = package
                .dependencies
                .iter()
                .filter(|dep| dep.kind == cargo_metadata::DependencyKind::Normal)
                .filter(|dep| internal.contains(&dep.name))
                .map(|dep| dep.name.clone())
                .collect::<HashSet<_>>();

            (package.name.clone(), package_deps)
        })
        .collect()
}

fn validate_dependency_policy(
    packages: &HashMap<String, HashSet<String>>,
    policy: &Policy,
) -> Vec<String> {
    let mut issues = Vec::new();

    // Structural rule set defined by the policy.
    let forbidden_pairs = policy.forbidden_pairs();

    for (source, deps) in packages {
        let Some(allowed_deps) = policy.allowed_internal(source) else {
            continue;
        };

        let unexpected: Vec<String> = deps
            .iter()
            .filter(|dep| !allowed_deps.contains(*dep))
            .cloned()
            .collect();
        for unexpected_dep in unexpected {
            issues.push(format!(
                "`{source}` depends on `{unexpected_dep}`, which is not in the allowed policy set"
            ));
        }
    }

    for (source, destination) in forbidden_pairs {
        if let Some(deps) = packages.get(source)
            && deps.contains(destination)
        {
            issues.push(format!(
                "forbidden dependency `{source}` -> `{destination}` detected"
            ));
        }
    }

    for (source, required_targets) in policy.required_dependencies() {
        let Some(deps) = packages.get(source) else {
            continue;
        };

        for required in required_targets {
            if !deps.contains(required) {
                issues.push(format!("`{source}` is required to depend on `{required}`"));
            }
        }
    }

    issues
}

fn validate_protocol_contracts(
    protocol_file: &Path,
    expected_symbols: &HashSet<String>,
) -> Result<Vec<String>, String> {
    let protocol_text = fs::read_to_string(protocol_file).map_err(|err| {
        format!(
            "unable to read protocol file `{}`: {err}",
            protocol_file.display()
        )
    })?;

    let missing = expected_symbols
        .iter()
        .filter(|symbol| !protocol_contains_symbol(&protocol_text, symbol))
        .map(|symbol| format!("protocol contract symbol `{symbol}` missing from `crates/devil-protocol/src/lib.rs`"))
        .collect();

    Ok(missing)
}

fn protocol_contains_symbol(text: &str, symbol: &str) -> bool {
    for line in text.lines() {
        let line = line.trim();
        if protocol_definition_has_token(line, "struct", symbol)
            || protocol_definition_has_token(line, "enum", symbol)
            || protocol_definition_has_token(line, "trait", symbol)
        {
            return true;
        }
    }

    false
}

fn protocol_definition_has_token(line: &str, keyword: &str, symbol: &str) -> bool {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("pub") => {
            let Some(second_word) = words.next() else {
                return false;
            };
            if second_word != keyword {
                return false;
            }
        }
        Some(word) if word == keyword => {}
        _ => return false,
    }

    let Some(candidate_symbol) = words.next() else {
        return false;
    };

    let Some(found_symbol) = candidate_symbol
        .split(&['(', ';', ':', '{', '<', '[', ','][..])
        .next()
    else {
        return false;
    };

    found_symbol == symbol
}

#[derive(Default)]
struct Policy {
    // Crate -> allowed internal workspace dependencies.
    allowed: HashMap<String, HashSet<String>>,
    // Crate -> required direct dependencies.
    required: HashMap<String, HashSet<String>>,
    // Explicitly forbidden crate dependency pairs.
    forbidden: HashSet<(String, String)>,
    // Boundary symbols expected to exist in protocol crate.
    protocol_symbols: HashSet<String>,
}

impl Policy {
    fn from_markdown(source: &str) -> Result<Self, String> {
        let mut policy = Self::default();

        let mut section = String::new();
        let mut active_crate: Option<String> = None;

        for raw_line in source.lines() {
            let line = raw_line.trim();
            let items = extract_backticked_items(line);

            match line {
                l if l.starts_with("### 1.") => {
                    section = "directional".to_string();
                    active_crate = None;
                }
                l if l.starts_with("### 2.") => {
                    section = "contracts".to_string();
                    active_crate = None;
                }
                l if l.starts_with("###") => {
                    section.clear();
                    active_crate = None;
                }
                _ => {}
            }

            match section.as_str() {
                "directional" => {
                    if line.contains("MUST NOT depend on") {
                        if items.len() >= 2 {
                            policy
                                .forbidden
                                .insert((items[0].clone(), items[1].clone()));
                        }
                        continue;
                    }

                    if line.contains("may depend on") {
                        if let Some(crate_name) = items.first() {
                            active_crate = Some(crate_name.clone());
                            policy.allowed.entry(crate_name.clone()).or_default();
                        }
                        continue;
                    }

                    if (line.starts_with("- ") || line.starts_with("  -"))
                        && let Some(source) = active_crate.clone()
                    {
                        for dep in items {
                            if dep.starts_with("devil-") {
                                policy
                                    .allowed
                                    .entry(source.clone())
                                    .or_default()
                                    .insert(dep);
                            }
                        }
                    }
                }

                "contracts" if line.starts_with("  -") => {
                    for item in items {
                        policy.protocol_symbols.insert(item);
                    }
                }
                _ => {}
            }
        }

        // Enforce explicit hard constraints used by the milestone-0 freeze.
        policy.required.insert(
            "devil-ai".to_string(),
            ["devil-protocol", "devil-security"]
                .iter()
                .map(ToString::to_string)
                .collect(),
        );
        policy.required.insert(
            "devil-ai-providers".to_string(),
            ["devil-ai"].iter().map(ToString::to_string).collect(),
        );
        policy.required.insert(
            "devil-editor".to_string(),
            ["devil-text", "devil-protocol"]
                .iter()
                .map(ToString::to_string)
                .collect(),
        );
        policy.required.insert(
            "devil-platform".to_string(),
            ["devil-protocol"].iter().map(ToString::to_string).collect(),
        );
        policy.required.insert(
            "devil-ui".to_string(),
            ["devil-editor", "devil-protocol"]
                .iter()
                .map(ToString::to_string)
                .collect(),
        );

        // Explicitly forbidden dependency required by the milestone-0 boundary.
        policy
            .forbidden
            .insert(("devil-editor".to_string(), "devil-project".to_string()));

        Ok(policy)
    }

    fn allowed_internal(&self, package: &str) -> Option<&HashSet<String>> {
        self.allowed.get(package)
    }

    fn forbidden_pairs(&self) -> &HashSet<(String, String)> {
        &self.forbidden
    }

    fn required_dependencies(&self) -> &HashMap<String, HashSet<String>> {
        &self.required
    }

    fn protocol_symbols(&self) -> &HashSet<String> {
        &self.protocol_symbols
    }
}

fn extract_backticked_items(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };

        values.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }

    values
}
