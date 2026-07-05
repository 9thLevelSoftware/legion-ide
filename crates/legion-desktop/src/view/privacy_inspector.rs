use legion_ui::ShellProjectionSnapshot;

use super::bounded_join;

pub(crate) fn post_run_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let privacy = &snapshot.privacy_inspector_projection;
    let manifest_id = privacy.manifest_id.as_deref().unwrap_or("<none>");
    let proposal_id = privacy
        .proposal_id
        .as_ref()
        .map(|proposal_id| proposal_id.0.to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let workspace_id = privacy
        .workspace_id
        .as_ref()
        .map(|workspace_id| workspace_id.0.to_string())
        .unwrap_or_else(|| "<none>".to_string());

    let mut rows = vec![format!(
        "privacy inspector {}: retention_handle=run:{} deletion_handle=delete:{} manifest={} proposal={} workspace={} records={} denied={} redacted={} external={} high-risk={}",
        privacy.inspector_id,
        privacy.inspector_id,
        privacy.inspector_id,
        manifest_id,
        proposal_id,
        workspace_id,
        privacy.records.len(),
        privacy.denied_record_count,
        privacy.redacted_record_count,
        privacy.external_egress_record_count,
        privacy.high_risk_record_count,
    )];

    if privacy.records.is_empty() && privacy.refusal.is_none() {
        rows.push("privacy inspector: no post-run exposure rows projected".to_string());
        return rows;
    }

    rows.extend(privacy.records.iter().take(10).map(|record| {
        let retention_handle = format!(
            "run:{}:record:{}",
            privacy.inspector_id, record.exposure_id
        );
        let deletion_handle = format!(
            "delete:{}:record:{}",
            privacy.inspector_id, record.exposure_id
        );
        let permission_label = record
            .permission_label
            .as_ref()
            .map(|capability| capability.0.as_str())
            .unwrap_or("<none>");
        format!(
            "privacy record {}: retention_handle={} deletion_handle={} source={:?} redaction={:?} privacy={:?} inclusion={:?} egress={:?} risk={:?} permission={} labels={} reasons={}",
            record.exposure_id,
            retention_handle,
            deletion_handle,
            record.source_kind,
            record.redaction_state,
            record.privacy_label,
            record.inclusion,
            record.egress,
            record.risk_label,
            permission_label,
            bounded_join(&record.labels),
            bounded_join(&record.reasons),
        )
    }));

    if let Some(refusal) = &privacy.refusal {
        rows.push(format!(
            "privacy refusal {}: {} scope={:?} capability={} risk={:?} reasons={}",
            refusal.reason_code,
            refusal.label,
            refusal.privacy_scope,
            refusal
                .capability
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            refusal.risk_label,
            bounded_join(&refusal.reasons)
        ));
    }

    rows
}
