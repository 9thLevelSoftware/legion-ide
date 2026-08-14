# ADR-0047: Extension Distribution Strategy

## Status

Accepted

## Context

Legion has a functional WASM plugin runtime (`legion-plugin`) with Wasmtime-based
sandboxing, deny-by-default capability brokering, per-invocation quota enforcement,
and hostile fixture probes. The `SignedExtensionRegistry` validates signed manifests
with ed25519 signatures before install, rejecting unsigned artifacts by default. WIT
interfaces exist for grammars, themes, and LSP adapters. The manifest permission
review system (`manifest.rs`) generates structured permission-review rows for
install-time approval covering commands, tree-sitter grammars, formatters, language
providers, LSP registrations, workspace scanners, and AI context providers.

The VS Code compatibility layer (`legion-vscode-compat`) classifies VS Code
extension manifests into four tiers:

- **Tier 0 (Declarative)**: themes, icon themes, snippets, keybindings, languages,
  grammars -- supported without a runtime host.
- **Tier 1 (Protocol Adapter)**: commands, configuration, menus, debuggers, tasks,
  tests, SCM -- supported with policy.
- **Tier 2 (Extension Host Sidecar)**: views, startup-finished activation --
  supported with policy, requires Node or web-worker sidecar.
- **Tier 3 (Webview/Notebook/Custom Editor)**: deferred.

The compat layer also resolves Open VSX registry metadata (namespace, version,
HTTPS download URL) and loads package manifests into normalized compatibility DTOs,
but does not execute VSIX contents or grant any runtime authority.

Despite this infrastructure, there is **no distribution channel**. Every Cargo.toml
is `license = "Proprietary"`, `publish = false`. The course correction plan (W7)
identifies this as an unconsidered default: "Proprietary + no marketplace = no
extension ecosystem at a time it still matters." Extension breadth remains the
single biggest reason users stay on VS Code, and even Zed's ~1,000-extension
library struggles against VS Code's 100K. The question is not whether to build a
full marketplace now -- Master Plan v0.2 explicitly defers that -- but to make a
deliberate decision about the v1 distribution channel so the existing runtime has a
delivery mechanism.

## Options Considered

### Option A: Enterprise Curated Signed-Allowlist

Extensions distributed as signed WASM bundles via internal registries (corporate
artifact stores, air-gapped mirrors). A publisher-maintained allowlist of approved
extension manifest IDs and SHA-256 hashes gates what the runtime will load.

- Leverages existing `SignedExtensionRegistry` directly.
- Fits enterprise-trust positioning and air-gap mode (ADR-0010).
- Limitation: no community growth engine. Extension breadth depends entirely on
  first-party investment.

### Option B: Community Open VSX Read-Only

Read-only consumption of the Open VSX registry for all extension types. The
existing `resolve_open_vsx_extension_metadata` and `load_open_vsx_extension`
functions already normalize Open VSX responses into compatibility DTOs.

- Opens a community channel to the ~4,000 Open VSX extensions.
- Limitation: runtime extensions (Node sidecar, Tier 2+) remain unsupported in
  v1. Only Tier 0 declarative content is usable without additional execution
  infrastructure. Conflates metadata-only and runtime-capable extensions in a
  single channel, creating user confusion about what "install" means.

### Option C: Hybrid Tiered Distribution (Recommended)

Two tiers with distinct trust requirements, matching the infrastructure that
already exists:

- **Tier 1 -- Signed WASM bundles for runtime extensions** (commands, formatters,
  LSP adapters, workspace scanners, AI context providers). Distributed via
  enterprise registries or vendored bundles. Requires publisher signing (ed25519
  via `PluginSignatureMetadata`). Uses `SignedExtensionRegistry` for
  install/update/remove lifecycle. Subject to manifest permission review and
  deny-by-default capability brokering.

- **Tier 2 -- Open VSX read-only for metadata-only contributions** (themes,
  grammars, keymaps, snippets, icon themes). Uses existing
  `legion-vscode-compat` classification to filter to `Tier0Declarative`
  contributions only. No publisher signing required since these contributions
  carry no runtime authority. HTTPS-only download URLs enforced by existing
  validation.

