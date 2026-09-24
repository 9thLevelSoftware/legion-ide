//! Drift guard for the disclosed Node-minimum divergence between the pinned
//! TypeScript archives in `legion-lsp` and the offline bundle descriptor in
//! `legion-app`.
//!
//! The two crates describe the same two npm archives:
//!
//! * `legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE` and
//!   `legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE` are the registry's pinned
//!   catalog identities, and each records the `engines.node` constraint its own
//!   release declares — `22.22.2` for the language server, `14.17.0` for the
//!   compiler.
//! * `legion_app::language::TypeScriptBundleDescriptor::pinned()` builds both
//!   of its descriptors from a single shared `LspArtifactRuntime::Node`, so it
//!   gives the compiler `22.22.2` as well.
//!
//! The compiler's Node minimum therefore differs between the two crates today.
//! That divergence is **not resolved here.** `ArtifactDescriptor::runtime` is
//! part of the input to `ArtifactDescriptor::from_metadata`, and the
//! materializer's cache manifest records the runtime identity, so changing the
//! bundle's compiler minimum changes the artifact's receipt identity. That is a
//! behavioural change to the artifact materializer and needs its own packet
//! with its own blast-radius analysis; `typescript_bundle.rs` is deliberately
//! untouched by the packet that added this file.
//!
//! What a future packet must do: pick one authority for the compiler's Node
//! minimum (most likely the `legion-lsp` constant, which records what the
//! published release actually declares), change
//! `TypeScriptBundleDescriptor::pinned()` to read it, and then re-derive and
//! re-record every materializer receipt that embeds `node:22.22.2` for the
//! `typescript` artifact. Until then, this test pins **both absolute values**
//! so that moving either side alone turns it red rather than letting the two
//! drift silently together or apart.

use legion_app::language::TypeScriptBundleDescriptor;
use legion_lsp::{LspArtifactRuntime, LspNodeVersion};

/// Reads the Node minimum out of a runtime descriptor.
fn node_minimum(runtime: &LspArtifactRuntime) -> LspNodeVersion {
    let LspArtifactRuntime::Node { minimum_version } = runtime;
    *minimum_version
}

/// Builds the expected version literal, spelled out at each call site so the
/// assertion carries the number rather than a reference to it.
fn version(major: u32, minor: u32, patch: u32) -> LspNodeVersion {
    LspNodeVersion {
        major,
        minor,
        patch,
    }
}

/// The language server's Node minimum is `22.22.2` on both sides, and this
/// test fails if either side moves.
///
/// This is the half of the pair that currently agrees. It is asserted as two
/// independent literal comparisons, not as an equality between the two crates,
/// so a change that moved both sides together would still be caught.
#[test]
fn pinned_language_server_node_minimum_is_22_22_2_in_both_crates() {
    let registry_minimum = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.minimum_node;
    assert_eq!(
        registry_minimum,
        version(22, 22, 2),
        "legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.minimum_node moved to \
         {registry_minimum:?}; it records the engines.node constraint of the \
         pinned typescript-language-server 6.0.0 release. If the pinned release \
         changed, TypeScriptBundleDescriptor::pinned() must be reconsidered in \
         the same change and this literal updated deliberately."
    );

    let bundle = TypeScriptBundleDescriptor::pinned();
    let bundle_minimum = node_minimum(&bundle.server.runtime);
    assert_eq!(
        bundle_minimum,
        version(22, 22, 2),
        "TypeScriptBundleDescriptor::pinned().server Node minimum moved to \
         {bundle_minimum:?}; legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE \
         still records 22.22.2, and the descriptor's runtime is part of the \
         materializer receipt identity."
    );

    // The two sides agree today. Asserted last, so a failure above names the
    // side that actually moved instead of only reporting a mismatch.
    assert_eq!(registry_minimum, bundle_minimum);
}

/// The compiler's Node minimum is `14.17.0` in `legion-lsp` and `22.22.2` in
/// `legion-app`, and this test fails if either side moves — including if
/// someone unifies them without doing the receipt-identity work.
///
/// A test that asserted only "these two differ" would pass while both values
/// changed, so each is pinned to its own literal first and the inequality is
/// derived from them.
#[test]
fn pinned_compiler_node_minimum_diverges_between_legion_lsp_and_legion_app() {
    let registry_minimum = legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.minimum_node;
    assert_eq!(
        registry_minimum,
        version(14, 17, 0),
        "legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.minimum_node moved to \
         {registry_minimum:?}. This is the disclosed divergence from \
         TypeScriptBundleDescriptor::pinned() (22.22.2); moving one side alone \
         is exactly what this guard exists to stop. Resolving the divergence \
         changes ArtifactDescriptor::from_metadata input and therefore the \
         compiler artifact's receipt identity, so it needs its own packet."
    );

    let bundle = TypeScriptBundleDescriptor::pinned();
    let bundle_minimum = node_minimum(&bundle.compiler.runtime);
    assert_eq!(
        bundle_minimum,
        version(22, 22, 2),
        "TypeScriptBundleDescriptor::pinned().compiler Node minimum moved to \
         {bundle_minimum:?}. legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE still \
         records 14.17.0. If this was an intentional unification, the compiler \
         artifact's materializer receipt identity changed with it and every \
         recorded receipt must be re-derived in the same change."
    );

    // Both literals are pinned above, so this states the consequence rather
    // than being the guard itself.
    assert_ne!(
        registry_minimum, bundle_minimum,
        "the compiler Node minimum is expected to diverge between legion-lsp \
         and legion-app until a dedicated packet unifies it; if it no longer \
         diverges, delete this guard in the packet that unified it rather than \
         relaxing it here"
    );

    // The identity fields that are *not* divergent stay asserted, so this test
    // fails loudly if the two crates stop describing the same archive at all.
    assert_eq!(
        bundle.compiler.package_name,
        legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.package_name
    );
    assert_eq!(
        bundle.compiler.version,
        legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.version
    );
    assert_eq!(
        bundle.compiler.expected_sha256,
        legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.checksum_sha256
    );
    assert_eq!(
        bundle.compiler.entrypoint,
        std::path::Path::new(legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE.entrypoint)
    );
}
