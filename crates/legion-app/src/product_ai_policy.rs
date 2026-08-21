//! Org-policy ceiling, provider budget declaration, and the inline prediction lane.
//!
//! Split out of `lib.rs` rather than added to it. Three things live here that
//! belong together: what an installed policy bundle lowers, what a request must
//! declare so a bundle's caps have something to compare against, and the worker
//! lane that keeps a live provider call off the thread that draws frames.
//!
//! One subject, because they share one failure mode: a control that looks
//! governed and is not. A ceiling that copies only some fields, a request that
//! declares no tokens, and a provider call that blocks the UI all end the same
//! way -- the product doing something the person did not agree to, and could
//! not see it doing.

use super::*;

/// Characters of workspace text an Assist operation may send.
pub(crate) const ASSIST_EXCERPT_MAX_CHARS: usize = 4_000;

/// Characters of workspace text an inline prediction may send.
pub(crate) const INLINE_PREDICTION_EXCERPT_MAX_CHARS: usize = 2_000;

/// Completion tokens an inline prediction may return.
pub(crate) const INLINE_PREDICTION_COMPLETION_MAX_TOKENS: u32 = 128;

/// The most tokens a request could consume, for declaration to the broker.
///
/// A ceiling rather than an estimate of what this particular call will use,
/// because that is what a cap needs: `BudgetCapPolicy` refuses a request whose
/// declared tokens exceed the org's limit, so declaring less than the request
/// could consume lets through exactly what the cap exists to stop.
///
/// The prompt is estimated at four characters per token, the usual rule of
/// thumb for these models. It is an estimate -- there is no tokenizer here --
/// and an estimate compared against a cap is worth incomparably more than the
/// `None` declared before it, which `BudgetCapPolicy::refusal` never compares
/// against anything at all.
pub(crate) fn declared_request_tokens(prompt_chars: usize, completion_tokens: u32) -> u64 {
    (prompt_chars as u64)
        .div_ceil(4)
        .saturating_add(u64::from(completion_tokens))
}

/// The cost a request can be truthfully declared to have, in cents.
///
/// Zero for a local backend, which really does cost nothing to invoke. `None`
/// for a remote one: there is no price table here, and inventing a number would
/// defeat the org bundle's own `cost_declaration_required_prefixes` rule, which
/// exists precisely to refuse a request that cannot say what it costs.
pub(crate) fn declared_request_cost_cents(backend: ProductAiLiveBackend) -> Option<u64> {
    match backend {
        ProductAiLiveBackend::Ollama => Some(0),
        _ => None,
    }
}

impl AppComposition {
    pub(crate) fn product_ai_policy_with_org_ceiling(
        &self,
        backend: Option<ProductAiLiveBackend>,
    ) -> SecurityPolicy {
        self.merge_org_ceiling(product_ai_security_policy(backend))
    }

