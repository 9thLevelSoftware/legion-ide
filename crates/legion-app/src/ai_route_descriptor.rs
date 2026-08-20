//! Where a product AI chat turn's bytes actually go, and what the audit says.
//!
//! New in its own module: `lib.rs` is a chokepoint with a line budget, and this
//! mapping is the whole of a security property -- the route request, the
//! capability decision and the audit record must name the destination the
//! buffer text really takes.

use crate::{ProductAiLiveBackend, ProductAiProviderPreference, product_ai_selected_live_backend};

/// The network target, health and cost labels for a chosen backend.
///
/// Split from the preference lookup on purpose. Resolving a preference probes
/// the host -- is Ollama listening, is there an Anthropic key -- so a test that
/// went through it would exercise whatever machine happened to run it. On a
/// machine with neither, every preference resolves to `None` and the remote arm
/// below is never taken: a guard written against the preference passed happily
/// with the remote arm mutated to claim local-only traffic. This mapping is the
/// part that must hold everywhere, so this is the part that is tested.
pub(crate) fn route_descriptor_for_backend(
    backend: Option<ProductAiLiveBackend>,
) -> (
    legion_protocol::NetworkTarget,
    &'static str,
    &'static str,
    legion_protocol::ProposalPrivacyLabel,
) {
    match backend {
        Some(ProductAiLiveBackend::Anthropic) => (
            legion_protocol::NetworkTarget {
                scheme: "https".to_string(),
                host: "api.anthropic.com".to_string(),
                port: Some(443),
            },
            "delegate.remote.anthropic",
            "remote.metered",
            // The buffer excerpt leaves the machine on this route, so the label
            // has to say so; `WorkspaceMetadata` would describe a different
            // request than the one being made.
            legion_protocol::ProposalPrivacyLabel::ExternalEgressMetadata,
        ),
        Some(ProductAiLiveBackend::Ollama) => (
            legion_protocol::NetworkTarget {
                scheme: "http".to_string(),
                host: "localhost".to_string(),
                port: Some(11434),
            },
            "delegate.local.ollama",
            "local.free",
            legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
        ),
        None => (
            legion_protocol::NetworkTarget {
                scheme: "http".to_string(),
                host: "localhost".to_string(),
                port: Some(11434),
            },
            "delegate.local.deterministic",
            "local.free",
            legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
        ),
    }
}

/// The route the bytes of a product chat turn will really take.
///
/// The Delegate chat route request used to hard-code `http://localhost:11434`
/// with `delegate.local.deterministic` and `local.free`, and then hand the
/// preferred provider up to 3,000 characters of buffer text -- so a turn
/// answered by Anthropic was authorized, and audited, as local metadata-only
/// traffic. The capability decision described one destination and the bytes went
/// to another.
pub(crate) fn product_ai_route_descriptor(
    preference: ProductAiProviderPreference,
) -> (
    legion_protocol::NetworkTarget,
    &'static str,
    &'static str,
    legion_protocol::ProposalPrivacyLabel,
) {
    route_descriptor_for_backend(product_ai_selected_live_backend(preference))
}

#[cfg(test)]
mod delegate_chat_route_honesty_tests {
    use super::{ProductAiLiveBackend, route_descriptor_for_backend};
    use legion_protocol::ProposalPrivacyLabel;

    /// The route request must describe the destination the bytes actually take.
    ///
    /// Delegate chat used to hard-code `http://localhost:11434` with
    /// `delegate.local.deterministic` and `local.free`, then hand the preferred
    /// provider up to 3,000 characters of buffer text. A turn answered by
    /// Anthropic was therefore authorized and audited as local metadata-only
    /// traffic: the capability decision named one destination and the source
    /// code went to another. In a system whose premise is metadata-only
    /// discipline and an honest audit trail, that is the audit lying.
    #[test]
    fn a_remote_backend_declares_external_egress_and_a_remote_cost() {
        let (target, health, cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));
        assert_eq!(target.host, "api.anthropic.com");
        assert_eq!(target.scheme, "https");
        assert_eq!(
            privacy,
            ProposalPrivacyLabel::ExternalEgressMetadata,
            "a route that carries buffer text off the machine must say so"
        );
        assert_eq!(
            cost, "remote.metered",
            "the exact cost label is part of the contract this file states; `is not local.free` would accept any other wrong string"
        );
        assert!(health.contains("remote"), "health label was {health}");
    }

    /// Local backends stay local, and say so.
    #[test]
    fn local_backends_stay_loopback_and_workspace_scoped() {
        for backend in [None, Some(ProductAiLiveBackend::Ollama)] {
            let (target, _health, cost, privacy) = route_descriptor_for_backend(backend);
            assert_eq!(target.host, "localhost", "{backend:?} left the loopback");
            assert_eq!(cost, "local.free");
            assert_eq!(
                privacy,
                ProposalPrivacyLabel::WorkspaceMetadata,
                "{backend:?} does not leave the machine, so it must not claim egress"
            );
        }
    }
}
