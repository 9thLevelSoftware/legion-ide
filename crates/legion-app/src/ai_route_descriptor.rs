//! Where a product AI chat turn's bytes actually go, and what the audit says.
//!
//! New in its own module: `lib.rs` is a chokepoint with a line budget, and this
//! mapping is the whole of a security property -- the route request, the
//! capability decision and the audit record must name the destination the
//! buffer text really takes.

use crate::ProductAiLiveBackend;

/// The Anthropic base URL this build will actually contact.
///
/// Wraps the `ai`-gated reader so the rest of this module -- which is not gated
/// -- still names a destination in a build without the feature.
fn anthropic_base_url() -> String {
    #[cfg(feature = "ai")]
    {
        crate::anthropic_base_url_from_env()
    }
    #[cfg(not(feature = "ai"))]
    {
        "https://api.anthropic.com".to_string()
    }
}

/// The endpoint a base URL names, as a `NetworkTarget`.
///
/// One parser for every product AI route. The destination has to be derived
/// from the same configuration the client reads, and a second copy of this
/// parsing is a second thing to drift -- the copy that drifts being the one the
/// broker allowlists, or the one the audit record names.
///
/// A base URL with no explicit port resolves to the scheme's default, because
/// that is the port the HTTP client will actually connect to. Substituting a
/// service's conventional port here would name a destination the request never
/// reaches.
fn network_target_from_base_url(base: &str, fallback_host: &str) -> legion_protocol::NetworkTarget {
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
            fallback_host.to_string()
        } else {
            host
        },
        port,
    }
}

/// The Anthropic endpoint this build will actually contact.
///
/// Parsed from the configured base URL so the authorized target, the audit
/// record and the request all name one destination.
pub(crate) fn anthropic_network_target() -> legion_protocol::NetworkTarget {
    network_target_from_base_url(&anthropic_base_url(), "api.anthropic.com")
}