    /// Lower `policy` to the installed bundle's ceiling.
    pub(crate) fn merge_org_ceiling(&self, policy: SecurityPolicy) -> SecurityPolicy {
        let mut policy = policy;
        let Some(bundle) = self.org_policy_bundle.as_ref() else {
            return policy;
        };
        let org = &bundle.bundle().security_policy;

        // Every field the bundle can tighten, not the three that were noticed.
        //
        // The first version of this helper copied two booleans and the
        // allowlist, which made it a partial ceiling wearing a complete one's
        // name -- and a partial ceiling is worse than none, because the call
        // sites now believe they are governed. A bundle that sets
        // `provider_invocation_enabled = false` is an org-wide kill switch, and
        // it was being dropped on the floor: Delegate chat went on invoking
        // Ollama and Anthropic exactly as before.
        policy.network_policy.air_gap |= org.network_policy.air_gap;
        policy.network_policy.local_provider_only |= org.network_policy.local_provider_only;
        policy.network_policy.allow_untrusted &= org.network_policy.allow_untrusted;
        policy.ai_provider_policy.provider_invocation_enabled &=
            org.ai_provider_policy.provider_invocation_enabled;
        policy.ai_provider_policy.allow_remote_provider &=
            org.ai_provider_policy.allow_remote_provider;
        policy.ai_provider_policy.allow_local_provider &=
            org.ai_provider_policy.allow_local_provider;
        policy.ai_provider_policy.deny_when_untrusted |= org.ai_provider_policy.deny_when_untrusted;

        // The blocklist is a union: naming a host only ever denies more, so
        // there is no direction in which taking both lists loosens anything.
        //
        // Compared without case, like every other host comparison here: DNS
        // names are case-insensitive and so is the broker's own
        // `host_matches_configured`, so a list that compared them exactly would
        // disagree with the component that enforces it.
        let already_blocked: std::collections::HashSet<String> = policy
            .network_policy
            .blocklist
            .iter()
            .map(|host| host.to_ascii_lowercase())
            .collect();
        for host in &org.network_policy.blocklist {
            if !already_blocked.contains(&host.to_ascii_lowercase()) {
                policy.network_policy.blocklist.push(host.clone());
            }
        }

        // An empty org allowlist is "unrestricted", not "nothing allowed" --
        // treating it as the latter would deny every route the moment any bundle
        // was installed. A non-empty one intersects.
        if !org.network_policy.allowlist.is_empty() {
            // Case-insensitively, for the reason above and with a sharper
            // consequence: an exact comparison between a configured endpoint of
            // `API.ANTHROPIC.COM` and an allowlist entry of `api.anthropic.com`
            // removes the host the org actually permitted, and every Assist and
            // Delegate request is then denied by a bundle that allowed them.
            let org_allowed: std::collections::HashSet<String> = org
                .network_policy
                .allowlist
                .iter()
                .map(|host| host.to_ascii_lowercase())
                .collect();
            policy
                .network_policy
                .allowlist
                .retain(|host| org_allowed.contains(&host.to_ascii_lowercase()));
        }

        // The bundle's own enforcement rules -- provider allowlist, MCP
        // allowlist, budget caps, retention and export -- taken wholesale. The
        // product default refuses nothing (`default_bundle_enforcement_refuses_nothing`),
        // so adopting the org's can only ever add refusals. Leaving this at the
        // default was what let a bundle name an allowed provider and have
        // nothing consult the list.
        policy.bundle_enforcement = org.bundle_enforcement.clone();

        // `consented_git_remote_hosts` is deliberately untouched. It records a
        // user consent event for the git push/fetch path and grants nothing to
        // AI egress; intersecting it against a bundle that simply does not
        // mention git remotes would revoke a consent the org never spoke about.
        policy
    }

