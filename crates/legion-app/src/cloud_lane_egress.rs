//! Surfacing and acknowledgement for Legion Cloud Lane egress manifests.
//!
//! P9.F3.T3's stop condition is "stop if a Cloud Lane upload can complete
//! without surfacing its manifest". The contract layer already refused a
//! manifest whose `scope_visible_to_user` was false, and the security broker
//! already denied `cloud.lane.submit` without it — but that flag is set by the
//! caller. It records a claim that the scope was shown; nothing connected it to
//! anything having been shown. A submit path could set it to `true` and upload
//! whatever it liked, passing every check.
//!
//! This module makes the claim cost something. [`CloudLaneEgressManifestView`]
//! is what a renderer draws, and [`CloudLaneEgressManifestView::acknowledge`]
//! is the only way to obtain a [`CloudLaneEgressAcknowledgement`], which
//! `AppComposition::submit_legion_cloud_lane_task` now requires.
//!
//! ## Why the acknowledgement carries a digest
//!
//! Binding it to the task id alone would stop nothing: show a two-file manifest,
//! acknowledge it, then submit a request whose manifest lists two hundred. The
//! acknowledgement therefore carries a digest over the manifest's *contents* —
//! every allowed and forbidden scope, the byte total, the scan status, and the
//! budget the user was shown. Change any of them after acknowledgement and the
//! digest no longer matches, so the submit is refused.
//!
//! The digest is length-prefixed for the same reason the extension signing
//! payload is (P7.F2): a delimiter-joined encoding lets one field's contents
//! impersonate a field boundary, so two different manifests can produce
//! identical bytes.

use legion_protocol::{
    LegionCloudLaneSecretScanStatus, LegionCloudLaneTaskId, LegionCloudLaneTaskRequest,
    LegionTaskFileScope,
};
use sha2::{Digest, Sha256};

/// Whether a scope is permitted to leave the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudLaneEgressDisposition {
    /// The scope is part of the upload.
    Uploaded,
    /// The scope is explicitly withheld.
    Withheld,
}

impl CloudLaneEgressDisposition {
    /// Display label for the disposition.
    pub fn label(self) -> &'static str {
        match self {
            Self::Uploaded => "uploaded",
            Self::Withheld => "withheld",
        }
    }
}

/// One line of the egress manifest as a user sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudLaneEgressRow {
    /// 1-based position in the rendered manifest.
    pub ordinal: u32,
    /// Display-safe scope path.
    pub path: String,
    /// Whether this scope leaves the machine.
    pub disposition: CloudLaneEgressDisposition,
}

/// The egress manifest a renderer must show before a submit is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudLaneEgressManifestView {
    task_id: LegionCloudLaneTaskId,
    manifest_id: String,
    rows: Vec<CloudLaneEgressRow>,
    total_upload_bytes: u64,
    estimated_cost_cents: u32,
    max_cost_cents: u32,
    hard_cap_enforced: bool,
    secret_scan_status: LegionCloudLaneSecretScanStatus,
    digest: String,
}

impl CloudLaneEgressManifestView {
    /// Build the view a renderer draws for one submission.
    pub fn from_request(request: &LegionCloudLaneTaskRequest) -> Self {
        let mut rows = Vec::with_capacity(
            request.upload_manifest.allowed_files.len()
                + request.upload_manifest.forbidden_files.len(),
        );
        // Withheld scopes are rendered too. A manifest that lists only what is
        // leaving answers "what am I sending?" but not "what did it decide to
        // keep back?", and the second question is the one a user asks when they
        // are deciding whether to trust the first answer.
        push_rows(
            &mut rows,
            &request.upload_manifest.allowed_files,
            CloudLaneEgressDisposition::Uploaded,
        );
        push_rows(
            &mut rows,
            &request.upload_manifest.forbidden_files,
            CloudLaneEgressDisposition::Withheld,
        );

        let digest = manifest_digest(request, &rows);
        Self {
            task_id: request.task_id.clone(),
            manifest_id: request.upload_manifest.manifest_id.clone(),
            rows,
            total_upload_bytes: request.upload_manifest.total_upload_bytes,
            estimated_cost_cents: request.budget.estimated_cost_cents,
            max_cost_cents: request.budget.max_cost_cents,
            hard_cap_enforced: request.budget.hard_cap_enforced,
            secret_scan_status: request.upload_manifest.secret_scan_status,
            digest,
        }
    }

    /// Task this manifest belongs to.
    pub fn task_id(&self) -> &LegionCloudLaneTaskId {
        &self.task_id
    }

    /// Manifest id.
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// Every scope, uploaded and withheld, in render order.
    pub fn rows(&self) -> &[CloudLaneEgressRow] {
        &self.rows
    }

    /// Total bytes leaving the machine.
    pub fn total_upload_bytes(&self) -> u64 {
        self.total_upload_bytes
    }

    /// Estimated cost shown to the user.
    pub fn estimated_cost_cents(&self) -> u32 {
        self.estimated_cost_cents
    }