/// The Ollama endpoint this build will actually contact.
///
/// `OLLAMA_BASE_URL` is read by `OllamaProvider::default` and by the
/// reachability probe, so it is what receives the buffer excerpt. A descriptor
/// hard-coding `http://localhost:11434` would misname the destination for
/// exactly the deployments that configured one -- the same mismatch the
/// Anthropic arm was fixed for.
pub(crate) fn ollama_network_target() -> legion_protocol::NetworkTarget {
    network_target_from_base_url(&crate::ollama_base_url_from_env(), "localhost")
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
            // Derived from `OLLAMA_BASE_URL`, for the same reason the Anthropic
            // arm is: the descriptor has to name the endpoint the excerpt goes
            // to, not the endpoint it usually goes to.
            ollama_network_target(),
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
    use super::{ProductAiLiveBackend, network_target_from_base_url, route_descriptor_for_backend};
    use legion_protocol::ProposalPrivacyLabel;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Every environment variable a product AI route reads.
    const ROUTE_ENV_VARS: [&str; 4] = [
        "LEGION_ANTHROPIC_BASE_URL",
        "DEVIL_ANTHROPIC_BASE_URL",
        "ANTHROPIC_BASE_URL",
        "OLLAMA_BASE_URL",
    ];

    /// Serialises the tests that touch the process environment.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            // A test that fails while holding this lock poisons it. The next
            // test still needs a usable environment, and the guard below
            // restores state on unwind, so the poison carries no information
            // worth aborting the rest of the suite for.
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A known, exclusive environment for one test, restored on the way out.
    ///
    /// Two problems, one type. Cargo runs the tests in this binary on several
    /// threads and the environment is process-wide -- which is the reason
    /// `set_var` is `unsafe` in Rust 2024 -- so a test configuring a proxy URL
    /// and a test asserting the default endpoint otherwise race, and the loser
    /// fails on a scheduling accident rather than on a defect. And an assertion
    /// failing between a `set_var` and its matching `remove_var` leaks the
    /// value for the rest of the process, so one failure silently reshapes
    /// every later route in the same binary.
    ///
    /// Holding the `MutexGuard` as a field is what makes the `unsafe` blocks
    /// below sound rather than merely commented: the mutation cannot outlive
    /// the exclusion, because the borrow checker will not let it. `Drop` runs
    /// on the unwind too, so the restore does not depend on reaching the end of
    /// a test.
    struct RouteEnv {
        previous: Vec<(&'static str, Option<String>)>,
        _guard: MutexGuard<'static, ()>,
    }

    impl RouteEnv {
        /// Takes the lock, remembers every route variable, and clears them, so
        /// a test starts from a known environment rather than the developer's.
        fn cleared() -> Self {
            let guard = env_lock();
            let previous = ROUTE_ENV_VARS
                .iter()
                .map(|name| (*name, std::env::var(name).ok()))
                .collect();
            for name in ROUTE_ENV_VARS {
                // SAFETY: `guard` is held and is moved into the returned value,
                // so no other test in this binary reads or writes these
                // variables for as long as this one can mutate them.
                unsafe { std::env::remove_var(name) };
            }
            Self {
                previous,
                _guard: guard,
            }
        }

        fn set(&self, name: &str, value: &str) {
            // SAFETY: `self` owns the lock guard, so this mutation is exclusive.
            unsafe { std::env::set_var(name, value) };
        }
    }

    impl Drop for RouteEnv {
        fn drop(&mut self) {
            for (name, value) in &self.previous {
                // SAFETY: the guard field is still alive during `drop`; fields
                // are dropped after this body runs.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

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
        let _env = RouteEnv::cleared();
        let (target, health, cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));
        assert_eq!(
            (target.scheme.as_str(), target.host.as_str(), target.port),
            ("https", "api.anthropic.com", Some(443)),
            "with no base URL configured the route is the production endpoint"
        );
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

    /// The authorized Anthropic target follows the configured base URL.
    ///
    /// Set through the process environment, which is what the client reads, so
    /// this exercises the same path a proxy deployment takes rather than a
    /// parallel one.
    #[test]
    fn a_configured_base_url_becomes_the_authorized_target() {
        let env = RouteEnv::cleared();
        env.set(
            "LEGION_ANTHROPIC_BASE_URL",
            "https://proxy.internal:8443/v1",
        );
        let (target, _health, _cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));
        assert_eq!(
            (target.scheme.as_str(), target.host.as_str(), target.port),
            ("https", "proxy.internal", Some(8443)),
            "the authorized target must follow the base URL the client reads"
        );
        assert_eq!(
            privacy,
            ProposalPrivacyLabel::ExternalEgressMetadata,
            "a proxy is still off-machine"
        );
    }

    /// Local backends stay local, and say so.
    #[test]
    fn local_backends_stay_loopback_and_workspace_scoped() {
        let _env = RouteEnv::cleared();
        for backend in [None, Some(ProductAiLiveBackend::Ollama)] {
            let (target, _health, cost, privacy) = route_descriptor_for_backend(backend);
            assert_eq!(
                (target.host.as_str(), target.port),
                ("localhost", Some(11434)),
                "{backend:?} left the loopback"
            );
            assert_eq!(cost, "local.free");
            assert_eq!(
                privacy,
                ProposalPrivacyLabel::WorkspaceMetadata,
                "{backend:?} does not leave the machine, so it must not claim egress"
            );
        }
    }

    /// The Ollama target follows `OLLAMA_BASE_URL`, and only the Ollama one.
    ///
    /// `OllamaProvider::default` and the reachability probe both read this
    /// variable, so a deployment on a non-default port receives the excerpt
    /// there while a hard-coded descriptor authorized and audited
    /// `localhost:11434` -- the same destination mismatch the Anthropic arm was
    /// fixed for, in the arm nobody re-checked.
    ///
    /// The deterministic backend is asserted alongside it because it must *not*
    /// follow the variable: nothing is sent on that route, and inheriting a
    /// configured endpoint would have the audit name a server the fixture never
    /// contacts.
    #[test]
    fn a_configured_ollama_endpoint_becomes_the_authorized_target() {
        let env = RouteEnv::cleared();
        env.set("OLLAMA_BASE_URL", "http://127.0.0.1:11500");

        let (target, _health, cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Ollama));
        assert_eq!(
            (target.scheme.as_str(), target.host.as_str(), target.port),
            ("http", "127.0.0.1", Some(11500)),
            "the Ollama route must name the configured endpoint"
        );
        assert_eq!(
            cost, "local.free",
            "a configured loopback endpoint is still local and still free"
        );
        assert_eq!(privacy, ProposalPrivacyLabel::WorkspaceMetadata);

        let (deterministic, _, _, _) = route_descriptor_for_backend(None);
        assert_eq!(
            deterministic.port,
            Some(11434),
            "the deterministic route sends nothing, so it must not adopt a configured Ollama endpoint"
        );
    }

    /// The parser, without the process environment.
    ///
    /// The env-backed tests above prove the descriptor is *wired* to the
    /// configuration; these prove it *parses*, and they need no lock, no
    /// `unsafe` and no cleanup, so the shapes can be covered exhaustively
    /// without four more racing tests.
    #[test]
    fn base_urls_parse_to_the_endpoint_the_client_would_contact() {
        let cases: [(&str, &str, &str, Option<u16>); 7] = [
            // Scheme and explicit port are carried verbatim.
            (
                "https://proxy.internal:8443/v1",
                "https",
                "proxy.internal",
                Some(8443),
            ),
            ("http://127.0.0.1:11500", "http", "127.0.0.1", Some(11500)),
            // No port: the scheme's default, because that is the port the HTTP
            // client actually opens. Substituting a service's conventional port
            // would name a destination the request never reaches.
            (
                "https://api.anthropic.com",
                "https",
                "api.anthropic.com",
                Some(443),
            ),
            (
                "http://ollama.internal",
                "http",
                "ollama.internal",
                Some(80),
            ),
            // A path is not part of the endpoint.
            (
                "https://gateway.example/anthropic/v1",
                "https",
                "gateway.example",
                Some(443),
            ),
            // Surrounding whitespace survives a hand-edited env file.
            (
                "  https://proxy.internal:8443  ",
                "https",
                "proxy.internal",
                Some(8443),
            ),
            // No scheme at all: assume the safe one rather than downgrading.
            ("proxy.internal:8443", "https", "proxy.internal", Some(8443)),
        ];
        for (base, scheme, host, port) in cases {
            let target = network_target_from_base_url(base, "fallback.invalid");
            assert_eq!(
                (target.scheme.as_str(), target.host.as_str(), target.port),
                (scheme, host, port),
                "parsing {base:?}"
            );
        }
    }

    /// An unusable base URL falls back rather than authorizing an empty host.
    ///
    /// An empty host in a `NetworkTarget` is not a harmless blank: it is a
    /// target the broker cannot match against any allowlist entry, and an audit
    /// record naming nowhere.
    #[test]
    fn an_empty_authority_falls_back_to_the_known_host() {
        let target = network_target_from_base_url("https:///v1", "api.anthropic.com");
        assert_eq!(target.host, "api.anthropic.com");
        assert!(!target.host.is_empty());
    }
}
