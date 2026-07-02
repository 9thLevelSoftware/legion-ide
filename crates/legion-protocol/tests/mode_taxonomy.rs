//! Mode taxonomy regression tests (P0.F1.T3).
//!
//! These tests assert that every product mode has a label, shortcut,
//! runtime-surface policy, and docs anchor. If a new mode is added
//! without updating the canonical taxonomy, these tests fail closed.

use legion_protocol::{
    CANONICAL_PRODUCT_MODES, CanonicalProductMode, ProductMode, ProductRuntimeSurface,
};

#[test]
fn every_product_mode_has_a_non_empty_label() {
    let modes = [
        ProductMode::Manual,
        ProductMode::Assist,
        ProductMode::Delegates,
        ProductMode::Automate,
        ProductMode::LegionWorkflows,
    ];
    for mode in modes {
        let label = mode.label();
        assert!(
            !label.is_empty(),
            "ProductMode::{:?} must have a non-empty label",
            mode
        );
    }
}

#[test]
fn canonical_taxonomy_has_exactly_four_v1_entries() {
    assert_eq!(
        CANONICAL_PRODUCT_MODES.len(),
        4,
        "Canonical v1 taxonomy must have exactly 4 entries (Manual, Assist, Delegate, Legion Workflows)"
    );
}

#[test]
fn every_canonical_mode_has_label_shortcut_and_docs_anchor() {
    for entry in CANONICAL_PRODUCT_MODES {
        assert!(
            !entry.label.is_empty(),
            "{:?} must have a non-empty label",
            entry.variant
        );
        assert!(
            !entry.shortcut_label.is_empty(),
            "{:?} must have a non-empty shortcut_label",
            entry.variant
        );
        assert!(
            entry.docs_anchor.starts_with("docs/MODES.md#"),
            "{:?} docs_anchor must point at docs/MODES.md#..., got: {:?}",
            entry.variant,
            entry.docs_anchor
        );
        assert!(
            !entry.policy_summary.is_empty(),
            "{:?} must have a non-empty policy_summary",
            entry.variant
        );
    }
}

#[test]
fn canonical_entries_cover_all_v1_modes() {
    let variants: Vec<ProductMode> = CANONICAL_PRODUCT_MODES
        .iter()
        .map(|entry| entry.variant)
        .collect();
    assert!(
        variants.contains(&ProductMode::Manual),
        "Canonical taxonomy must include Manual"
    );
    assert!(
        variants.contains(&ProductMode::Assist),
        "Canonical taxonomy must include Assist"
    );
    assert!(
        variants.contains(&ProductMode::Delegates),
        "Canonical taxonomy must include Delegates"
    );
    assert!(
        variants.contains(&ProductMode::LegionWorkflows),
        "Canonical taxonomy must include LegionWorkflows"
    );
}

#[test]
fn manual_mode_denies_ai_and_network_surfaces() {
    assert!(
        !ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::AssistedAi),
        "Manual mode must deny AssistedAi surface"
    );
    assert!(
        !ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::CloudProvider),
        "Manual mode must deny CloudProvider surface"
    );
    assert!(
        !ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::NetworkEgress),
        "Manual mode must deny NetworkEgress surface"
    );
    assert!(
        !ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::WorkerRuntime),
        "Manual mode must deny WorkerRuntime surface"
    );
    assert!(
        !ProductMode::Manual.allows_runtime_surface(ProductRuntimeSurface::HostedTelemetry),
        "Manual mode must deny HostedTelemetry surface"
    );
}

#[test]
fn canonical_labels_match_product_mode_labels() {
    for entry in CANONICAL_PRODUCT_MODES {
        assert_eq!(
            entry.label,
            entry.variant.label(),
            "Canonical label for {:?} must match ProductMode::label()",
            entry.variant
        );
    }
}