    /// Hard cost cap shown to the user.
    pub fn max_cost_cents(&self) -> u32 {
        self.max_cost_cents
    }

    /// Whether the cap is enforced rather than advisory.
    pub fn hard_cap_enforced(&self) -> bool {
        self.hard_cap_enforced
    }

    /// Secret-scan disposition shown to the user.
    pub fn secret_scan_status(&self) -> LegionCloudLaneSecretScanStatus {
        self.secret_scan_status
    }

    /// Digest binding this acknowledgement to the manifest's contents.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Stable text rendering, one line per row plus a summary.
    ///
    /// This is what an accessibility projection and an audit both read, so the
    /// numbers here are the numbers the user saw.
    pub fn rendered_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(self.rows.len() + 1);
        lines.push(format!(
            "cloud egress {}: {} bytes, {} of max {} cents, hard_cap={}, secret_scan={:?}",
            self.task_id.0,
            self.total_upload_bytes,
            self.estimated_cost_cents,
            self.max_cost_cents,
            self.hard_cap_enforced,
            self.secret_scan_status
        ));
        for row in &self.rows {
            lines.push(format!(
                "cloud egress {} {}. {} — {}",
                self.task_id.0,
                row.ordinal,
                row.path,
                row.disposition.label()
            ));
        }
        lines
    }

    /// Record that this manifest was shown, yielding the submit proof.
    ///
    /// Deliberately takes `&self` on the *view*: there is no way to build an
    /// acknowledgement except from a view, and no way to build a view except
    /// from the request whose manifest it describes.
    pub fn acknowledge(&self) -> CloudLaneEgressAcknowledgement {
        CloudLaneEgressAcknowledgement {
            task_id: self.task_id.clone(),
            digest: self.digest.clone(),
        }
    }
}

/// Proof that a Cloud Lane egress manifest was surfaced before submission.
///
/// Constructible only through [`CloudLaneEgressManifestView::acknowledge`], so
/// a submit path that demands one cannot be reached without the manifest having
/// been built and rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudLaneEgressAcknowledgement {
    task_id: LegionCloudLaneTaskId,
    digest: String,
}

impl CloudLaneEgressAcknowledgement {
    /// Task this acknowledgement covers.
    pub fn task_id(&self) -> &LegionCloudLaneTaskId {
        &self.task_id
    }

    /// Digest of the manifest the user was shown.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Whether this acknowledgement covers the manifest in `request`.
    ///
    /// Both the task id and the content digest must match: the id alone would
    /// let an acknowledged two-file manifest authorise a two-hundred-file one.
    pub fn covers(&self, request: &LegionCloudLaneTaskRequest) -> bool {
        if self.task_id != request.task_id {
            return false;
        }
        let view = CloudLaneEgressManifestView::from_request(request);
        self.digest == view.digest
    }
}

fn push_rows(
    rows: &mut Vec<CloudLaneEgressRow>,
    scopes: &[LegionTaskFileScope],
    disposition: CloudLaneEgressDisposition,
) {
    for scope in scopes {
        rows.push(CloudLaneEgressRow {
            ordinal: u32::try_from(rows.len() + 1).unwrap_or(u32::MAX),
            path: scope.path.0.clone(),
            disposition,
        });
    }
}

/// Length-prefixed digest over everything the user was shown.
fn manifest_digest(request: &LegionCloudLaneTaskRequest, rows: &[CloudLaneEgressRow]) -> String {
    let mut hasher = Sha256::new();
    let mut field = |name: &str, value: &str| {
        hasher.update(name.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(name.as_bytes());
        hasher.update(value.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(value.as_bytes());
    };

    field("task_id", &request.task_id.0);
    field("manifest_id", &request.upload_manifest.manifest_id);
    field(
        "total_upload_bytes",
        &request.upload_manifest.total_upload_bytes.to_string(),
    );
    field(
        "secret_scan_status",
        &format!("{:?}", request.upload_manifest.secret_scan_status),
    );
    field(
        "contains_forbidden_material",
        &request
            .upload_manifest
            .contains_forbidden_material
            .to_string(),
    );
    field(
        "estimated_cost_cents",
        &request.budget.estimated_cost_cents.to_string(),
    );
    field("max_cost_cents", &request.budget.max_cost_cents.to_string());
    field(
        "hard_cap_enforced",
        &request.budget.hard_cap_enforced.to_string(),
    );
    field("row_count", &rows.len().to_string());
    for row in rows {
        field(
            &format!("row_{}", row.ordinal),
            &format!("{}|{}", row.disposition.label(), row.path),
        );
    }

    format!("sha256:{:x}", hasher.finalize())
}

/// A Cloud Lane operation routed from a shell intent.
///
/// Defined here rather than as another `AppCommandRequest` variant so the
/// chokepoint file gains one line instead of a family of them — the same reason
/// `ExtensionCatalogRequest` exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudLaneRequest {
    /// Cancel an in-flight task with a display-safe reason.
    CancelTask {
        /// Task id selected from projection data.
        task_id: LegionCloudLaneTaskId,
        /// Reason recorded with the cancellation.
        reason_label: String,
    },
}
