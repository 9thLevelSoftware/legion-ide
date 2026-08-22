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
pub(crate) const ASSIST_EXCERPT_MAX_BYTES: usize = 4_000;

/// Characters of workspace text an inline prediction may send.
pub(crate) const INLINE_PREDICTION_EXCERPT_MAX_BYTES: usize = 2_000;

/// Completion tokens an inline prediction may return.
pub(crate) const INLINE_PREDICTION_COMPLETION_MAX_TOKENS: u32 = 128;

/// Characters of framing an inline prediction prompt carries.
///
/// `try_live_product_inline_prediction` sends a fixed system prompt plus the
/// language, the cursor position and delimiters around the excerpt. Declaring
/// the excerpt alone understated the request by all of it, which is the same
/// defect the Assist declaration had -- a cap set between the declared size and
/// the real one admits the request it was configured to refuse.
pub(crate) const INLINE_PREDICTION_FRAMING_MAX_BYTES: usize = 512;

/// The inline total must be every part of its prompt, like the Assist one.
///
/// A compile-time check, because it cannot be true at some times and false at
/// others -- and because a test asserting it is a constant expression, which is
/// a lint rather than a guarantee.
const _: () = assert!(
    INLINE_PREDICTION_PROMPT_MAX_BYTES
        == INLINE_PREDICTION_EXCERPT_MAX_BYTES + INLINE_PREDICTION_FRAMING_MAX_BYTES,
    "the inline declared prompt total must be the sum of every bounded part"
);

/// Every character an inline prediction prompt can carry.
pub(crate) const INLINE_PREDICTION_PROMPT_MAX_BYTES: usize =
    INLINE_PREDICTION_EXCERPT_MAX_BYTES + INLINE_PREDICTION_FRAMING_MAX_BYTES;

/// Characters of caller-supplied instruction an Assist operation may send.
///
/// Bounded because it is declared. `StartAiProposal` takes an instruction label
/// from the caller with no length of its own, and the prompt built from it also
/// carries the file path, framing and a system preamble -- so a declaration
/// counting only the excerpt understated the request, and an org token cap set
/// between the declared size and the real one permitted exactly the request it
/// was meant to refuse. A budget declaration is only as honest as the smallest
/// bound on what it describes.
pub(crate) const ASSIST_INSTRUCTION_MAX_BYTES: usize = 2_000;

/// Characters of file path an Assist prompt may carry.
///
/// Bounded for the same reason the instruction is: the prompt embeds the active
/// file's canonical path, which has no length of its own, so a declaration
/// allowing for a nominal path understated any request against a deeply nested
/// one. Long enough for any path somebody works in, short enough to be a bound.
pub(crate) const ASSIST_PATH_MAX_BYTES: usize = 512;

/// Characters of fixed prompt framing: system preamble and delimiters.
///
/// A generous allowance rather than a measurement of the template, so that
/// editing the prompt cannot silently shrink what was declared.
pub(crate) const ASSIST_PROMPT_FRAMING_MAX_BYTES: usize = 1_000;

/// Every character an Assist prompt can carry: excerpt, instruction, path, framing.
///
/// One number, so the four bounds and the declaration cannot drift apart -- and
/// the compile-time assertion below holds them to it, which is what stops a term
/// being added here and forgotten in the sum.
pub(crate) const ASSIST_PROMPT_MAX_BYTES: usize = ASSIST_EXCERPT_MAX_BYTES
    + ASSIST_INSTRUCTION_MAX_BYTES
    + ASSIST_PATH_MAX_BYTES
    + ASSIST_PROMPT_FRAMING_MAX_BYTES;

/// The declared total must *be* every part, not merely exceed some of them.
///
/// Equality, because `>` reduced to "framing is non-zero" and would have been
/// satisfied by a total that dropped a whole term -- which is exactly the
/// regression this guard exists to catch.
///
/// A compile-time check rather than a test: this cannot be true at some times
/// and false at others, and a test asserting it would only restate arithmetic.
const _: () = assert!(
    ASSIST_PROMPT_MAX_BYTES
        == ASSIST_EXCERPT_MAX_BYTES
            + ASSIST_INSTRUCTION_MAX_BYTES
            + ASSIST_PATH_MAX_BYTES
            + ASSIST_PROMPT_FRAMING_MAX_BYTES,
    "the declared prompt total must be the sum of every bounded part"
);

