use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::no_egui_textedit::{NoEguiTextEditConfig, run_no_egui_textedit};

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("legion-no-egui-textedit-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create temp repo root");
        Self { root }
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, text).expect("write fixture file");
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn no_egui_textedit_flags_textedit_in_scanned_path() {
    let repo = TempRepo::new("flags-textedit");
    repo.write(
        "crates/legion-desktop/src/view.rs",
        "fn render() { let _widget = egui::TextEdit::multiline(&mut String::new()); }\n",
    );

    let result = run_no_egui_textedit(&repo.root, &NoEguiTextEditConfig::default());
    let violations = result.expect_err("expected egui::TextEdit violation");

    assert!(
        violations.iter().any(|violation| {
            violation.line == 1 && violation.message.contains("egui::TextEdit")
        })
    );
}

#[test]
fn no_egui_textedit_flags_textedit_in_painter_module() {
    let repo = TempRepo::new("flags-painter-module");
    repo.write(
        "crates/legion-desktop/src/view/code_canvas_painter.rs",
        "fn render() { let _widget = egui::TextEdit::singleline(&mut String::new()); }\n",
    );

    let result = run_no_egui_textedit(&repo.root, &NoEguiTextEditConfig::default());
    let violations = result.expect_err("expected egui::TextEdit violation in painter module");

    assert!(violations.iter().any(|violation| {
        violation.path.ends_with("view/code_canvas_painter.rs")
            && violation.message.contains("egui::TextEdit")
    }));
}

#[test]
fn no_egui_textedit_allows_textedit_in_unscanned_path() {
    let repo = TempRepo::new("unscanned-path");
    repo.write(
        "crates/legion-desktop/tests/fixtures/view.rs",
        "fn fixture() { let _widget = egui::TextEdit::singleline(&mut String::new()); }\n",
    );

    run_no_egui_textedit(&repo.root, &NoEguiTextEditConfig::default())
        .expect("unscanned paths should pass");
}

#[test]
fn no_egui_textedit_allows_textedit_in_allowlisted_path() {
    let repo = TempRepo::new("allowlisted-path");
    repo.write(
        "crates/legion-desktop/src/generated/fixture.rs",
        "fn fixture() { let _widget = egui::TextEdit::singleline(&mut String::new()); }\n",
    );
    let config = NoEguiTextEditConfig {
        allowlisted_paths: vec!["crates/legion-desktop/src/generated/".to_string()],
        ..NoEguiTextEditConfig::default()
    };

    run_no_egui_textedit(&repo.root, &config).expect("allowlisted paths should pass");
}

#[test]
fn no_egui_textedit_ignores_legion_text_textedit() {
    let repo = TempRepo::new("legion-text-textedit");
    repo.write(
        "crates/legion-desktop/src/view.rs",
        "use legion_text::TextEdit;\nfn edit(edit: TextEdit) { let _ = edit; }\n",
    );

    run_no_egui_textedit(&repo.root, &NoEguiTextEditConfig::default())
        .expect("legion_text::TextEdit is not the forbidden egui widget");
}

#[test]
fn no_egui_textedit_loads_config_from_toml() {
    let repo = TempRepo::new("config");
    repo.write(
        "xtask/no-egui-textedit.toml",
        "scanned_paths = [\"crates/legion-desktop/src/\"]\nallowlisted_paths = [\"crates/legion-desktop/src/generated/\"]\nforbidden_tokens = [\"egui::TextEdit\"]\n",
    );

    let config = NoEguiTextEditConfig::from_file(&repo.path("xtask/no-egui-textedit.toml"))
        .expect("config should parse");

    assert_eq!(config.scanned_paths, vec!["crates/legion-desktop/src/"]);
    assert_eq!(
        config.allowlisted_paths,
        vec!["crates/legion-desktop/src/generated/"]
    );
    assert_eq!(config.forbidden_tokens, vec!["egui::TextEdit"]);
}
