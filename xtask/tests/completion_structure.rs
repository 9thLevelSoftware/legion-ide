use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
use xtask::completion::structure::validate_register_structure;

static N: AtomicUsize = AtomicUsize::new(0);
fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-structure-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("plans/completion")).unwrap();
    fs::create_dir_all(root.join("plans/kanban")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/source.md"), "source").unwrap();
    fs::write(
        root.join("plans/kanban/legion-ga-backlog.toml"),
        r#"
[meta]
plan = "completion"
milestone = "S0"
[[epics]]
id = "E1"
title = "Epic"
milestone = "S0"
[[epics.features]]
id = "F1"
title = "Feature"
[[epics.features.tasks]]
id = "LEG-1"
title = "Task"
mode = "Manual"
readiness_row = "R-1"
files = ["docs/source.md"]
dependencies = []
verification = ["check"]
acceptance = ["done"]
stop_condition = "stop"
status = "todo"
"#,
    )
    .unwrap();
    let req = json!({"schema_version":1,"requirements":[{"id":"R-1","title":"Requirement","kind":"product","required":true,"source_refs":[{"path":"docs/source.md","identity":"source"}],"legacy_ids":["LEG-1"],"stage":"S0","package_id":"P-1","owner_role":"owner","depends_on":[],"implementation":"partial","acceptance":"unassessed","scenario_ids":["S-1"],"configuration_ids":["C-1"],"protected_product_ids":[],"defect_ids":["D-1"]}]});
    let matrix = json!({"schema_version":1,"configurations":[{"id":"C-1","os":"windows","architecture":"x64","tool_versions":{"rust":"1"},"hardware":"lab","project_category":"desktop","required":true,"owner_approval_ref":"approval"}]});
    let scenarios = json!({"schema_version":1,"scenarios":[{"id":"S-1","requirement_ids":["R-1"],"configuration_ids":["C-1"],"steps":["run"],"external_oracles":[{"id":"O-1","description":"oracle"}],"recovery_cases":[{"id":"RC-1","description":"recover"}],"sensitive_artifact_policy":"redact"}]});
    let deps = json!({"schema_version":1,"packages":[{"package_id":"P-1","requirement_ids":["R-1"],"milestones":[{"id":"P-1:implemented","depends_on":[],"deliverable_refs":["src"]},{"id":"P-1:accepted","depends_on":["P-1:implemented"],"deliverable_refs":["artifact"]}],"external_prerequisites":[],"owner_role":"owner","implementation_stage":"S0","acceptance_stage":"S0"}]});
    let defects = json!({"schema_version":1,"defects":[{"id":"D-1","requirement_ids":["R-1"],"scenario_id":"S-1","configuration_id":"C-1","severity":"P1","invalidates_required_outcome":false,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"open","repair_package_id":"P-1","verification_run_ids":[]}]});
    for (name, value) in [
        ("requirements.json", req),
        ("matrix.json", matrix),
        ("scenarios.json", scenarios),
        ("dependencies.json", deps),
        ("defects.json", defects),
    ] {
        fs::write(
            root.join("plans/completion").join(name),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
    root
}
fn read(root: &Path, file: &str) -> serde_json::Value {
    serde_json::from_slice(&fs::read(root.join("plans/completion").join(file)).unwrap()).unwrap()
}
fn write(root: &Path, file: &str, value: serde_json::Value) {
    fs::write(
        root.join("plans/completion").join(file),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn valid_incomplete_development_register_passes() {
    let root = fixture();
    let issues = validate_register_structure(&root).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
}
#[test]
fn duplicate_requirement_is_reported() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["id"] = "R-1".into();
    let copy = v["requirements"][0].clone();
    v["requirements"].as_array_mut().unwrap().push(copy);
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("duplicate requirement id")));
}
#[test]
fn unknown_owner_package_is_reported() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["package_id"] = "NOPE".into();
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("package_id unknown")));
}
#[test]
fn unknown_legacy_task_is_reported() {
    let root = fixture();
    let p = root.join("plans/kanban/legion-ga-backlog.toml");
    let s = fs::read_to_string(&p).unwrap().replace("LEG-1", "LEG-X");
    fs::write(p, s).unwrap();
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("unknown legacyID")));
}
#[test]
fn unknown_legacy_reference_is_reported_even_when_task_is_covered() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["legacy_ids"] = json!(["LEG-1", "LEG-NOPE"]);
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(
        e.iter()
            .any(|x| x.contains("unknown Kanban task `LEG-NOPE`"))
    );
}
#[test]
fn missing_source_is_reported() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["source_refs"][0]["path"] = "docs/nope.md".into();
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("missing")));
}
#[test]
fn source_directory_is_reported_as_not_file() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["source_refs"][0]["path"] = "docs".into();
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("is not a file")));
}
#[test]
fn symlink_source_escape_is_rejected_when_supported() {
    let root = fixture();
    let outside = root
        .parent()
        .unwrap()
        .join(format!("legion-outside-{}", std::process::id()));
    fs::write(&outside, "outside").unwrap();
    let link = root.join("docs/link.md");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&outside, &link);
    if linked.is_err() {
        eprintln!("symlink creation unavailable; skipping portability case");
        return;
    }
    let mut v = read(&root, "requirements.json");
    v["requirements"][0]["source_refs"][0]["path"] = "docs/link.md".into();
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("escapes root")));
}
#[test]
fn duplicate_reference_lists_are_reported() {
    let root = fixture();
    let mut req = read(&root, "requirements.json");
    req["requirements"][0]["legacy_ids"] = json!(["LEG-1", "LEG-1"]);
    req["requirements"][0]["protected_product_ids"] = json!([]);
    write(&root, "requirements.json", req);
    let mut deps = read(&root, "dependencies.json");
    deps["packages"][0]["external_prerequisites"] = json!(["x", "x"]);
    deps["packages"][0]["milestones"][0]["deliverable_refs"] = json!(["src", "src"]);
    write(&root, "dependencies.json", deps);
    let mut defects = read(&root, "defects.json");
    defects["defects"][0]["verification_run_ids"] = json!(["RUN-1", "RUN-1"]);
    write(&root, "defects.json", defects);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("legacy_ids duplicate")));
    assert!(
        e.iter()
            .any(|x| x.contains("external_prerequisites duplicate"))
    );
    assert!(e.iter().any(|x| x.contains("deliverable_refs duplicate")));
    assert!(
        e.iter()
            .any(|x| x.contains("verification_run_ids duplicate"))
    );
}
#[test]
fn empty_defect_requirements_are_reported() {
    let root = fixture();
    let mut v = read(&root, "defects.json");
    v["defects"][0]["requirement_ids"] = json!([]);
    write(&root, "defects.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.iter().any(|x| x.contains("requirement_ids is empty")));
}
#[test]
fn internal_protected_product_reference_is_valid_and_duplicates_fail() {
    let root = fixture();
    let mut req = read(&root, "requirements.json");
    let mut product = req["requirements"][0].clone();
    product["id"] = "R-PRODUCT".into();
    product["defect_ids"] = json!([]);
    product["legacy_ids"] = json!([]);
    product["required"] = false.into();
    product["scenario_ids"] = json!([]);
    product["configuration_ids"] = json!([]);
    let mut internal = req["requirements"][0].clone();
    internal["id"] = "R-INTERNAL".into();
    internal["kind"] = "internal".into();
    internal["required"] = false.into();
    internal["scenario_ids"] = json!([]);
    internal["configuration_ids"] = json!([]);
    internal["defect_ids"] = json!([]);
    internal["protected_product_ids"] = json!(["R-PRODUCT"]);
    internal["legacy_ids"] = json!([]);
    req["requirements"]
        .as_array_mut()
        .unwrap()
        .extend([product, internal]);
    write(&root, "requirements.json", req);
    let mut deps = read(&root, "dependencies.json");
    deps["packages"][0]["requirement_ids"] = json!(["R-1", "R-PRODUCT", "R-INTERNAL"]);
    write(&root, "dependencies.json", deps);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.is_empty(), "{e:?}");

    let mut req = read(&root, "requirements.json");
    req["requirements"][1]["protected_product_ids"] = json!(["R-PRODUCT", "R-PRODUCT"]);
    write(&root, "requirements.json", req);
    let e = validate_register_structure(&root).unwrap();
    assert!(
        e.iter()
            .any(|x| x.contains("protected_product_ids duplicate"))
    );
}
#[test]
fn consumer_package_and_signing_crosscut_graph_are_valid() {
    let root = fixture();
    let mut deps = read(&root, "dependencies.json");
    let milestones = deps["packages"][0]["milestones"].as_array_mut().unwrap();
    milestones.extend([
        json!({"id":"P-1:signerimplemented","depends_on":[],"deliverable_refs":["signer"]}),
        json!({"id":"P-1:helperimplemented","depends_on":[],"deliverable_refs":["helper"]}),
        json!({"id":"P-1:manualimplemented","depends_on":[],"deliverable_refs":["manual"]}),
        json!({"id":"P-1:artifactsready","depends_on":["P-1:signerimplemented","P-1:helperimplemented","P-1:manualimplemented"],"deliverable_refs":["artifacts"]}),
        json!({"id":"P-1:helperaccepted","depends_on":["P-1:artifactsready"],"deliverable_refs":["helper-ok"]}),
        json!({"id":"P-1:manualaccepted","depends_on":["P-1:artifactsready"],"deliverable_refs":["manual-ok"]}),
    ]);
    deps["packages"].as_array_mut().unwrap().push(json!({"package_id":"P-2","requirement_ids":["R-1"],"milestones":[{"id":"P-2:implemented","depends_on":[],"deliverable_refs":["planned"]},{"id":"P-2:accepted","depends_on":["P-2:implemented"],"deliverable_refs":["planned-ok"]}],"external_prerequisites":[],"owner_role":"consumer","implementation_stage":"S0","acceptance_stage":"S0"}));
    write(&root, "dependencies.json", deps);
    let e = validate_register_structure(&root).unwrap();
    assert!(e.is_empty(), "{e:?}");
}
#[test]
fn missing_backlog_error_names_file() {
    let root = fixture();
    fs::remove_file(root.join("plans/kanban/legion-ga-backlog.toml")).unwrap();
    let e = validate_register_structure(&root).unwrap_err();
    assert!(e.contains("plans/kanban/legion-ga-backlog.toml"));
}