/// Trim a file path to the length that was declared, keeping its tail.
///
/// The tail, because the end of a path is what identifies the file; truncating
/// from the front would leave every deeply nested file looking like the same
/// prefix.
pub(crate) fn bounded_assist_path(path: &str) -> String {
    if path.len() <= ASSIST_PATH_MAX_BYTES {
        return path.to_string();
    }
    let mut start = path.len() - ASSIST_PATH_MAX_BYTES;
    while start < path.len() && !path.is_char_boundary(start) {
        start += 1;
    }
    path[start..].to_string()
}

/// Trim a caller-supplied instruction to the length that was declared.
///
/// Declaring a bound and not enforcing it is the same defect as declaring
/// nothing: the cap is compared against a number the request is free to exceed.
pub(crate) fn bounded_assist_instruction(instruction: &str) -> String {
    bounded_by_bytes(instruction, ASSIST_INSTRUCTION_MAX_BYTES)
}

/// Trim to a byte budget without splitting a character.
///
/// Byte-bounded because the declaration is: trimming by characters and
/// declaring by bytes would let a multi-byte instruction pass the trim and
/// exceed the declaration, which is the same understatement one layer along.
pub(crate) fn bounded_by_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// The most tokens a request could consume, for declaration to the broker.
///
/// A ceiling rather than an estimate of what this particular call will use,
/// because that is what a cap needs: `BudgetCapPolicy` refuses a request whose
/// declared tokens exceed the org's limit, so declaring less than the request
/// could consume lets through exactly what the cap exists to stop.
///
/// One token per UTF-8 byte, which no text can exceed.
///
/// Characters were the second attempt and still understated. A `char` is a
/// Unicode scalar, and one scalar can be several tokens: an emoji outside the
/// BMP, a combining sequence, an unusual script. Two thousand of those in a
/// prompt exceed a two-thousand-character declaration, and a cap set between
/// the declared value and the real one admits the request it was configured to
/// refuse.
///
/// Bytes cannot be beaten that way. Every tokenizer these providers use emits
/// at most one token per input byte -- most emit far fewer -- so a byte count
/// is an over-declaration in the safe direction. A real tokenizer would be
/// tighter and is what this should become; a bound that cannot be exceeded is
/// what is available now.
pub(crate) fn declared_request_tokens(prompt_bytes: usize, completion_tokens: u32) -> u64 {
    (prompt_bytes as u64).saturating_add(u64::from(completion_tokens))
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

        // An empty org allowlist means what it means to the broker: nothing.
        //
        // `DenyByDefaultBroker` allows a destination only when some allowlist
        // entry matches it, so an empty list denies every host. Reading it here
        // as "unrestricted" made the ceiling disagree with the component that
        // enforces it, in the one direction a ceiling must never take: a bundle
        // that switched off network egress entirely kept the product's own
        // Ollama and Anthropic hosts, and the routes this PR made reachable
        // would have uploaded workspace excerpts under a policy forbidding it.
        //
        // Intersecting unconditionally is what the org asked for. An empty
        // result denies, which is exactly what an empty org list says.
        {
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
    #[cfg(feature = "ai")]
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
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled_for_worker = cancelled.clone();
        let worker = move || {
            let prediction = try_live_product_inline_prediction(
                Some(backend),
                &metadata_for_worker,
                &buffer_excerpt,
            );
            // Cancelled while this ran. The lane was already released from the
            // app thread so the product would not sit blocked behind a request
            // nobody wanted; releasing it again here could free a lane that now
            // belongs to somebody else.
            if cancelled_for_worker.load(std::sync::atomic::Ordering::SeqCst) {
                lane_reservation.abandon();
                return;
            }
            lane_reservation.finish_background(
                ProductAiBackgroundResult {
                    // Inline prediction has its own fallback path; this flag is
                    // about the Assist proposal record.
                    live_failed: false,
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
                self.pending_inline_prediction_cancelled = Some(cancelled);
                Ok(true)
            }
            // A spawn failure is not a reason to have no ghost text, and it is
            // certainly not a reason to run the provider on this thread after
            // deciding not to. The lane reservation is dropped with the closure,
            // which releases it.
            Err(_error) => Ok(false),
        }
    }

    #[cfg(not(feature = "ai"))]
    pub(crate) fn spawn_live_inline_prediction(
        &mut self,
        _metadata: &InlinePredictionRequestMetadata,
    ) -> Result<bool, AppCompositionError> {
        Ok(false)
    }

    /// The buffer text a prediction request is allowed to see.
    pub(crate) fn inline_prediction_excerpt(
        &self,
        metadata: &InlinePredictionRequestMetadata,
    ) -> String {
        // Bounded in bytes, because that is the unit the declaration is in.
        //
        // Taking characters and declaring bytes was the same understatement one
        // layer along: two thousand emoji are two thousand `char`s and eight
        // thousand bytes, so the excerpt sailed past a bound the broker had
        // been told was the whole request.
        bounded_by_bytes(
            self.editor.text(metadata.buffer_id).unwrap_or(""),
            INLINE_PREDICTION_EXCERPT_MAX_BYTES,
        )
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
                        INLINE_PREDICTION_PROMPT_MAX_BYTES,
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

    /// Sign and verify a bundle payload this test built.
    fn signed(payload: &str) -> legion_security::VerifiedPolicyBundle {
        let keyring = PolicyKeyring::new(vec![PolicySigningKey {
            key_id: KEY_ID.to_string(),
            verifying_key_b64: policy_bundle_verifying_key_b64(&SEED),
        }]);
        sign_policy_bundle(payload, KEY_ID, &SEED)
            .verify(&keyring)
            .expect("a bundle this test signed must verify")
    }

    /// The shipped example with one line replaced.
    ///
    /// Editing the real bundle rather than hand-rolling one keeps the fixture
    /// honest about the schema: a bundle that stopped parsing would fail here
    /// rather than quietly testing a default.
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
            // An empty org allowlist denies every host, which is a different
            // rule with its own test, so case-insensitivity cannot be
            // exercised through this bundle. Say so rather than passing
            // quietly on a fixture that proves nothing.
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
            super::ASSIST_EXCERPT_MAX_BYTES,
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
    fn an_empty_org_allowlist_denies_every_host() {
        // `DenyByDefaultBroker` allows a destination only when some allowlist
        // entry matches it, so an empty list denies every host. Reading it here
        // as "unrestricted" made the ceiling disagree with the component that
        // enforces it, in the one direction a ceiling must never take: a bundle
        // that switched network egress off entirely kept the product's own
        // Ollama and Anthropic hosts, and the routes this PR made reachable
        // would have uploaded workspace excerpts under a policy forbidding it.
        // Line endings normalised: the file is CRLF in this repository and may be
        // checked out as LF elsewhere, and a fixture that matched only one of
        // those would pass or fail by platform rather than by behaviour.
        let payload =
            include_str!("../../../xtask/legion-policy.example.toml").replace("\r\n", "\n");
        let hosts = "  \"localhost\",\n  \"127.0.0.1\",\n  \"::1\",\n";
        assert!(
            payload.contains(hosts),
            "the example bundle no longer lists those hosts, so emptying them proves nothing"
        );

        let mut app = super::AppComposition::new();
        app.set_org_policy_bundle(signed(&payload.replace(hosts, "")));

        // A backend whose product policy actually allowlists a host.
        //
        // Asked with `None`, `product_ai_security_policy` pushes no hosts at
        // all, so the intersection came out empty whether or not the fix was
        // applied -- the test passed against the code it was written to catch,
        // and only exercised the bundle parser. The whole claim is that a
        // bundle emptying its allowlist stops the *product's own* egress, so
        // the product side has to have some.
        let product =
            super::product_ai_security_policy(Some(super::ProductAiLiveBackend::Anthropic));
        assert!(
            !product.network_policy.allowlist.is_empty(),
            "the fixture needs a backend with product-side hosts, or it proves nothing"
        );

        let merged =
            app.product_ai_policy_with_org_ceiling(Some(super::ProductAiLiveBackend::Anthropic));
        assert!(
            merged.network_policy.allowlist.is_empty(),
            "an org bundle that allows no hosts left {:?} allowed, so a policy forbidding \
             egress would still have let workspace text out",
            merged.network_policy.allowlist
        );
    }

    #[test]
    fn the_assist_declaration_covers_the_whole_prompt() {
        // The declaration counted only the excerpt while the prompt also carried
        // an unbounded instruction, the file path and a system preamble. An org
        // token cap set between the declared size and the real one then
        // permitted exactly the request it was meant to refuse.
        // And the bound is enforced, not merely declared. A bound nothing
        // applies is a number the request is free to exceed.
        let long = "x".repeat(super::ASSIST_INSTRUCTION_MAX_BYTES * 3);
        assert_eq!(
            super::bounded_assist_instruction(&long).chars().count(),
            super::ASSIST_INSTRUCTION_MAX_BYTES,
            "an instruction longer than the declared bound was not trimmed"
        );
        assert_eq!(
            super::bounded_assist_instruction("short"),
            "short",
            "an instruction within the bound must be left exactly as written"
        );

        // Multi-byte text, which is where a character bound stops being one.
        //
        // The declaration counts bytes because one Unicode scalar can be
        // several provider tokens; trimming by characters while declaring by
        // bytes would let a prompt of emoji pass the trim and exceed the
        // declaration, which is the same understatement one layer along.
        let emoji = "\u{1F600}".repeat(super::ASSIST_INSTRUCTION_MAX_BYTES);
        let trimmed = super::bounded_assist_instruction(&emoji);
        assert!(
            trimmed.len() <= super::ASSIST_INSTRUCTION_MAX_BYTES,
            "a multi-byte instruction was trimmed to {} bytes against a bound of {}",
            trimmed.len(),
            super::ASSIST_INSTRUCTION_MAX_BYTES
        );
        assert!(
            !trimmed.is_empty(),
            "trimming must keep what fits rather than discarding everything"
        );

        // The path is embedded in the prompt too, and a canonical path has no
        // length of its own.
        let deep = format!("/{}/main.rs", "nested".repeat(400));
        let bounded = super::bounded_assist_path(&deep);
        assert_eq!(
            bounded.chars().count(),
            super::ASSIST_PATH_MAX_BYTES,
            "a path longer than the declared bound was not trimmed, so a deeply nested file \
             makes the declaration understate the request"
        );
        assert!(
            bounded.ends_with("main.rs"),
            "the tail identifies the file; trimming from the end would leave every deep path \
             looking alike, got {bounded:?}"
        );
        assert_eq!(
            super::bounded_assist_path("src/lib.rs"),
            "src/lib.rs",
            "an ordinary path must be left exactly as written"
        );

        // And a path of multi-byte characters, trimmed from the front, must
        // still be valid UTF-8 rather than a split scalar.
        let deep_unicode = format!(
            "/{}/main.rs",
            "\u{00E9}".repeat(super::ASSIST_PATH_MAX_BYTES)
        );
        let bounded_unicode = super::bounded_assist_path(&deep_unicode);
        assert!(
            bounded_unicode.len() <= super::ASSIST_PATH_MAX_BYTES,
            "a multi-byte path was trimmed to {} bytes against a bound of {}",
            bounded_unicode.len(),
            super::ASSIST_PATH_MAX_BYTES
        );
        assert!(
            bounded_unicode.ends_with("main.rs"),
            "the tail identifies the file, got {bounded_unicode:?}"
        );
    }

    /// A failed live backend is not reported as a deterministic run.
    ///
    /// Both cases used to produce the same proposal: no live backend selected,
    /// and a live backend that failed. The second is a fixture wearing a
    /// provider'''s name -- a person reviewing it has no way to know the
    /// provider never answered, and the details they would check say the
    /// opposite.
    #[cfg(feature = "ai")]
    #[test]
    fn a_failed_live_backend_says_so_in_its_proposal() {
        // Through `resolve_assisted_edit_proposal_text`, not the helper it calls.
        //
        // Asserting against the helper directly proves only that the helper
        // works: removing its call site left this test passing while the
        // product collapsed failures back into the offline path, which is the
        // exact defect being fixed.
        let (offline, _) = crate::product_ai_completion::resolve_assisted_edit_proposal_text(
            None,
            "tidy this",
            "fn main() {}",
            "src/main.rs",
            None,
        );
        // Anthropic with no credential resolvable in the test environment: the
        // backend is selected and the call cannot succeed, which is the shape
        // of a live failure.
        let (failed, stream) = crate::product_ai_completion::resolve_assisted_edit_proposal_text(
            Some(super::ProductAiLiveBackend::Anthropic),
            "tidy this",
            "fn main() {}",
            "src/main.rs",
            None,
        );
        assert!(
            stream.is_none(),
            "the fixture needs the live call to fail, or this asserts nothing"
        );

        assert_ne!(
            failed.summary, offline.summary,
            "a failed provider run must not be summarised as an ordinary offline one"
        );
        assert!(
            failed.summary.contains("anthropic"),
            "the summary must name the backend that failed, got {:?}",
            failed.summary
        );
        assert!(
            failed
                .details
                .iter()
                .any(|line| line.contains("outcome=failed")),
            "the details a reviewer checks must record the failure, got {:?}",
            failed.details
        );
        assert_eq!(
            failed.replacement, offline.replacement,
            "the fallback content itself is unchanged; only what it claims about itself is"
        );
    }

    /// A failed Delegate provider is not reported as an answer.
    ///
    /// The fixture reply said an answer was "ready" and then advised enabling
    /// the very backend that had just been selected and failed. Same defect as
    /// the Assist path had, in the lane that shows its text to somebody who
    /// asked a question.
    #[cfg(feature = "ai")]
    #[test]
    fn a_failed_delegate_provider_says_so_in_its_reply() {
        let (offline, _) = crate::product_ai_completion::resolve_delegate_chat_reply(
            None,
            "what does this do?",
            "fn main() {}",
            "src/main.rs",
            0,
            "route-1",
            &[],
            None,
        );
        let (failed, stream) = crate::product_ai_completion::resolve_delegate_chat_reply(
            Some(super::ProductAiLiveBackend::Anthropic),
            "what does this do?",
            "fn main() {}",
            "src/main.rs",
            0,
            "route-1",
            &[],
            None,
        );

        assert!(
            stream.is_none(),
            "the fixture needs the live call to fail, or this asserts nothing"
        );
        assert_ne!(
            failed, offline,
            "a failed provider run must not read the same as an ordinary offline one"
        );
        assert!(
            failed.contains("did not answer"),
            "the reply must say the provider did not answer, got {failed:?}"
        );
        assert!(
            !failed.contains("enable Ollama loopback"),
            "advising somebody to enable the backend that just failed them is worse than \
             saying nothing, got {failed:?}"
        );
    }

    /// The capability a reviewer reads describes the provider that ran.
    ///
    /// It was hard-coded to the deterministic local one, so a proposal produced
    /// by Anthropic -- which had uploaded the buffer excerpt -- was presented as
    /// a free, offline, air-gap-safe local run with metadata-only retention.
    /// This projection is what somebody consults to decide whether to accept an
    /// edit, which makes it the worst possible place for an invented answer.
    #[test]
    fn the_provider_capability_describes_the_routed_provider() {
        let remote = super::phase4_provider_capability(
            legion_protocol::AssistedAiProviderClass::ByokRemote,
            "anthropic",
            None,
        );
        assert_eq!(remote.provider_id, "anthropic");
        assert_ne!(
            remote.cost_budget_label, "local.free",
            "a metered remote call must not be presented as free"
        );
        assert_ne!(
            remote.privacy_retention_label, "metadata-only",
            "a call that sends the excerpt itself must not claim metadata-only retention"
        );
        assert_eq!(
            remote.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Unsupported,
            "a remote provider is not air-gap safe"
        );

        let local = super::phase4_provider_capability(
            legion_protocol::AssistedAiProviderClass::LocalLoopback,
            "ollama",
            None,
        );
        assert_eq!(local.provider_id, "ollama");
        assert_eq!(
            local.cost_budget_label, "local.free",
            "a loopback call really is free and should say so"
        );
        assert_eq!(
            local.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Supported,
            "a loopback call really is air-gap safe"
        );

        // The arm nobody has taught this function about. A provider id it does
        // not recognise used to be relabelled "Deterministic local provider",
        // which is the half-coverage that let the original hard-coding through
        // review: the two ids the test exercised were the two the match knew.
        let hosted = super::phase4_provider_capability(
            legion_protocol::AssistedAiProviderClass::HostedRemote,
            "acme-hosted",
            None,
        );
        assert_eq!(hosted.provider_id, "acme-hosted");
        assert!(
            hosted.provider_label.contains("acme-hosted"),
            "an unfamiliar provider was relabelled as somebody else: {:?}",
            hosted.provider_label
        );
        assert_ne!(
            hosted.cost_budget_label, "local.free",
            "a hosted remote provider must not be presented as free"
        );
        assert_eq!(
            hosted.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Unsupported,
            "a hosted remote provider is not air-gap safe"
        );

        // An unrecognised *class* counts as remote, because guessing "local and
        // free" is the expensive way to be wrong.
        let unknown = super::phase4_provider_capability(
            legion_protocol::AssistedAiProviderClass::Unknown,
            "mystery",
            None,
        );
        assert_ne!(
            unknown.cost_budget_label, "local.free",
            "a provider class nothing recognises must not be assumed free"
        );
        assert_eq!(
            unknown.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Unsupported,
            "a provider class nothing recognises must not be assumed air-gap safe"
        );

        // A local provider that is not ollama is still local.
        let vendor = super::phase4_provider_capability(
            legion_protocol::AssistedAiProviderClass::Local,
            "vendor-local",
            None,
        );
        assert_eq!(vendor.provider_id, "vendor-local");
        assert_eq!(
            vendor.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Supported,
            "a local provider is air-gap safe whatever it is called"
        );
    }

    /// The route decides the class, and the capability follows the route.
    ///
    /// Deriving `remote` from the class was only half a fix while the class
    /// itself came from a caller: both Assist entry points passed
    /// `LocalLoopback` because that is what the fixture used to be, and an
    /// Anthropic proposal was therefore presented as local, free,
    /// metadata-only and air-gap safe. The parameter is gone, so the only
    /// answer available is the one this chain produces -- and this test is the
    /// chain.
    #[test]
    fn a_remote_route_produces_a_remote_capability_end_to_end() {
        let (provider_id, _model, class, ..) =
            super::product_ai_route_fields(Some(crate::ProductAiLiveBackend::Anthropic));
        assert_eq!(
            class,
            legion_protocol::AssistedAiProviderClass::ByokRemote,
            "an Anthropic route is a BYOK remote one; everything downstream reads this"
        );

        let capability = super::phase4_provider_capability(class, &provider_id, None);
        assert_ne!(
            capability.cost_budget_label, "local.free",
            "the capability for a metered remote route says it is free"
        );
        assert_eq!(
            capability.air_gap_support,
            legion_protocol::AssistedAiSupportLabel::Unsupported,
            "the capability for a remote route says it is air-gap safe"
        );

        // The other end of the same chain, so this cannot pass by calling
        // everything remote.
        let (provider_id, _model, class, ..) =
            super::product_ai_route_fields(Some(crate::ProductAiLiveBackend::Ollama));
        assert_eq!(
            class,
            legion_protocol::AssistedAiProviderClass::LocalLoopback,
            "an Ollama route is loopback"
        );
        let capability = super::phase4_provider_capability(class, &provider_id, None);
        assert_eq!(
            capability.cost_budget_label, "local.free",
            "a loopback route really is free and must keep saying so"
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
