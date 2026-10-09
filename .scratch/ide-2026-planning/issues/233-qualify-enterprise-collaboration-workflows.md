# 233: Qualify enterprise collaboration workflows

Status: ready-for-agent

**What to build:** Observe the declared two-client/service/identity native workflows and recovery against real services, with independent tenant/role/isolation evidence and no fixture acceptance.

**Blocked by:** [26 — Run the five-working-day Manual pilot](26-run-the-five-working-day-manual-pilot.md); [215 — Reconcile concurrent edits in a shared document](215-reconcile-concurrent-edits-in-a-shared-document.md); [216 — Review and apply a shared proposal](216-review-and-apply-a-shared-proposal.md); [218 — Provision and deprovision an enterprise account](218-provision-and-deprovision-an-enterprise-account.md); [219 — Roll out and revoke signed enterprise policy](219-roll-out-and-revoke-signed-enterprise-policy.md); [220 — Export and delete tenant audit data](220-export-and-delete-tenant-audit-data.md); [221 — Preview, upload and revoke telemetry consent](221-preview-upload-and-revoke-telemetry-consent.md); [223 — Inspect current security and deployment feedback](223-inspect-current-security-and-deployment-feedback.md); [225 — Restore a team-service backup](225-restore-a-team-service-backup.md); [226 — Upgrade, roll back and remove the team service](226-upgrade-roll-back-and-remove-the-team-service.md); [227 — Manage SCIM groups and memberships](227-manage-scim-groups-and-memberships.md); [228 — Expose complete bounded SCIM discovery and queries](228-expose-complete-bounded-scim-discovery-and-queries.md); [229 — Enforce identity and collaboration policy](229-enforce-identity-and-collaboration-policy.md); [230 — Enforce remote environment policy](230-enforce-remote-environment-policy.md); [231 — Enforce telemetry and export policy](231-enforce-telemetry-and-export-policy.md); [232 — Enforce policy expiry and anti-downgrade](232-enforce-policy-expiry-and-anti-downgrade.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 233.

**Traceability:** M5; S5-15. **Verification target:** A18/A19.

## Acceptance criteria

- [ ] Observe the declared two-client/service/identity native workflows and recovery against real services, with independent tenant/role/isolation evidence and no fixture acceptance.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p xtask --test completion_evidence`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