fn assert_case<F>(mutate: F, needle: &str)
where
    F: FnOnce(&Path),
{
    let root = fixture();
    mutate(&root);
    let issues = validate_register_structure(&root).unwrap();
    assert!(
        issues.iter().any(|x| x.contains(needle)),
        "missing `{needle}` in {issues:?}"
    );
}

fn internal_fixture(root: &Path) {
    let mut req = read(root, "requirements.json");
    let mut product = req["requirements"][0].clone();
    product["id"] = "R-PRODUCT".into();
    product["required"] = false.into();
    product["scenario_ids"] = json!([]);
    product["configuration_ids"] = json!([]);
    product["legacy_ids"] = json!([]);
    product["defect_ids"] = json!([]);
    let mut internal = product.clone();
    internal["id"] = "R-INTERNAL".into();
    internal["kind"] = "internal".into();
    internal["protected_product_ids"] = json!(["R-PRODUCT"]);
    req["requirements"]
        .as_array_mut()
        .unwrap()
        .extend([product, internal]);
    write(root, "requirements.json", req);
    let mut deps = read(root, "dependencies.json");
    deps["packages"][0]["requirement_ids"] = json!(["R-1", "R-PRODUCT", "R-INTERNAL"]);
    write(root, "dependencies.json", deps);
}

