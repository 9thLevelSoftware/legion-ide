//! A signed org policy bundle's mode ceiling gates the app's mode switch (P9.F2.T3).
//!
//! The bundle already refuses above-ceiling requests per capability. That is the
//! security boundary, but on its own it leaves the product in a mode it cannot
//! act in — the user flips to Delegate and every request quietly fails. These
//! tests hold the mode switch itself to the ceiling.

use legion_app::{AppComposition, AppProductMode};
use legion_security::{
    PolicyKeyring, PolicySigningKey, VerifiedPolicyBundle, policy_bundle_verifying_key_b64,
    sign_policy_bundle,
};

const ORG_SEED: [u8; 32] = [7u8; 32];
const ORG_KEY_ID: &str = "org-policy-signer-1";

/// The shipped restrictive enterprise example: `mode_ceiling = "Assist"`.
fn verified_enterprise_bundle() -> VerifiedPolicyBundle {
    let payload = include_str!("../../../xtask/legion-policy.example.toml");
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: ORG_KEY_ID.to_string(),
        verifying_key_b64: policy_bundle_verifying_key_b64(&ORG_SEED),
    }]);
    sign_policy_bundle(payload, ORG_KEY_ID, &ORG_SEED)
        .verify(&keyring)
        .expect("the shipped enterprise example must verify")
}

#[test]
fn a_mode_above_the_ceiling_is_refused() {
    let mut app = AppComposition::new();
    app.set_org_policy_bundle(verified_enterprise_bundle());

    app.set_product_mode(AppProductMode::Delegate);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Manual,
        "Delegate is above the Assist ceiling and must not take effect"
    );

    app.set_product_mode(AppProductMode::Automate);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Manual,
        "Automate is above the Assist ceiling and must not take effect"
    );
}

#[test]
fn a_mode_at_or_below_the_ceiling_is_permitted() {
    // Non-vacuity: the refusals above must be the ceiling talking, not a bundle
    // that blocks every mode change.
    let mut app = AppComposition::new();
    app.set_org_policy_bundle(verified_enterprise_bundle());

    app.set_product_mode(AppProductMode::Assist);
    assert_eq!(app.product_mode(), AppProductMode::Assist);

    app.set_product_mode(AppProductMode::Manual);
    assert_eq!(app.product_mode(), AppProductMode::Manual);
}

#[test]
fn installing_a_bundle_lowers_a_mode_that_is_already_above_the_ceiling() {
    // An org that pushes a bundle mid-session must not have to wait for the
    // user's next mode switch for it to bite.
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    assert_eq!(
        app.product_mode(),
        AppProductMode::Delegate,
        "without a bundle, Delegate is reachable"
    );

    app.set_org_policy_bundle(verified_enterprise_bundle());
    assert_eq!(
        app.product_mode(),
        AppProductMode::Manual,
        "installing an Assist-ceiling bundle must lower a Delegate session"
    );
}

#[test]
fn without_a_bundle_no_ceiling_applies() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Automate);
    assert_eq!(app.product_mode(), AppProductMode::Automate);
    assert!(!app.org_policy_mode_ceiling_denies(AppProductMode::Automate));
}

#[test]
fn the_ceiling_predicate_matches_the_bundle() {
    let mut app = AppComposition::new();
    app.set_org_policy_bundle(verified_enterprise_bundle());

    assert!(!app.org_policy_mode_ceiling_denies(AppProductMode::Manual));
    assert!(!app.org_policy_mode_ceiling_denies(AppProductMode::Assist));
    assert!(app.org_policy_mode_ceiling_denies(AppProductMode::Delegate));
    assert!(app.org_policy_mode_ceiling_denies(AppProductMode::Automate));
}
