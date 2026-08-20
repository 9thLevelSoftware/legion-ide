//! Where a product AI chat turn's bytes actually go, and what the audit says.
//!
//! New in its own module: `lib.rs` is a chokepoint with a line budget, and this
//! mapping is the whole of a security property -- the route request, the
//! capability decision and the audit record must name the destination the
//! buffer text really takes.

use crate::{ProductAiLiveBackend, anthropic_base_url_from_env};

/// The Anthropic endpoint this build will actually contact.
///
/// Parsed from the configured base URL so the authorized target, the audit
/// record and the request all name one destination.
fn anthropic_network_target() -> legion_protocol::NetworkTarget {
    let base = anthropic_base_url_from_env();
    let trimmed = base.trim();
    let (scheme, rest) = match trimmed.strip_prefix("https://") {
        Some(rest) => ("https", rest),
        None => match trimmed.strip_prefix("http://") {
            Some(rest) => ("http", rest),
            None => ("https", trimmed),
        },
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    let mut parts = authority.rsplitn(2, ':');
    let (host, port) = match (parts.next(), parts.next()) {
        // `rsplitn` yields the tail first, so a parsed port means an explicit
        // one; anything else is a bare host, including an IPv6 literal.
        (Some(tail), Some(head)) if tail.parse::<u16>().is_ok() => {
            (head.to_string(), tail.parse::<u16>().ok())
        }
        _ => (
            authority.to_string(),
            Some(if scheme == "http" { 80 } else { 443 }),
        ),
    };
    legion_protocol::NetworkTarget {
        scheme: scheme.to_string(),
        host: if host.is_empty() {
            "api.anthropic.com".to_string()
        } else {
            host
        },
        port,
    }
}

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
            // Derived from the same base-URL configuration the client uses, not
            // hard-coded. `LEGION_ANTHROPIC_BASE_URL` and its two aliases can
            // point at a proxy or a self-hosted endpoint, and the buffer excerpt
            // goes there -- so a descriptor naming `api.anthropic.com` would
            // misstate the destination for exactly the deployments that care
            // most, and would then have the broker allowlist a host the request
            // never contacts.
            anthropic_network_target(),
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
        // Host is not asserted literally: it comes from the configured base
        // URL, so pinning `api.anthropic.com` here would fail on any deployment
        // pointing at a proxy -- and would re-introduce the hard-coding this
        // parses away from. What must hold is that it is not the loopback.
        assert!(
            target.host != "localhost" && target.host != "127.0.0.1",
            "the Anthropic route must not be described as loopback, got {}",
            target.host
        );
        assert!(!target.host.is_empty());
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

    /// The authorized target follows the configured base URL.
    ///
    /// Set through the process environment, which is what the client reads, so
    /// this exercises the same path a proxy deployment takes rather than a
    /// parallel one. Serialised against the other env-reading test by running
    /// the whole scenario inside one test.
    #[test]
    fn a_configured_base_url_becomes_the_authorized_target() {
        // SAFETY: single-threaded within this test; no other test in this
        // module reads these variables.
        unsafe {
            std::env::set_var(
                "LEGION_ANTHROPIC_BASE_URL",
                "https://proxy.internal:8443/v1",
            );
        }
        let (target, _health, _cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));
        unsafe {
            std::env::remove_var("LEGION_ANTHROPIC_BASE_URL");
        }
        assert_eq!(
            target.host, "proxy.internal",
            "host must follow the base URL"
        );
        assert_eq!(target.port, Some(8443), "an explicit port must be carried");
        assert_eq!(target.scheme, "https");
        assert_eq!(
            privacy,
            ProposalPrivacyLabel::ExternalEgressMetadata,
            "a proxy is still off-machine"
        );
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