#[test]
fn required_reference_and_coverage_mutations_are_rejected() {
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "scenarios is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "matrix.json");
            v["configurations"] = json!([]);
            write(r, "matrix.json", v);
        },
        "configurations is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["source_refs"][0]["path"] = "../outside".into();
            write(r, "requirements.json", v);
        },
        "escapes root",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["source_refs"][0]["path"] = r
                .parent()
                .unwrap()
                .join("outside-absolute")
                .to_string_lossy()
                .to_string()
                .into();
            write(r, "requirements.json", v);
        },
        "escapes root",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["package_id"] = "P-1".into();
            let mut d = read(r, "dependencies.json");
            d["packages"][0]["requirement_ids"] = json!([]);
            write(r, "requirements.json", v);
            write(r, "dependencies.json", d);
        },
        "does not list owned requirement",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["depends_on"] = json!(["NOPE"]);
            write(r, "requirements.json", v);
        },
        "depends_on unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["depends_on"] = json!(["R-1"]);
            write(r, "requirements.json", v);
        },
        "depends_on self-reference",
    );
    assert_case(
        |r| {
            internal_fixture(r);
            let mut v = read(r, "requirements.json");
            v["requirements"][2]["protected_product_ids"] = json!(["R-INTERNAL"]);
            write(r, "requirements.json", v);
        },
        "not a product requirement",
    );
    assert_case(
        |r| {
            internal_fixture(r);
            let mut v = read(r, "requirements.json");
            v["requirements"][2]["protected_product_ids"] = json!(["R-INTERNAL"]);
            write(r, "requirements.json", v);
        },
        "protected_product_ids self-reference",
    );
    assert_case(
        |r| {
            internal_fixture(r);
            let mut v = read(r, "requirements.json");
            v["requirements"][2]["protected_product_ids"] = json!([]);
            write(r, "requirements.json", v);
        },
        "protected_product_ids is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["scenario_ids"] = json!([]);
            write(r, "requirements.json", v);
        },
        "empty scenario/configuration coverage",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["configuration_ids"] = json!([]);
            write(r, "requirements.json", v);
        },
        "empty scenario/configuration coverage",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["requirement_ids"] = json!(["NOPE"]);
            write(r, "scenarios.json", v);
        },
        "requirement_ids unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["configuration_ids"] = json!(["NOPE"]);
            write(r, "scenarios.json", v);
        },
        "configuration_ids unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["requirement_ids"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "R-1 scenario link S-1 is not bidirectional",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["scenario_ids"] = json!([]);
            write(r, "requirements.json", v);
        },
        "S-1 requirement link R-1 is not bidirectional",
    );
    assert_case(
        |r| {
            let mut m = read(r, "matrix.json");
            m["configurations"].as_array_mut().unwrap().push(json!({"id":"C-2","os":"windows","architecture":"x64","tool_versions":{"rust":"1"},"hardware":"lab","project_category":"desktop","required":true,"owner_approval_ref":"approval"}));
            write(r, "matrix.json", m);
            let mut s = read(r, "scenarios.json");
            s["scenarios"][0]["configuration_ids"] = json!(["C-2"]);
            write(r, "scenarios.json", s);
        },
        "not linked by requirement",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["configuration_ids"] = json!(["C-2"]);
            let mut m = read(r, "matrix.json");
            m["configurations"].as_array_mut().unwrap().push(json!({"id":"C-2","os":"windows","architecture":"x64","tool_versions":{"rust":"1"},"hardware":"lab","project_category":"desktop","required":true,"owner_approval_ref":"approval"}));
            write(r, "requirements.json", v);
            write(r, "matrix.json", m);
        },
        "lacks scenario coverage",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["steps"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "steps is empty or blank",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["steps"] = json!([" "]);
            write(r, "scenarios.json", v);
        },
        "steps is empty or blank",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["external_oracles"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "external_oracles is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["recovery_cases"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "recovery_cases is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["external_oracles"][0]["id"] = " ".into();
            write(r, "scenarios.json", v);
        },
        "blank id/description",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["recovery_cases"][0]["id"] = "O-1".into();
            write(r, "scenarios.json", v);
        },
        "check duplicate id",
    );
    assert_case(
        |r| {
            let mut v = read(r, "matrix.json");
            v["configurations"][0]["owner_approval_ref"] = " ".into();
            write(r, "matrix.json", v);
        },
        "owner_approval_ref",
    );
    assert_case(
        |r| {
            let mut v = read(r, "matrix.json");
            v["configurations"][0]["tool_versions"] = json!({});
            write(r, "matrix.json", v);
        },
        "tool_versions is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "matrix.json");
            v["configurations"][0]["tool_versions"][" "] = "1".into();
            write(r, "matrix.json", v);
        },
        "tool_versions is empty or blank",
    );
}