Both tiers pass through deny-by-default capability review (existing manifest
permission review). The runtime host (`PluginRuntimeHost`) enforces trust
metadata, ABI version compatibility, quota declarations, and capability
brokering regardless of distribution tier.

## Decision

Option C (Hybrid Tiered Distribution).

This matches the existing infrastructure without requiring new security
primitives. Tier 1 uses the `SignedExtensionRegistry` and `WasmPluginHost` that
are already built and tested. Tier 2 uses the `legion-vscode-compat` tier
classification and Open VSX metadata resolution that are already built and
tested. The boundary between tiers is the same boundary the codebase already
enforces: signed + trusted manifests for runtime authority, metadata-only
classification for declarative contributions.

## v1 Distribution Channel

The v1 distribution channel requires no centralized marketplace infrastructure.

**For Tier 1 (signed runtime extensions):**

A TOML manifest file (`extensions.toml`) listing approved extensions:

```toml
[extensions.legion-rust-analyzer]
manifest_id = "legion.rust-analyzer"
version = "0.1.0"
module_hash = "sha256:abcdef..."
signature_signer = "legion-publisher"
artifact_uri = "file://extensions/legion-rust-analyzer-0.1.0.wasm"

[extensions.legion-formatter]
manifest_id = "legion.formatter"
version = "0.1.0"
module_hash = "sha256:123456..."
signature_signer = "legion-publisher"
artifact_uri = "file://extensions/legion-formatter-0.1.0.wasm"
```

This file can be vendored in the install directory, served from an internal
artifact registry, or fetched from a versioned HTTPS endpoint. The existing
`SignedExtensionRegistry` validates each entry before loading. No new trust
primitives are needed.

**For Tier 2 (Open VSX declarative content):**

A read-only Open VSX client that queries the public registry (or a self-hosted
mirror for air-gapped deployments) and filters results to
`VsCodeCompatibilityTier::Tier0Declarative` before presenting them in the
extensions panel. The existing `resolve_open_vsx_extension_metadata` function
validates HTTPS download URLs. Only theme, grammar, snippet, keybinding,
language, and icon-theme contributions are surfaced; anything requiring a runtime
host is filtered out and not offered for install.

## Consequences

- Enterprise teams get auditable, signed runtime extensions with full capability
  review, quota enforcement, and deny-by-default brokering. Air-gap mode works
  with vendored bundles.
- Community contributors get a low-friction path for themes, grammars, and
  keymaps without publisher signing overhead.
- No marketplace infrastructure is required for v1. A centralized marketplace
  can be built later on top of this foundation without changing the trust model.
- The distribution tiers are explicitly named and documented, preventing the
  "unconsidered default" that W7 identified.
- The decision is revisable: promoting Tier 2 to support runtime extensions, or
  adding a community signing program, are incremental additions to the existing
  model.
- `legion-vscode-compat` remains metadata-only per ADR-0046 (surface expansion
  freeze). Product activation of the Open VSX read-only channel is gated on
  PR-UI-001 reaching "product workflow validated."

## References

- `crates/legion-plugin/src/host.rs` -- Wasmtime sandbox with deny-by-default
  capability brokering, quota enforcement, and hostile fixture probes
- `crates/legion-plugin/src/registry.rs` -- `SignedExtensionRegistry` with
  ed25519 signature validation
- `crates/legion-plugin/src/manifest.rs` -- Manifest permission review for
  install-time approval
- `crates/legion-plugin/wit/` -- WIT interfaces for grammars, themes, LSP
  adapters
- `crates/legion-vscode-compat/src/lib.rs` -- Tier 0-3 classification, Open VSX
  metadata resolution, HTTPS-only enforcement
- Course Correction Plan W7: "Proprietary + no marketplace = no extension
  ecosystem at a time it still matters"
- Master Plan v0.2 WS-EXT-01: extension and compatibility workstream
- Master Plan v0.2 section 4.3: "Do not claim VS Code marketplace compatibility
  beyond the implemented Open VSX/manifest/contribution surface"
- ADR-0019: WASM plugin runtime
- ADR-0046: Surface expansion freeze
