use serde_json::{Value, json};
use xtask::completion::schema::{
    CandidateManifest, Defect, DefectsDocument, DependenciesDocument, EvidenceRun, MatrixDocument,
    RequirementsDocument, ScenariosDocument,
};

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("fixture should parse")
}
fn requirements() -> Value {
    json!({"schema_version":1,"requirements":[{"id":"R-1","title":"Ship","kind":"product","required":true,"source_refs":[{"path":"docs/a.md","identity":"sec-1"}],"legacy_ids":[],"stage":"S0","package_id":"P-1","owner_role":"owner","depends_on":[],"implementation":"implemented","acceptance":"accepted","scenario_ids":["SC-1"],"configuration_ids":["CFG-1"],"protected_product_ids":[],"defect_ids":[]}]})
}
fn matrix() -> Value {
    json!({"schema_version":1,"configurations":[{"id":"CFG-1","os":"windows","architecture":"x64","tool_versions":{"rust":"1.0"},"hardware":"lab","project_category":"desktop","required":true,"owner_approval_ref":"APR-1"}]})
}
fn scenarios() -> Value {
    json!({"schema_version":1,"scenarios":[{"id":"SC-1","requirement_ids":["R-1"],"configuration_ids":["CFG-1"],"steps":["run"],"external_oracles":[{"id":"O-1","description":"oracle"}],"recovery_cases":[{"id":"RC-1","description":"recover"}],"sensitive_artifact_policy":"redact"}]})
}
fn evidence() -> Value {
    json!({"schema_version":1,"id":"RUN-1","scenario_id":"SC-1","configuration_id":"CFG-1","candidate_sha":"abc","artifact_sha256":"def","layer":"product","input_route":"native-input","dependencies":[{"name":"browser","version":"1","execution":"real","required_for_outcome":true,"substitution_reason":""}],"result":"passed","oracle_results":[{"id":"O-1","passed":true,"artifact_path":"o.txt","observed":"ok"}],"recovery_results":[{"id":"RC-1","passed":true,"artifact_path":"r.txt","observed":"ok"}],"artifact_hashes":{"o.txt":"def"},"defect_ids":[],"implementation_owners":["owner"],"reviewer":"reviewer","review_decision":"accepted","started_at_utc":"2026-01-01T00:00:00Z","ended_at_utc":"2026-01-01T00:01:00Z"})
}
fn defect() -> Value {
    json!({"id":"D-1","requirement_ids":["R-1"],"scenario_id":"SC-1","configuration_id":"CFG-1","severity":"P1","invalidates_required_outcome":false,"reproduction":["run"],"expected":"ok","observed":"bad","owner":"owner","status":"open","repair_package_id":"P-1","verification_run_ids":[]})
}
fn dependencies() -> Value {
    json!({"schema_version":1,"packages":[{"package_id":"P-1","requirement_ids":["R-1"],"milestones":[{"id":"P-1:implemented","depends_on":[],"deliverable_refs":["src"]}],"external_prerequisites":[],"owner_role":"owner","implementation_stage":"S0","acceptance_stage":"S0"}]})
}
fn candidate() -> Value {
    json!({"schema_version":1,"code_sha":"abc","artifacts":[{"component":"app","configuration_id":"CFG-1","path":"app.exe","sha256":"def","build_provenance_path":"build.toml"}],"configuration_ids":["CFG-1"],"nominated_at_utc":"2026-01-01T00:00:00Z","nominated_by":"owner"})
}
fn with_extra(mut value: Value) -> Value {
    value
        .as_object_mut()
        .unwrap()
        .insert("extra".into(), json!(true));
    value
}
fn without(mut value: Value, field: &str) -> Value {
    value.as_object_mut().unwrap().remove(field);
    value
}

#[test]
fn parses_all_completion_roots_and_nested_values() {
    assert_eq!(
        parse::<RequirementsDocument>(requirements()).requirements[0].source_refs[0].identity,
        "sec-1"
    );
    assert_eq!(
        parse::<MatrixDocument>(matrix()).configurations[0]
            .os
            .to_string(),
        "windows"
    );
    assert_eq!(
        parse::<ScenariosDocument>(scenarios()).scenarios[0].external_oracles[0].id,
        "O-1"
    );
    assert_eq!(
        parse::<EvidenceRun>(evidence()).result.to_string(),
        "passed"
    );
    assert_eq!(parse::<Defect>(defect()).severity.to_string(), "P1");
    assert_eq!(
        parse::<DefectsDocument>(json!({"schema_version":1,"defects":[defect()]})).defects[0].id,
        "D-1"
    );
    assert_eq!(
        parse::<DependenciesDocument>(dependencies()).packages[0].milestones[0].id,
        "P-1:implemented"
    );
    assert_eq!(
        parse::<CandidateManifest>(candidate()).artifacts[0].sha256,
        "def"
    );
}

