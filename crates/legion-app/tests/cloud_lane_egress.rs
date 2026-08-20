//! Cloud Lane egress: a manifest that was shown, and a cancel that reaches it.
//!
//! P9.F3.T3's stop condition is "stop if a Cloud Lane upload can complete
//! without surfacing its manifest". The pieces that looked like they enforced
//! that did not:
//!
//! * `LegionCloudLaneUploadManifest::scope_visible_to_user` is a `bool` the
//!   caller sets. The contract validator refuses `false` and the security
//!   broker denies submit without it, so the flag was enforced — but nothing
//!   rendered a manifest, so the flag attested to something that never
//!   happened. A caller could set it `true` and upload anything.
//! * `cancel_task` existed on the transport from the start with no product
//!   path reaching it, so "cancellable mid-flight" was true of the transport
//!   and false of the application.

use legion_app::{
    AppComposition,
    cloud_lane_egress::{CloudLaneEgressDisposition, CloudLaneEgressManifestView},
};
use legion_protocol::{
    CancellationTokenId, CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId,
    CausalityId, CorrelationId, FileFingerprint, LegionCloudLaneBudget,
    LegionCloudLaneSecretScanStatus, LegionCloudLaneTaskId, LegionCloudLaneTaskRequest,
    LegionCloudLaneTaskState, LegionCloudLaneUploadManifest, LegionEvidenceKind,
    LegionProviderLocalityPreference, LegionProviderPrivacyPolicy, LegionTaskContextRef,
    LegionTaskContextRefKind, LegionTaskFileScope, LegionTaskOutputContract, LegionTaskPacket,
    LegionTaskPacketId, LegionTaskPolicy, LegionTaskValidationPlan, LegionWorkerResultKind,
    PrincipalId, RedactionHint, WorkspaceId, WorkspaceTrustState,
};

/// A throwaway workspace root, removed on drop.
struct TempWorkspace {
    root: std::path::PathBuf,
}