#[test]
fn milestone_and_defect_mutations_are_rejected() {
    assert_case(
        |r| {
            let mut v = read(r, "dependencies.json");
            v["packages"][0]["milestones"][0]["id"] = "P-1:other".into();
            write(r, "dependencies.json", v);
        },
        "missing implemented milestone",
    );
    assert_case(
        |r| {
            let mut v = read(r, "dependencies.json");
            v["packages"][0]["milestones"][1]["id"] = "P-1:other".into();
            write(r, "dependencies.json", v);
        },
        "missing accepted milestone",
    );
    assert_case(
        |r| {
            let mut v = read(r, "dependencies.json");
            v["packages"][0]["milestones"][0]["depends_on"] = json!(["P-1:NOPE"]);
            write(r, "dependencies.json", v);
        },
        "unknown dependency",
    );
    assert_case(
        |r| {
            let mut v = read(r, "dependencies.json");
            v["packages"][0]["milestones"][0]["depends_on"] = json!(["P-1:implemented"]);
            write(r, "dependencies.json", v);
        },
        "self-reference",
    );
    assert_case(
        |r| {
            let mut v = read(r, "dependencies.json");
            v["packages"][0]["milestones"][0]["depends_on"] = json!(["P-1:accepted"]);
            v["packages"][0]["milestones"][1]["depends_on"] = json!(["P-1:implemented"]);
            write(r, "dependencies.json", v);
        },
        "milestone dependency cycle",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["requirement_ids"] = json!(["NOPE"]);
            write(r, "defects.json", v);
        },
        "requirement_ids unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["scenario_id"] = "NOPE".into();
            write(r, "defects.json", v);
        },
        "scenario_id unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["configuration_id"] = "NOPE".into();
            write(r, "defects.json", v);
        },
        "configuration_id unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["repair_package_id"] = "NOPE".into();
            write(r, "defects.json", v);
        },
        "repair_package_id unknown",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["defect_ids"] = json!([]);
            write(r, "requirements.json", v);
        },
        "D-1 requirement link is not bidirectional",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["requirement_ids"] = json!([]);
            write(r, "defects.json", v);
        },
        "requirement_ids is empty",
    );
    assert_case(
        |r| {
            let mut v = read(r, "scenarios.json");
            v["scenarios"][0]["requirement_ids"] = json!([]);
            write(r, "scenarios.json", v);
        },
        "scenario/configuration does not cover",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["reproduction"] = json!([]);
            write(r, "defects.json", v);
        },
        "reproduction/expected/observed/owner",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["expected"] = " ".into();
            write(r, "defects.json", v);
        },
        "reproduction/expected/observed/owner",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["observed"] = " ".into();
            write(r, "defects.json", v);
        },
        "reproduction/expected/observed/owner",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["owner"] = " ".into();
            write(r, "defects.json", v);
        },
        "reproduction/expected/observed/owner",
    );
    assert_case(
        |r| {
            let mut v = read(r, "defects.json");
            v["defects"][0]["status"] = "closed".into();
            write(r, "defects.json", v);
        },
        "closed defect has no verification_run_ids",
    );
    assert_case(
        |r| {
            let mut v = read(r, "requirements.json");
            v["requirements"][0]["implementation"] = "partial".into();
            v["requirements"][0]["acceptance"] = "accepted".into();
            write(r, "requirements.json", v);
        },
        "accepted while implementation is not implemented",
    );
}