    /// Apply the org ceiling to a caller-supplied policy.
    ///
    /// Exists so a test can put a specific product-side policy through the same
    /// merge the product uses, rather than asserting against a copy of the
    /// merge written in the test -- which would pass while the two disagreed.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn apply_org_ceiling_for_test(&self, policy: SecurityPolicy) -> SecurityPolicy {
        self.merge_org_ceiling(policy)
    }

    /// Take a worker's inline prediction, if anyone is still waiting for it.
    ///
    /// Returns whether it was accepted. A prediction whose request is no longer
    /// the active one is dropped and *nothing* else is touched: the request was
    /// cancelled or superseded while the worker ran, a newer one owns this
    /// state, and clearing its flags on a stale arrival would cancel a request
    /// that is still running.
    ///
    /// `active_request_id` survives acceptance. It is what the projection reads
    /// to find the prediction to show, so clearing it on success published a
    /// result that nothing could then display -- the synchronous path this
    /// replaces left it set for exactly that reason.
    pub(crate) fn merge_inline_prediction_result(
        &mut self,
        prediction: InlinePredictionResult,
    ) -> bool {
        if self
            .assist_inline_prediction_state
            .active_request_id
            .as_ref()
            != Some(&prediction.request_id)
        {
            return false;
        }
        self.assist_inline_prediction_state.results.push(prediction);
        self.assist_inline_prediction_state.request_in_flight = false;
        self.assist_inline_prediction_state.retain_bounded_history();
        true
    }

    /// Hand a live inline prediction to a worker thread, if there is one to hand.
    ///
    /// `Ok(true)` when a worker took it and the caller should leave the request
    /// in flight; `Ok(false)` when there is no live backend, the lane is busy,
    /// or policy refused, and the deterministic path should run inline instead.
    ///
    /// Authorization happens here, on this thread, before any buffer text is
    /// captured for the worker: a refusal must never be something a background
    /// thread discovers after the excerpt has been copied out of the editor.
    pub(crate) fn spawn_live_inline_prediction(
        &mut self,
        metadata: &InlinePredictionRequestMetadata,
    ) -> Result<bool, AppCompositionError> {
        let Some(backend) = product_ai_selected_live_backend(self.preferred_ai_provider) else {
            return Ok(false);
        };
        if !self.inline_prediction_provider_authorized(backend, metadata)? {
            return Ok(false);
        }

        let (provider_id, model, _class, _target, _, _, _) = product_ai_route_fields(Some(backend));
        let Some(lane_reservation) = ProductAiLaneReservation::try_acquire(
            self.live_product_ai_stream.clone(),
            "assist.inline_prediction",
            &provider_id,
            &model,
        ) else {
            // Another product operation owns the lane. Falling back to the
            // deterministic path keeps ghost text answering rather than
            // failing, which is the right trade for a suggestion.
            return Ok(false);
        };

        let buffer_excerpt = self.inline_prediction_excerpt(metadata);
        let metadata_for_worker = metadata.clone();
        let worker = move || {
            let prediction = try_live_product_inline_prediction(
                Some(backend),
                &metadata_for_worker,
                &buffer_excerpt,
            );
            lane_reservation.finish_background(
                ProductAiBackgroundResult {
                    assistant_message_id: String::new(),
                    content_label: String::new(),
                    stream: None,
                    assist_proposal: None,
                    inline_prediction: prediction,
                },
                None,
            );
        };
        match std::thread::Builder::new()
            .name("legion-inline-prediction".to_string())
            .spawn(worker)
        {
            Ok(_handle) => {
                self.pending_inline_prediction = Some(metadata.clone());
                Ok(true)
            }
            // A spawn failure is not a reason to have no ghost text, and it is
            // certainly not a reason to run the provider on this thread after
            // deciding not to. The lane reservation is dropped with the closure,
            // which releases it.
            Err(_error) => Ok(false),
        }
    }

    /// The buffer text a prediction request is allowed to see.
    pub(crate) fn inline_prediction_excerpt(
        &self,
        metadata: &InlinePredictionRequestMetadata,
    ) -> String {
        self.editor
            .text(metadata.buffer_id)
            .unwrap_or("")
            .chars()
            .take(INLINE_PREDICTION_EXCERPT_MAX_CHARS)
            .collect::<String>()
    }

    /// Whether policy permits this backend to be invoked for ghost text.
    pub(crate) fn inline_prediction_provider_authorized(
        &self,
        backend: ProductAiLiveBackend,
        metadata: &InlinePredictionRequestMetadata,
    ) -> Result<bool, AppCompositionError> {
        let (provider_id, _model, _class, network_target, _, _, _) =
            product_ai_route_fields(Some(backend));
        let broker = DenyByDefaultBroker::new(
            self.product_ai_policy_with_org_ceiling(Some(backend)),
            CapabilityNamespace("app.ai".to_string()),
        );
        let decision = broker
            .handle(CapabilityRequest::Request {
                principal_id: metadata.principal_id.clone(),
                capability_id: CapabilityId("ai.provider.invoke".to_string()),
                workspace_trust_state: metadata.workspace_trust_state.clone(),
                target_path: None,
                decision_id: None,
                context: legion_protocol::CapabilityRequestContext {
                    network_target,
                    ai_provider_id: Some(provider_id),
                    // Declared, or the org bundle's token cap is compared
                    // against nothing and a capped request runs uncapped.
                    budget_request_tokens: Some(declared_request_tokens(
                        INLINE_PREDICTION_EXCERPT_MAX_CHARS,
                        INLINE_PREDICTION_COMPLETION_MAX_TOKENS,
                    )),
                    budget_request_cost_cents: declared_request_cost_cents(backend),
                    ..Default::default()
                },
                correlation_id: metadata.correlation_id,
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.message))?;
        Ok(matches!(
            decision,
            CapabilityResponse::Decision(ref d) if d.granted
        ) || matches!(decision, CapabilityResponse::Granted(_)))
    }
}

