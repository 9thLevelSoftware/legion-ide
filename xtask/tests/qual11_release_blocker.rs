use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate parent is the workspace root")
        .to_path_buf()
}

#[test]
fn qual11_taxonomy_defines_p0_register_and_labels() {
    let text = fs::read_to_string(repo_root().join("plans/qual-11-release-blocker-taxonomy.md"))
        .expect("QUAL.11 taxonomy");
    assert!(text.contains("QUAL.11"));
    assert!(text.contains("`qual-11`"));
    assert!(text.contains("`release-blocker`"));
    for id in [
        "P0-01", "P0-02", "P0-03", "P0-04", "P0-05", "P0-06", "P0-07", "P0-08", "P0-09", "P0-10",
    ] {
        assert!(text.contains(id), "taxonomy missing {id}");
    }
    assert!(
        text.contains("bug_report.md"),
        "taxonomy must refuse the generic bug template as the queue"
    );
}

#[test]
fn release_blocker_issue_template_is_not_the_bug_template() {
    let template = fs::read_to_string(
        repo_root().join(".github/ISSUE_TEMPLATE/release-blocker.yml"),
    )
    .expect("release-blocker issue form");
    assert!(template.contains("name: Release blocker"));
    assert!(template.contains("qual-11"));
    assert!(template.contains("release-blocker"));
    assert!(template.contains("GAP / P0 id"));
    assert!(template.contains("Ledger row"));
    assert!(template.contains("Owner"));
    assert!(!template.contains("name: Bug report"));

    let config =
        fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/config.yml")).expect("config");
    assert!(config.contains("qual-11-release-blocker-taxonomy.md"));
}