#[test]
fn rejects_unknown_fields_at_every_root() {
    assert!(serde_json::from_value::<RequirementsDocument>(with_extra(requirements())).is_err());
    assert!(serde_json::from_value::<MatrixDocument>(with_extra(matrix())).is_err());
    assert!(serde_json::from_value::<ScenariosDocument>(with_extra(scenarios())).is_err());
    assert!(serde_json::from_value::<EvidenceRun>(with_extra(evidence())).is_err());
    assert!(
        serde_json::from_value::<DefectsDocument>(with_extra(
            json!({"schema_version":1,"defects":[]})
        ))
        .is_err()
    );
    assert!(serde_json::from_value::<DependenciesDocument>(with_extra(dependencies())).is_err());
    assert!(serde_json::from_value::<CandidateManifest>(with_extra(candidate())).is_err());
}

#[test]
fn rejects_unknown_fields_in_each_required_nested_object() {
    let mut v = requirements();
    v["requirements"][0]["source_refs"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<RequirementsDocument>(v).is_err());
    let mut v = scenarios();
    v["scenarios"][0]["external_oracles"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<ScenariosDocument>(v).is_err());
    let mut v = evidence();
    v["dependencies"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<EvidenceRun>(v).is_err());
    let mut v = evidence();
    v["oracle_results"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<EvidenceRun>(v).is_err());
    let mut v = dependencies();
    v["packages"][0]["milestones"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<DependenciesDocument>(v).is_err());
    let mut v = candidate();
    v["artifacts"][0]["extra"] = json!(true);
    assert!(serde_json::from_value::<CandidateManifest>(v).is_err());
}

#[test]
fn rejects_bad_versions_and_missing_root_fields_independently() {
    for version in [0, 2] {
        let mut v = matrix();
        v["schema_version"] = json!(version);
        assert!(serde_json::from_value::<MatrixDocument>(v).is_err());
    }
    let mut v = matrix();
    v["schema_version"] = json!("1");
    assert!(serde_json::from_value::<MatrixDocument>(v).is_err());
    assert!(serde_json::from_value::<MatrixDocument>(without(matrix(), "schema_version")).is_err());
    assert!(serde_json::from_value::<MatrixDocument>(without(matrix(), "configurations")).is_err());
    assert!(
        serde_json::from_value::<RequirementsDocument>(without(requirements(), "schema_version"))
            .is_err()
    );
    assert!(
        serde_json::from_value::<ScenariosDocument>(without(scenarios(), "schema_version"))
            .is_err()
    );
    assert!(serde_json::from_value::<EvidenceRun>(without(evidence(), "schema_version")).is_err());
    assert!(
        serde_json::from_value::<DefectsDocument>(without(
            json!({"schema_version":1,"defects":[]}),
            "schema_version"
        ))
        .is_err()
    );
    assert!(
        serde_json::from_value::<DependenciesDocument>(without(dependencies(), "schema_version"))
            .is_err()
    );
    assert!(
        serde_json::from_value::<CandidateManifest>(without(candidate(), "schema_version"))
            .is_err()
    );
}

#[test]
fn rejects_wrong_enum_labels() {
    let mut v = requirements();
    v["requirements"][0]["kind"] = json!("PRODUCT");
    assert!(serde_json::from_value::<RequirementsDocument>(v).is_err());
    let mut v = requirements();
    v["requirements"][0]["implementation"] = json!("done");
    assert!(serde_json::from_value::<RequirementsDocument>(v).is_err());
    let mut v = requirements();
    v["requirements"][0]["acceptance"] = json!("pass");
    assert!(serde_json::from_value::<RequirementsDocument>(v).is_err());
    let mut v = requirements();
    v["requirements"][0]["stage"] = json!("s0");
    assert!(serde_json::from_value::<RequirementsDocument>(v).is_err());
    let mut v = matrix();
    v["configurations"][0]["os"] = json!("solaris");
    assert!(serde_json::from_value::<MatrixDocument>(v).is_err());
    let mut v = evidence();
    v["dependencies"][0]["execution"] = json!("fake");
    assert!(serde_json::from_value::<EvidenceRun>(v).is_err());
    for (field, label) in [
        ("layer", "Layer"),
        ("input_route", "direct"),
        ("result", "ok"),
        ("review_decision", "yes"),
    ] {
        let mut v = evidence();
        v[field] = json!(label);
        assert!(serde_json::from_value::<EvidenceRun>(v).is_err());
    }
    let mut v = defect();
    v["severity"] = json!("P4");
    assert!(serde_json::from_value::<Defect>(v).is_err());
    let mut v = defect();
    v["status"] = json!("fixed");
    assert!(serde_json::from_value::<Defect>(v).is_err());
    let mut v = dependencies();
    v["packages"][0]["implementation_stage"] = json!("S7");
    assert!(serde_json::from_value::<DependenciesDocument>(v).is_err());
}