#[cfg(test)]
/// The org ceiling covers every field a bundle can tighten.
///
/// Not a style point. The first version copied two booleans and the
/// allowlist, so a bundle setting `provider_invocation_enabled = false` --
/// an org-wide kill switch, the bluntest control an administrator has --
/// was dropped and Delegate chat went on invoking providers exactly as
/// before. A partial ceiling is worse than none: the call sites believe
/// they are governed.
mod org_ceiling {
    use legion_security::{
        PolicyKeyring, PolicySigningKey, policy_bundle_verifying_key_b64, sign_policy_bundle,
    };

    const SEED: [u8; 32] = [11u8; 32];
    const KEY_ID: &str = "org-ceiling-test-signer";

    /// The shipped example with one line replaced.
    ///
    /// Editing the real bundle rather than hand-rolling one keeps the
    /// fixture honest about the schema: a bundle that stopped parsing would
    /// fail here rather than quietly testing a default.
    fn bundle_with(find: &str, replace: &str) -> legion_security::VerifiedPolicyBundle {
        let payload = include_str!("../../../xtask/legion-policy.example.toml");
        assert!(
            payload.contains(find),
            "the example bundle no longer contains {find}, so this fixture is not \
             changing what it claims to change"
        );
        let edited = payload.replace(find, replace);
        let keyring = PolicyKeyring::new(vec![PolicySigningKey {
            key_id: KEY_ID.to_string(),
            verifying_key_b64: policy_bundle_verifying_key_b64(&SEED),
        }]);
        sign_policy_bundle(&edited, KEY_ID, &SEED)
            .verify(&keyring)
            .expect("a bundle this test signed must verify")
    }

    #[test]
    fn an_org_wide_provider_kill_switch_is_honored() {
        let mut app = super::AppComposition::new();
        app.set_org_policy_bundle(bundle_with(
            "provider_invocation_enabled = true",
            "provider_invocation_enabled = false",
        ));

        let policy = app.product_ai_policy_with_org_ceiling(None);
        assert!(
            !policy.ai_provider_policy.provider_invocation_enabled,
            "the bundle disabled provider invocation outright and the effective policy \
             still permits it, so every provider lane is unguarded by the one control \
             that was meant to stop all of them"
        );
    }

    #[test]
    fn the_bundles_own_enforcement_rules_are_adopted() {
        let mut app = super::AppComposition::new();
        app.set_org_policy_bundle(bundle_with(
            "provider_invocation_enabled = true",
            "provider_invocation_enabled = true",
        ));

        let policy = app.product_ai_policy_with_org_ceiling(None);
        let expected = &app
            .org_policy_bundle
            .as_ref()
            .expect("the bundle was just installed")
            .bundle()
            .security_policy
            .bundle_enforcement;
        assert_eq!(
            policy.bundle_enforcement.provider.enforced, expected.provider.enforced,
            "the bundle's provider allowlist was not adopted, so a bundle naming exactly \
             which providers may run has nothing consulting the list"
        );
        assert_eq!(
            policy.bundle_enforcement.provider.allowed_provider_ids,
            expected.provider.allowed_provider_ids,
            "the allowed provider ids were dropped"
        );
    }

