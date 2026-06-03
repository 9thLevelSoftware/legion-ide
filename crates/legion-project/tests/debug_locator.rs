use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use legion_project::{CargoDebugLocatorOptions, discover_cargo_debug_configurations};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempCargoProject {
    root: PathBuf,
}

impl TempCargoProject {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_debug_locator_{}_{}",
            std::process::id(),
            id
        ));
        fs::create_dir(&root).expect("temp project should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }
        fs::write(path, content).expect("fixture file should be written");
    }
}

impl Drop for TempCargoProject {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_debug_locator_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn cargo_locator_builds_deterministic_lldb_launch_configs_for_bins() {
    let project = TempCargoProject::new();
    project.write(
        "Cargo.toml",
        r#"[package]
name = "sample-app"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "admin-tool"
path = "src/bin/admin.rs"
"#,
    );
    project.write("src/main.rs", "fn main() {}\n");
    project.write("src/bin/admin.rs", "fn main() {}\n");

    let configs =
        discover_cargo_debug_configurations(project.path(), CargoDebugLocatorOptions::default())
            .expect("cargo locator should inspect fixture manifest");

    assert_eq!(configs.len(), 2);
    let package_bin = configs
        .iter()
        .find(|config| config.configuration_id.0 == "cargo:sample-app:bin:sample-app")
        .expect("package default binary config should exist");
    assert_eq!(package_bin.adapter_type, "lldb-dap");
    assert_eq!(package_bin.cargo_package.as_deref(), Some("sample-app"));
    assert_eq!(package_bin.cargo_target.as_deref(), Some("sample-app"));
    assert_eq!(
        package_bin.cargo_args,
        vec![
            "build".to_string(),
            "--package".to_string(),
            "sample-app".to_string(),
            "--bin".to_string(),
            "sample-app".to_string(),
        ]
    );
    assert!(
        package_bin
            .program_label
            .ends_with("target/debug/sample-app")
    );
    assert!(package_bin.deterministic);

    assert!(configs.iter().any(|config| {
        config.configuration_id.0 == "cargo:sample-app:bin:admin-tool"
            && config.program_label.ends_with("target/debug/admin-tool")
    }));
}
