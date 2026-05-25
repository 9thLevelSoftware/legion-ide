# Repository Discovery, Ignore, And Fingerprint Evidence

Date: 2026-05-24

Accepted evidence:

- Protocol DTO coverage is tested by `dto_contracts_workspace_discovery_dtos_golden_and_required_fields`.
- Index importer coverage is tested by `repository_discovery_importer_accepts_workspace_dtos_and_never_scans_paths`.
- Deletion invalidation is tested by `repository_discovery_importer_invalidates_deleted_records_from_workspace_delta`.
- Workspace-authored discovery records carry skip decisions for generated, binary, vendored, oversized, deleted, policy-denied, external, hidden, and metadata-only cases.
- `RepositoryDiscoveryImporter` consumes `WorkspaceDiscoverySnapshot` and `WorkspaceDiscoveryDelta` DTOs and never receives a root path scanner configuration.
- Disk fingerprint and semantic content hash remain separate fields: workspace file fingerprints protect saves, while semantic content hashes drive cache and query invalidation.
- Metadata-only discovery records are retained without source bodies; excluded or deleted records invalidate known file ids before replacement records become authoritative.