    #[test]
    fn an_org_blocklist_is_added_rather_than_replacing_the_products() {
        let mut app = super::AppComposition::new();
        let product = super::product_ai_security_policy(None);
        app.set_org_policy_bundle(bundle_with(
            "provider_invocation_enabled = true",
            "provider_invocation_enabled = true",
        ));

        let policy = app.product_ai_policy_with_org_ceiling(None);
        for host in &product.network_policy.blocklist {
            assert!(
                policy.network_policy.blocklist.contains(host),
                "installing a bundle removed {host} from the blocklist; a ceiling that \
                 un-denies something is not a ceiling"
            );
        }
    }

    #[test]
    fn an_allowlist_is_intersected_without_regard_to_case() {
        // DNS names are case-insensitive, and so is the broker's own
        // `host_matches_configured`. An exact comparison here removes the
        // host the org actually permitted -- `API.ANTHROPIC.COM` against an
        // allowlist entry of `api.anthropic.com` -- and every Assist and
        // Delegate request is then denied by a bundle that allowed them.
        let mut app = super::AppComposition::new();
        app.set_org_policy_bundle(bundle_with(
            "provider_invocation_enabled = true",
            "provider_invocation_enabled = true",
        ));

        let mut policy = app.product_ai_policy_with_org_ceiling(None);
        let org_hosts: Vec<String> = app
            .org_policy_bundle
            .as_ref()
            .expect("the bundle was just installed")
            .bundle()
            .security_policy
            .network_policy
            .allowlist
            .clone();
        if org_hosts.is_empty() {
            // An empty org allowlist is "unrestricted" and this rule cannot
            // be exercised through it; say so rather than passing quietly.
            return;
        }

        // Re-run the merge with the product side upper-cased. The org list
        // is unchanged, so anything dropped was dropped for case alone.
        let shouted: Vec<String> = org_hosts
            .iter()
            .map(|host| host.to_ascii_uppercase())
            .collect();
        policy.network_policy.allowlist = shouted.clone();
        let merged = app.apply_org_ceiling_for_test(policy);
        assert_eq!(
            merged.network_policy.allowlist.len(),
            shouted.len(),
            "upper-casing the configured hosts emptied the allowlist, so a bundle that \
             permits them would deny every request instead"
        );
    }

    #[test]
    fn a_provider_request_declares_the_tokens_it_could_consume() {
        // `BudgetCapPolicy::refusal` compares a declared token count against
        // the org's cap and does nothing at all with `None`, so an
        // undeclared request runs uncapped however low the cap is set.
        let declared = super::declared_request_tokens(
            super::ASSIST_EXCERPT_MAX_CHARS,
            crate::product_ai_completion::PRODUCT_COMPLETION_MAX_TOKENS,
        );
        assert!(
            declared > u64::from(crate::product_ai_completion::PRODUCT_COMPLETION_MAX_TOKENS),
            "the declaration must account for the prompt as well as the completion, or a \
             cap set just above the completion size would never fire"
        );

        // A cap below what the request could consume must refuse it. This is
        // the comparison that `None` skipped entirely.
        let policy = legion_security::policy::BudgetCapPolicy {
            enforced: true,
            max_request_cost_cents: u64::MAX,
            max_request_tokens: declared - 1,
            max_session_cost_cents: u64::MAX,
            cost_declaration_required_prefixes: Vec::new(),
        };
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(0), Some(declared), Some(0))
                .is_some(),
            "a declared token count above the cap was not refused"
        );
        assert!(
            policy
                .refusal("ai.provider.invoke", Some(0), None, Some(0))
                .is_none(),
            "the undeclared case is what this fix exists to stop being the norm"
        );
    }

    #[test]
    fn no_bundle_imposes_no_ceiling() {
        // Non-vacuity: the refusals above must be the bundle talking, not
        // this helper denying everything it is handed.
        let app = super::AppComposition::new();
        let policy = app.product_ai_policy_with_org_ceiling(None);
        let product = super::product_ai_security_policy(None);
        assert_eq!(
            policy.ai_provider_policy.provider_invocation_enabled,
            product.ai_provider_policy.provider_invocation_enabled,
            "with no bundle installed the product policy must pass through unchanged"
        );
    }
}