#[test]
fn empty_defect_register_is_valid() {
    let root = fixture();
    let mut defects = read(&root, "defects.json");
    defects["defects"] = json!([]);
    write(&root, "defects.json", defects);
    let mut req = read(&root, "requirements.json");
    req["requirements"][0]["defect_ids"] = json!([]);
    write(&root, "requirements.json", req);
    assert!(validate_register_structure(&root).unwrap().is_empty());
}
#[test]
fn cyclic_requirement_dependencies_are_reported() {
    let root = fixture();
    let mut v = read(&root, "requirements.json");
    let mut second = v["requirements"][0].clone();
    second["id"] = "R-2".into();
    second["legacy_ids"] = json!([]);
    second["depends_on"] = json!(["R-1"]);
    v["requirements"][0]["depends_on"] = json!(["R-2"]);
    v["requirements"].as_array_mut().unwrap().push(second);
    write(&root, "requirements.json", v);
    let e = validate_register_structure(&root).unwrap();
    assert!(
        e.iter()
            .any(|x| x.contains("self-reference") || x.contains("cycle"))
    );
}
#[test]
fn malformed_json_is_a_load_error() {
    let root = fixture();
    fs::write(root.join("plans/completion/matrix.json"), "{").unwrap();
    let e = validate_register_structure(&root).unwrap_err();
    assert!(e.contains("matrix.json"));
}