impl TempWorkspace {
    fn path(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        // Guarded on the prefix so a bug in path construction cannot delete
        // something else.
        if self.root.starts_with(std::env::temp_dir())
            && self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("legion-cloud-lane-egress-"))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn temp_workspace() -> TempWorkspace {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |value| value.as_nanos());
    let root = std::env::temp_dir().join(format!(
        "legion-cloud-lane-egress-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    std::fs::write(
        root.join("main.rs"),
        "fn main() {}
",
    )
    .expect("seed workspace file");
    TempWorkspace { root }
}

fn automate_app_with_workspace(root: &std::path::Path) -> (AppComposition, WorkspaceId) {
    let mut app = AppComposition::new();
    app.set_product_mode(legion_app::AppProductMode::Automate);
    let opened = app
        .open_workspace(
            root,
            WorkspaceTrustState::Trusted,
            PrincipalId("principal:cloud".to_string()),
        )
        .expect("open workspace");
    let workspace_id = opened.workspace_id;
    (app, workspace_id)
}

/// A second scope, used to grow a manifest after it was acknowledged.
fn extra_scope(label: &str, path: &str) -> LegionTaskFileScope {
    LegionTaskFileScope {
        scope_id: label.to_string(),
        path: CanonicalPath(path.to_string()),
        fingerprint: Some(fingerprint(label)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn causality(value: u128) -> CausalityId {
    CausalityId(uuid::Uuid::from_u128(value))
}

fn cloud_lane_task_request(workspace_id: WorkspaceId) -> LegionCloudLaneTaskRequest {
    let allowed_scope = LegionTaskFileScope {
        scope_id: "cloud-app-allowed:main".to_string(),
        path: CanonicalPath("/workspace/main.txt".to_string()),
        fingerprint: Some(fingerprint("cloud-app-main")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let forbidden_scope = LegionTaskFileScope {
        scope_id: "cloud-app-forbidden:env".to_string(),
        path: CanonicalPath("/workspace/.env".to_string()),
        fingerprint: Some(fingerprint("cloud-app-env")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    LegionCloudLaneTaskRequest {
        task_id: LegionCloudLaneTaskId("cloud-task:app:1".to_string()),
        lane_id: "cloud-lane:validation".to_string(),
        control_plane_endpoint_id: "endpoint:legion-cloud:app".to_string(),
        task_packet: LegionTaskPacket {
            packet_id: LegionTaskPacketId("cloud-packet:app:1".to_string()),
            workspace_id,
            objective_summary_hash: fingerprint("cloud-app-objective"),
            allowed_files: vec![allowed_scope.clone()],
            forbidden_files: vec![forbidden_scope.clone()],
            context_snippet_refs: vec![LegionTaskContextRef {
                reference_id: "cloud-app-context:1".to_string(),
                kind: LegionTaskContextRefKind::ContextSnippet,
                payload_hash: fingerprint("cloud-app-context-hash"),
                redacted_summary: "redacted cloud task context".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            full_file_refs: Vec::new(),
            command_output_refs: Vec::new(),
            output_contract: LegionTaskOutputContract {
                expected_result_kind: LegionWorkerResultKind::PatchProposal,
                proposal_only: true,
                direct_mutation_allowed: false,
                required_evidence_kinds: vec![LegionEvidenceKind::CommandRun],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            validation_plan: LegionTaskValidationPlan {
                required_commands: vec!["cargo test -p legion-app legion_cloud_lane".to_string()],
                success_criteria: vec!["cloud lane app test passes".to_string()],
                stop_conditions: vec!["policy denied".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy: LegionTaskPolicy {
                locality_preference: LegionProviderLocalityPreference::RemoteAllowed,
                privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
                cost_budget_cents: Some(75),
                latency_budget_ms: Some(30_000),
                allow_network: true,
                allow_direct_workspace_mutation: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            correlation_id: CorrelationId(901),
            causality_id: causality(901),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        upload_manifest: LegionCloudLaneUploadManifest {
            manifest_id: "cloud-upload:app:1".to_string(),
            allowed_files: vec![allowed_scope],
            forbidden_files: vec![forbidden_scope],
            total_upload_bytes: 12_288,
            scope_visible_to_user: true,
            contains_forbidden_material: false,
            secret_scan_status: LegionCloudLaneSecretScanStatus::Passed,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        budget: LegionCloudLaneBudget {
            max_cost_cents: 75,
            estimated_cost_cents: 50,
            max_queue_depth: 2,
            current_queue_depth: 1,
            usage_metering_label: "meter:app:cloud-lane".to_string(),
            hard_cap_enforced: true,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        capability_decision: CapabilityDecision {
            decision_id: CapabilityDecisionId(701),
            granted: true,
            capability: CapabilityId("cloud.lane.submit".to_string()),
            reason: Some("allowed".to_string()),
        },
        cancellation_token: CancellationTokenId(uuid::Uuid::from_u128(0xaaaa)),
        correlation_id: CorrelationId(901),
        causality_id: causality(901),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn the_manifest_lists_what_leaves_and_what_is_withheld() {
    let view = CloudLaneEgressManifestView::from_request(&cloud_lane_task_request(WorkspaceId(11)));

    let rows = view.rows();
    assert_eq!(rows.len(), 2, "one allowed and one forbidden scope");
    assert_eq!(rows[0].path, "/workspace/main.txt");
    assert_eq!(rows[0].disposition, CloudLaneEgressDisposition::Uploaded);
    assert_eq!(rows[1].path, "/workspace/.env");
    assert_eq!(
        rows[1].disposition,
        CloudLaneEgressDisposition::Withheld,
        "withheld scopes are rendered too: 'what did it keep back' is the          question a user asks to decide whether to trust 'what is it sending'"
    );
    assert_eq!(
        rows.iter().map(|row| row.ordinal).collect::<Vec<_>>(),
        vec![1, 2]
    );

    let lines = view.rendered_lines();
    assert!(lines[0].contains("12288 bytes"), "line was: {}", lines[0]);
    assert!(
        lines[0].contains("50 of max 75 cents"),
        "line was: {}",
        lines[0]
    );
    assert!(lines[0].contains("hard_cap=true"), "line was: {}", lines[0]);
    assert_eq!(lines.len(), 3, "a summary line plus one line per scope");
}

#[test]
fn an_acknowledgement_covers_only_the_manifest_it_was_shown_for() {
    let original = cloud_lane_task_request(WorkspaceId(11));
    let acknowledgement = CloudLaneEgressManifestView::from_request(&original).acknowledge();
    assert!(acknowledgement.covers(&original));

    // The bait-and-switch this exists to stop: show a small manifest, then
    // submit a bigger one under the same task id.
    let mut swollen = original.clone();
    swollen.upload_manifest.allowed_files.push(extra_scope(
        "cloud-app-extra:key",
        "/workspace/secrets/key.pem",
    ));
    assert!(
        !acknowledgement.covers(&swollen),
        "an extra uploaded file must invalidate the acknowledgement"
    );

    let mut cheaper_looking = original.clone();
    cheaper_looking.budget.estimated_cost_cents = 74;
    assert!(
        !acknowledgement.covers(&cheaper_looking),
        "the cost the user was shown is part of what they agreed to"
    );

    let mut unenforced_cap = original.clone();
    unenforced_cap.budget.hard_cap_enforced = false;
    assert!(
        !acknowledgement.covers(&unenforced_cap),
        "a cap that stops being enforced changes the bargain"
    );

    let mut raised_cap = original.clone();
    raised_cap.budget.max_cost_cents = 7_500;
    assert!(
        !acknowledgement.covers(&raised_cap),
        "the cap shown was 75 cents; submitting under a 7500 cent cap is the          same bait-and-switch as swapping the file list"
    );

    let mut fewer_bytes = original.clone();
    fewer_bytes.upload_manifest.total_upload_bytes = 1;
    assert!(!acknowledgement.covers(&fewer_bytes));

    let mut other_task = original.clone();
    other_task.task_id = LegionCloudLaneTaskId("cloud-task:app:2".to_string());
    assert!(!acknowledgement.covers(&other_task));
}

#[test]
fn a_withheld_file_moving_into_the_upload_invalidates_the_acknowledgement() {
    let original = cloud_lane_task_request(WorkspaceId(11));
    let acknowledgement = CloudLaneEgressManifestView::from_request(&original).acknowledge();

    // `.env` was shown as withheld. Promoting it to allowed keeps the row
    // count and the byte total identical, so only a digest over dispositions
    // catches it -- a count-based check would wave this through.
    let mut promoted = original.clone();
    promoted.upload_manifest.forbidden_files.clear();
    promoted
        .upload_manifest
        .allowed_files
        .push(extra_scope("cloud-app-forbidden:env", "/workspace/.env"));
    assert_eq!(
        promoted.upload_manifest.allowed_files.len()
            + promoted.upload_manifest.forbidden_files.len(),
        original.upload_manifest.allowed_files.len()
            + original.upload_manifest.forbidden_files.len(),
        "the fixture must keep the row count equal or this test proves nothing"
    );
    assert!(
        !acknowledgement.covers(&promoted),
        "a withheld file becoming an uploaded one must invalidate it"
    );
}

#[test]
fn submit_is_refused_without_an_acknowledgement_for_that_exact_upload() {
    let workspace = temp_workspace();
    let (mut app, workspace_id) = automate_app_with_workspace(workspace.path());
    app.enable_legion_cloud_lane_runtime("https://cloud.legion.invalid", 75, 32_768)
        .expect("enable cloud lane");

    let shown = cloud_lane_task_request(workspace_id);
    let acknowledgement = CloudLaneEgressManifestView::from_request(&shown).acknowledge();

    let mut swollen = shown.clone();
    swollen.upload_manifest.allowed_files.push(extra_scope(
        "cloud-app-extra:key",
        "/workspace/secrets/key.pem",
    ));
    let error = app
        .submit_legion_cloud_lane_task(swollen, &acknowledgement)
        .expect_err("an upload the user never saw must be refused");
    assert!(
        error
            .to_string()
            .contains("was not surfaced for this exact upload"),
        "refusal should name the cause, got: {error}"
    );
    assert!(
        app.legion_cloud_lane_projection().rows.is_empty(),
        "a refused submit must not create a task row"
    );

    // The manifest that was actually shown still submits.
    let status = app
        .submit_legion_cloud_lane_task(shown, &acknowledgement)
        .expect("the acknowledged upload submits");
    assert_eq!(status.state, LegionCloudLaneTaskState::Submitted);
}

#[test]
fn an_in_flight_task_can_be_cancelled_and_a_finished_one_cannot() {
    let workspace = temp_workspace();
    let (mut app, workspace_id) = automate_app_with_workspace(workspace.path());
    app.enable_legion_cloud_lane_runtime("https://cloud.legion.invalid", 75, 32_768)
        .expect("enable cloud lane");

    let submitted = cloud_lane_task_request(workspace_id);
    let acknowledgement = CloudLaneEgressManifestView::from_request(&submitted).acknowledge();
    app.submit_legion_cloud_lane_task(submitted, &acknowledgement)
        .expect("submit");

    let task_id = LegionCloudLaneTaskId("cloud-task:app:1".to_string());
    let status = app
        .cancel_legion_cloud_lane_task(&task_id, "user changed their mind")
        .expect("an in-flight task cancels");
    assert_eq!(status.state, LegionCloudLaneTaskState::Cancelled);
    assert!(status.status_label.contains("user changed their mind"));

    let projection = app.legion_cloud_lane_projection();
    assert_eq!(
        projection.rows[0].state,
        LegionCloudLaneTaskState::Cancelled
    );

    // Cancelling again must fail rather than report a second success. A cancel
    // that "succeeds" against a finished upload tells the user their data was
    // withheld when it has already left.
    let error = app
        .cancel_legion_cloud_lane_task(&task_id, "again")
        .expect_err("a cancelled task cannot be cancelled again");
    assert!(
        error.to_string().contains("cannot be cancelled"),
        "got: {error}"
    );
}

#[test]
fn cancelling_an_unknown_task_is_refused() {
    let workspace = temp_workspace();
    let (mut app, _workspace_id) = automate_app_with_workspace(workspace.path());
    app.enable_legion_cloud_lane_runtime("https://cloud.legion.invalid", 75, 32_768)
        .expect("enable cloud lane");

    let error = app
        .cancel_legion_cloud_lane_task(&LegionCloudLaneTaskId("task:nope".to_string()), "reason")
        .expect_err("an untracked task must not report a successful cancel");
    assert!(error.to_string().contains("is not tracked"), "got: {error}");
}

#[test]
fn a_cancellation_reason_is_required() {
    let workspace = temp_workspace();
    let (mut app, workspace_id) = automate_app_with_workspace(workspace.path());
    app.enable_legion_cloud_lane_runtime("https://cloud.legion.invalid", 75, 32_768)
        .expect("enable cloud lane");
    let submitted = cloud_lane_task_request(workspace_id);
    let acknowledgement = CloudLaneEgressManifestView::from_request(&submitted).acknowledge();
    app.submit_legion_cloud_lane_task(submitted, &acknowledgement)
        .expect("submit");

    let error = app
        .cancel_legion_cloud_lane_task(
            &LegionCloudLaneTaskId("cloud-task:app:1".to_string()),
            "   ",
        )
        .expect_err("a blank reason must be refused");
    assert!(
        error.to_string().contains("must be non-empty"),
        "got: {error}"
    );
}

#[test]
fn the_projection_carries_the_manifest_visibility_flag_into_the_shell() {
    let workspace = temp_workspace();
    let (mut app, workspace_id) = automate_app_with_workspace(workspace.path());
    app.enable_legion_cloud_lane_runtime("https://cloud.legion.invalid", 75, 32_768)
        .expect("enable cloud lane");
    let submitted = cloud_lane_task_request(workspace_id);
    let acknowledgement = CloudLaneEgressManifestView::from_request(&submitted).acknowledge();
    app.submit_legion_cloud_lane_task(submitted, &acknowledgement)
        .expect("submit");

    let snapshot = app
        .shell_projection_snapshot("Cloud Lane")
        .expect("projection snapshot");
    assert!(snapshot.legion_cloud_lane.runtime_enabled);
    assert_eq!(snapshot.legion_cloud_lane.rows.len(), 1);
    assert!(
        snapshot.legion_cloud_lane.rows[0].scope_visible_to_user,
        "the shell must be able to show whether the scope was surfaced"
    );
}

#[test]
fn app_composition_exposes_the_manifest_builder_a_renderer_needs() {
    let request = cloud_lane_task_request(WorkspaceId(11));
    let view = AppComposition::legion_cloud_lane_egress_manifest(&request);
    assert_eq!(view.task_id().0, "cloud-task:app:1");
    assert_eq!(view.total_upload_bytes(), 12_288);
    assert!(view.digest().starts_with("sha256:"));
}
