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
/// A base URL as parsed, with the detail `NetworkTarget` has nowhere to put.
///
/// A local type rather than a field on the protocol DTO: whether a port was
/// written down is a fact about the *text*, useful only while rewriting it, and
/// nothing downstream of the route descriptor has any use for it.
struct ParsedBaseUrl {
    target: legion_protocol::NetworkTarget,
    /// Whether the URL named a port, rather than one being supplied for the scheme.
    port_was_explicit: bool,
    /// Userinfo as written, without the `@`, when the URL carried any.
    ///
    /// Deliberately absent from `target`: a proxy password must not reach a
    /// record that carries a metadata-only redaction hint. It is returned
    /// rather than dropped because the *client* URL needs it, and computing it
    /// a second time from the same grammar is how this file has already lost a
    /// query, a set of IPv6 brackets and this credential in turn.
    userinfo: Option<String>,
    /// Everything after the authority: path, query and fragment as written.
    path_and_query: String,
    /// The scheme the URL named, lowercased, when it named one at all.
    declared_scheme: Option<String>,
}

/// The network target a base URL addresses.
fn network_target_from_base_url(base: &str, fallback_host: &str) -> legion_protocol::NetworkTarget {
    parse_base_url(base, fallback_host).target
}

/// A base URL with its scheme written out, if it was only implied.
fn normalized_with_scheme(base_url: &str, scheme: &str) -> String {
    if base_url.contains("://") {
        return base_url.to_string();
    }
    format!("{scheme}://{base_url}")
}

/// The one URL parser in this module.
///
/// Everything that needs a piece of authority context reads it from here.
/// Asking the text a second question meant a second copy of the same eight
/// lines -- trim, find the scheme, cut at `/`, `?` or `#`, drop userinfo, split
/// the port -- which is how a third gets written and how two end up disagreeing.
fn parse_base_url(base: &str, fallback_host: &str) -> ParsedBaseUrl {
    let trimmed = base.trim();
    // Schemes are case-insensitive per RFC 3986, and a case-sensitive check here
    // was a way past the TLS enforcement rather than a cosmetic gap:
    // `HTTP://proxy.internal` matched neither prefix, fell to the default arm,
    // and was labelled `https` -- so `enforce_https_for_remote` saw nothing to
    // correct and the client sent the credential over plaintext while the audit
    // record claimed TLS.
    // Found by position rather than by arithmetic on two strings. Slicing the
    // original by the length of a slice of the lowercased one is correct only
    // while `to_ascii_lowercase` preserves byte length -- true today, stated
    // nowhere, enforced by nothing, and exactly the kind of cleverness the next
    // reader simplifies back into the case-sensitive bug this replaced.
    let (scheme, rest, declared_scheme) = match trimmed.find("://") {
        Some(scheme_end) => {
            let rest = &trimmed[scheme_end + "://".len()..];
            let declared = trimmed[..scheme_end].to_ascii_lowercase();
            // Only the two schemes this client speaks are honoured. Anything
            // else used to be recorded as HTTPS while
            // `enforce_https_for_remote` handed the original string to the
            // client -- so policy authorized `https://proxy.internal:443` and
            // the request went to `ftp://proxy.internal`, a different route
            // that policy never saw. An unknown scheme is treated as HTTPS for
            // the *target* and reported here so the caller can refuse it.
            let scheme = if declared == "http" { "http" } else { "https" };
            (scheme, rest, Some(declared))
        }
        None => ("https", trimmed, None),
    };
    // The authority ends at the first `/`, `?` or `#`.
    //
    // Splitting on `/` alone kept a query in the host:
    // `https://proxy.internal?token=secret` recorded
    // `proxy.internal?token=secret`, so policy authorized and audited a
    // destination the client never connects to, and a credential went into
    // route metadata carrying a metadata-only redaction hint. The same defect
    // as the userinfo one, through a different delimiter -- which is the
    // argument for terminating on all of them at once rather than adding them
    // as they are reported.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    // Everything after it, kept as written: the client needs the path, and the
    // query is often where a gateway's token lives.
    let path_and_query = rest
        .find(['/', '?', '#'])
        .map(|index| {
            let tail = &rest[index..];
            if tail.starts_with('/') {
                tail.to_string()
            } else {
                format!("/{tail}")
            }
        })
        .unwrap_or_default();
    // Userinfo is not part of the host, and keeping it was worse than untidy.
    //
    // `https://user:secret@proxy.internal/v1` produced a `NetworkTarget.host` of
    // `user:secret@proxy.internal`. The policy then allowlisted and authorized
    // that string while the HTTP client connected to `proxy.internal` -- so the
    // decision was made about a host nothing talks to -- and the password was
    // copied into route metadata that carries a metadata-only redaction hint,
    // which is precisely the promise that the value is safe to keep.
    //
    // The last `@` wins: userinfo may contain a percent-encoded one, a host may
    // not contain any.
    let (userinfo, authority) = authority
        .rsplit_once('@')
        .map_or((None, authority), |(userinfo, host)| {
            (Some(userinfo.to_string()), host)
        });
    let mut parts = authority.rsplitn(2, ':');
    // An IPv6 literal is written `[::1]` in a URL and `::1` everywhere else.
    //
    // Keeping the brackets meant the product allowlisted `[::1]` while an org
    // bundle -- and the rest of this repository, and every operator writing a
    // list by hand -- says `::1`, so the ceiling intersection removed a host
    // both sides had permitted and denied a local provider outright. Brackets
    // belong to URL syntax and not to the host.
    let unbracket = |host: String| {
        host.strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .map_or(host.clone(), str::to_string)
    };
    let (host, port, port_was_explicit) = match (parts.next(), parts.next()) {
        // `rsplitn` yields the tail first, so a parsed port means an explicit
        // one; anything else is a bare host, including an IPv6 literal.
        (Some(tail), Some(head)) if tail.parse::<u16>().is_ok() => {
            (unbracket(head.to_string()), tail.parse::<u16>().ok(), true)
        }
        _ => (
            unbracket(authority.to_string()),
            Some(if scheme == "http" { 80 } else { 443 }),
            false,
        ),
    };
    let target = legion_protocol::NetworkTarget {
        scheme: scheme.to_string(),
        host: if host.is_empty() {
            fallback_host.to_string()
        } else {
            host
        },
        port,
    };
    ParsedBaseUrl {
        userinfo,
        path_and_query,
        declared_scheme,
        target,
        port_was_explicit,
    }
}

/// Whether a host is a loopback address, and therefore never leaves the machine.
///
/// The broker's definition, not a second one. This decides both whether
/// plaintext is acceptable here and whether the host is added to the policy
/// allowlist, and the broker then re-decides it when the request is made -- so
/// any disagreement between the two shows up as a request that was authorized
/// locally and denied centrally.
///
/// Textual rather than resolved, on purpose: a DNS lookup that can be repointed
/// is not a basis for deciding whether to send a credential in the clear.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    legion_security::policy::is_loopback_host(host)
}

/// A configured Anthropic base URL with plaintext refused off-machine.
///
/// **Applied to the URL, not to a copy of it.** An earlier version rewrote only
/// the `NetworkTarget` used by the broker and the audit record, while
/// `anthropic_client_with_keyring_fallback` still handed the raw environment
/// value to the client -- so the credential and the buffer excerpt went over
/// HTTP anyway, and the authorized route now *disagreed* with the real one. That
/// is the same defect the descriptor exists to prevent, introduced by the fix
/// for it. Enforcing here means the client, the broker allowlist and the audit
/// record all read one already-corrected string.
///
/// Upgraded rather than silently honoured: if the proxy does not speak TLS the
/// request fails visibly, which is the right outcome for a credential-bearing
/// call. Loopback keeps `http` -- it never leaves the machine, and demanding TLS
/// there would break local proxies for nothing.
pub(crate) fn enforce_https_for_remote(base_url: &str) -> String {
    let parsed = parse_base_url(base_url, "api.anthropic.com");
    let target = &parsed.target;
    // A scheme this client does not speak is rebuilt, not passed through.
    //
    // `ftp://proxy.internal` was recorded as HTTPS for policy while the client
    // received the original string, so the authorization and the request named
    // different routes -- the exact split this function exists to close. Only
    // `http` and `https` survive as written; anything else is reconstructed as
    // the HTTPS the record already claims, which is the one destination policy
    // approved.
    let unsupported_scheme = parsed
        .declared_scheme
        .as_deref()
        .is_some_and(|scheme| scheme != "http" && scheme != "https");
    if !unsupported_scheme && (target.scheme != "http" || is_loopback_host(&target.host)) {
        // Whatever the caller wrote, with the scheme the parser inferred.
        //
        // A scheme-less `proxy.internal:8443` is authorized as
        // `https://proxy.internal:8443` -- the parser defaults to HTTPS -- and
        // was then handed to the client unchanged, so reqwest received
        // `proxy.internal:8443/v1/messages` and every request failed against a
        // destination policy had already approved. Returning the text as
        // written is only correct while the text names a scheme.
        return normalized_with_scheme(base_url.trim(), &target.scheme);
    }
    // A port written down is a choice; a port supplied for the scheme is not.
    //
    // `network_target_from_base_url` fills in 80 for a bare `http://` URL, so
    // by the time it is read here an omitted port and an explicit `:80` look
    // identical -- and rewriting both to 443 contradicted the promise on this
    // very branch to keep what the operator configured. A proxy terminating
    // TLS on 80 is unusual and entirely legal, and moving it silently points
    // the client somewhere nothing is listening.
    //
    // Asked of the URL text instead, where the difference is still visible.
    // Whether the port was written down comes from the parser that read it.
    let port = if parsed.port_was_explicit {
        target.port.unwrap_or(443)
    } else {
        443
    };
    // The path, query, fragment and userinfo all come from the one parser.
    //
    // Re-deriving them here was three copies of the same grammar, and this file
    // has already lost a query, a set of IPv6 brackets and a proxy credential
    // one at a time to exactly that duplication. `parse_base_url` computes each
    // of them on the way to the target; it returns them now instead of throwing
    // them away.
    let path = parsed.path_and_query.clone();
    let userinfo = parsed
        .userinfo
        .as_ref()
        .map(|userinfo| format!("{userinfo}@"))
        .unwrap_or_default();
    format!(
        "https://{}{}:{}{}",
        userinfo,
        url_authority_host(&target.host),
        port,
        path
    )
}

/// A host as a URL authority writes it: an IPv6 literal wears its brackets.
fn url_authority_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

/// The Anthropic endpoint this build will actually contact.
///
/// Parsed from the configured base URL -- already HTTPS-enforced by
/// [`enforce_https_for_remote`] -- so the authorized target, the audit record
/// and the request all name one destination.
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

    /// Userinfo never reaches the authorized host, or the audit record.
    ///
    /// `https://user:secret@proxy.internal/v1` gave a host of
    /// `user:secret@proxy.internal`. The policy allowlisted and authorized that
    /// string while the client connected to `proxy.internal` -- a decision made
    /// about a host nothing talks to -- and the password was copied into route
    /// metadata carrying a metadata-only redaction hint, which is exactly the
    /// promise that the value is safe to keep.
    #[test]
    fn userinfo_is_not_part_of_the_authorized_host() {
        for base in [
            "https://user:secret@proxy.internal/v1",
            "https://user@proxy.internal/v1",
            "HTTPS://user:secret@proxy.internal:8443/v1",
        ] {
            let target = super::network_target_from_base_url(base, "api.anthropic.com");
            assert!(
                !target.host.contains('@'),
                "{base} left userinfo in the host: {}",
                target.host
            );
            assert!(
                !target.host.contains("secret"),
                "{base} carried a credential into the route metadata: {}",
                target.host
            );
            assert_eq!(
                target.host, "proxy.internal",
                "{base} must authorize the host the client actually connects to"
            );
        }
    }

    /// A scheme-less base URL is handed to the client with its scheme.
    ///
    /// `proxy.internal:8443` is an explicitly supported form: the parser infers
    /// HTTPS and the broker authorizes `https://proxy.internal:8443`. Returning
    /// the text unchanged handed reqwest `proxy.internal:8443/v1/messages`, so
    /// every request failed against a destination policy had already approved
    /// -- a failure that looks like the provider being down and is not.
    #[test]
    fn a_scheme_less_base_url_is_normalized_for_the_client() {
        for base in ["proxy.internal:8443", "proxy.internal", "api.anthropic.com"] {
            let normalized = super::enforce_https_for_remote(base);
            assert!(
                normalized.starts_with("https://"),
                "{base} reached the client without a scheme, as {normalized:?}"
            );
            assert!(
                normalized.contains(base),
                "{base} lost part of itself in normalization, becoming {normalized:?}"
            );
        }

        // A URL that already names its scheme is untouched.
        assert_eq!(
            super::enforce_https_for_remote("https://api.anthropic.com"),
            "https://api.anthropic.com",
            "an https URL must pass through exactly as written"
        );
    }

    /// An IPv6 host is recorded the way an allowlist is written.
    ///
    /// A URL spells the literal `[::1]` and every list written by hand -- and
    /// the rest of this repository -- spells it `::1`. Keeping the brackets
    /// made the ceiling intersection remove a host both the product and the org
    /// had permitted, denying a local provider outright.
    #[test]
    fn a_bracketed_ipv6_host_is_stored_without_its_brackets() {
        for (base, expected_host, expected_port) in [
            ("http://[::1]:11434", "::1", Some(11434)),
            ("http://[::1]", "::1", Some(80)),
            ("https://[2001:db8::1]:8443", "2001:db8::1", Some(8443)),
        ] {
            let target = super::network_target_from_base_url(base, "api.anthropic.com");
            assert_eq!(
                target.host, expected_host,
                "{base} kept URL bracket syntax in the host, where an allowlist entry \
                 written {expected_host:?} can never match it"
            );
            assert_eq!(target.port, expected_port, "{base} lost or invented a port");
        }
    }

    /// A scheme this client does not speak does not reach the client.
    ///
    /// `ftp://proxy.internal` was recorded as HTTPS for policy while the client
    /// received the original string -- so the authorization and the request
    /// named different routes, which is the split this function exists to
    /// close.
    #[test]
    fn an_unsupported_scheme_is_rebuilt_rather_than_passed_through() {
        for base in ["ftp://proxy.internal", "gopher://proxy.internal/v1"] {
            let client = super::enforce_https_for_remote(base);
            assert!(
                client.starts_with("https://"),
                "{base} reached the client as {client}, which is not the route policy authorized"
            );
            let target = super::network_target_from_base_url(base, "api.anthropic.com");
            assert_eq!(
                target.scheme, "https",
                "{base} was authorized as something other than what the client will use"
            );
        }

        // The two schemes this client does speak are still passed through as
        // written, loopback included.
        assert_eq!(
            super::enforce_https_for_remote("https://api.anthropic.com"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            super::enforce_https_for_remote("http://[::1]:11434"),
            "http://[::1]:11434"
        );
    }

    /// A proxy credential survives the upgrade, and stays out of the record.
    ///
    /// Userinfo is stripped from `NetworkTarget` on purpose -- an audit trail
    /// should not carry a proxy password -- and rebuilding the client URL from
    /// that stripped host dropped the credential from the request too, so an
    /// authenticated proxy rejected every call.
    #[test]
    fn a_proxy_credential_survives_the_upgrade_but_not_the_record() {
        assert_eq!(
            super::enforce_https_for_remote("http://user:pass@proxy.internal/v1"),
            "https://user:pass@proxy.internal:443/v1",
            "the client lost the proxy credential in the upgrade"
        );

        let target = super::network_target_from_base_url(
            "http://user:pass@proxy.internal/v1",
            "api.anthropic.com",
        );
        assert_eq!(
            target.host, "proxy.internal",
            "the authorized target must name the host and not the credential"
        );
        assert!(
            !format!("{target:?}").contains("pass"),
            "a proxy password reached the policy record: {target:?}"
        );
    }

    /// A gateway's query survives the upgrade to HTTPS.
    ///
    /// Dropping it was a mistake in the safer-looking direction: a gateway's
    /// token often *is* the query, so the upgraded URL reached the right host
    /// without the credential and every request failed against an endpoint
    /// policy had authorized -- which reads as the provider rejecting the key.
    #[test]
    fn a_query_survives_the_upgrade_to_https() {
        for (base, expected) in [
            (
                "http://proxy.internal/anthropic?token=secret",
                "https://proxy.internal:443/anthropic?token=secret",
            ),
            (
                "http://proxy.internal?token=secret",
                "https://proxy.internal:443/?token=secret",
            ),
            (
                "http://proxy.internal/anthropic",
                "https://proxy.internal:443/anthropic",
            ),
            ("http://proxy.internal", "https://proxy.internal:443"),
        ] {
            assert_eq!(
                super::enforce_https_for_remote(base),
                expected,
                "{base} lost part of itself in the upgrade"
            );
        }
    }

    /// An IPv6 host is bracketed again when it is written back into a URL.
    ///
    /// The policy representation and the URL representation are different
    /// strings on purpose: an allowlist says `::1` and a URL authority has to
    /// say `[::1]`, or the colons in the address are read as a port separator.
    /// Normalizing for the first and then reusing it for the second built
    /// `https://2001:db8::1:8080/v1`, which nothing can parse -- so every
    /// request failed against an endpoint policy had just authorized.
    #[test]
    fn an_ipv6_host_is_bracketed_again_when_it_becomes_a_url() {
        for (base, expected) in [
            (
                "http://[2001:db8::1]:8080/v1",
                "https://[2001:db8::1]:8080/v1",
            ),
            // Loopback keeps its scheme -- and its brackets, which is the
            // half of this the early return has always got right.
            ("http://[::1]:11434", "http://[::1]:11434"),
            (
                "http://proxy.internal:8080/v1",
                "https://proxy.internal:8080/v1",
            ),
        ] {
            assert_eq!(
                super::enforce_https_for_remote(base),
                expected,
                "{base} was rewritten into a URL the client cannot parse"
            );
        }
    }

    /// A port the operator wrote down survives, including port 80.
    ///
    /// Parsing fills in 80 for a bare `http://` URL, so by the time the rewrite
    /// reads it an omitted port and an explicit `:80` are indistinguishable --
    /// and moving both to 443 contradicted the promise to keep what was
    /// configured. A proxy terminating TLS on 80 is unusual and entirely legal.
    #[test]
    fn an_explicitly_configured_port_survives_the_https_upgrade() {
        assert_eq!(
            super::enforce_https_for_remote("http://proxy.internal:80/v1"),
            "https://proxy.internal:80/v1",
            "an explicitly configured port 80 was rewritten to 443"
        );
        assert_eq!(
            super::enforce_https_for_remote("http://proxy.internal/v1"),
            "https://proxy.internal:443/v1",
            "an omitted port must move to the HTTPS default"
        );
        assert_eq!(
            super::enforce_https_for_remote("http://proxy.internal:8080/v1"),
            "https://proxy.internal:8080/v1",
            "a nonstandard port must be kept, as it already was"
        );
    }

    /// A query or fragment is not part of the host either.
    ///
    /// The same defect as userinfo through a different delimiter: splitting the
    /// authority on `/` alone recorded `proxy.internal?token=secret` as the
    /// host, so policy authorized and audited a destination nothing connects to
    /// and a credential went into metadata marked safe to retain.
    #[test]
    fn a_query_or_fragment_is_not_part_of_the_authorized_host() {
        for base in [
            "https://proxy.internal?token=secret",
            "https://proxy.internal#fragment",
            "https://proxy.internal:8443?token=secret",
        ] {
            let target = super::network_target_from_base_url(base, "api.anthropic.com");
            assert_eq!(
                target.host, "proxy.internal",
                "{base} authorized a host the client never connects to"
            );
            assert!(
                !target.host.contains("secret"),
                "{base} carried a credential into the route metadata"
            );
        }
    }

    /// An explicit port still parses once userinfo is gone.
    #[test]
    fn a_port_survives_userinfo_removal() {
        let target = super::network_target_from_base_url(
            "https://user:secret@proxy.internal:8443/v1",
            "api.anthropic.com",
        );
        assert_eq!(target.port, Some(8443), "the configured port was lost");
    }

    /// A proxy path survives the upgrade to HTTPS, whatever case the scheme is in.
    ///
    /// Scheme detection was made case-insensitive and the path extraction was
    /// not, so `HTTP://proxy.internal/anthropic` -- accepted by one half and
    /// rejected by the other -- was rewritten to `https://proxy.internal:443`.
    /// The path is where the proxy routes on, so requests went to a different
    /// endpoint from the configured one while everything reported success.
    #[test]
    fn upgrading_a_mixed_case_proxy_keeps_its_path() {
        for base in [
            "HTTP://proxy.internal/anthropic",
            "http://proxy.internal/anthropic",
            "HtTp://proxy.internal/anthropic",
        ] {
            assert_eq!(
                super::enforce_https_for_remote(base),
                "https://proxy.internal:443/anthropic",
                "{base} lost its path, so the client would be pointed at a different \
                 endpoint from the one configured"
            );
        }
    }

    /// A path with several segments survives whole.
    #[test]
    fn upgrading_keeps_every_path_segment() {
        assert_eq!(
            super::enforce_https_for_remote("HTTP://proxy.internal:8080/v1/anthropic/messages"),
            "https://proxy.internal:8080/v1/anthropic/messages",
            "an explicitly configured port stays, and so does the whole path"
        );
    }

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

    /// A non-loopback Anthropic endpoint is never contacted over plaintext.
    ///
    /// `http://proxy.internal` would otherwise produce an HTTP target that
    /// `product_ai_security_policy` allowlists and the generic
    /// `ai.provider.invoke` policy accepts — putting the BYOK credential and the
    /// raw buffer excerpt on the wire in clear. Upgraded rather than silently
    /// honoured: if the proxy does not speak TLS the request fails visibly,
    /// which is the right outcome for a credential-bearing call.
    #[test]
    fn a_non_loopback_anthropic_endpoint_is_forced_to_https() {
        let env = RouteEnv::cleared();
        env.set("LEGION_ANTHROPIC_BASE_URL", "http://proxy.internal/v1");
        let (target, _health, _cost, _privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));

        assert_eq!(
            target.scheme, "https",
            "a remote Anthropic endpoint must not be contacted over plaintext"
        );
        assert_eq!(
            target.port,
            Some(443),
            "the port must move with the scheme when it was HTTP's default"
        );
        assert_eq!(target.host, "proxy.internal");
    }

    /// An explicitly configured port survives the scheme upgrade.
    #[test]
    fn forcing_https_keeps_a_configured_port() {
        let env = RouteEnv::cleared();
        env.set("LEGION_ANTHROPIC_BASE_URL", "http://proxy.internal:8443/v1");
        let (target, _health, _cost, _privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));

        assert_eq!(target.scheme, "https");
        assert_eq!(
            target.port,
            Some(8443),
            "an operator's explicit port is what they asked for and must be kept"
        );
    }

    /// The *URL* is corrected, not just the descriptor's view of it.
    ///
    /// The first attempt at this rewrote only the `NetworkTarget` handed to the
    /// broker and the audit record, while the client was still constructed from
    /// the raw environment value — so the credential went over HTTP anyway and
    /// the authorized route now disagreed with the real one, which is worse than
    /// the defect it was meant to fix.
    ///
    /// Asserting on the string every consumer reads is what makes that
    /// impossible: there is no second copy left to correct.
    #[test]
    fn the_configured_url_itself_is_upgraded_not_just_the_target() {
        assert_eq!(
            super::enforce_https_for_remote("http://proxy.internal/v1"),
            "https://proxy.internal:443/v1",
            "a remote endpoint must be corrected in the URL the client is built from"
        );
        assert_eq!(
            super::enforce_https_for_remote("http://proxy.internal:8443/v1"),
            "https://proxy.internal:8443/v1",
            "an explicitly configured port is the operator's choice and stays"
        );
        // Loopback is left exactly as configured — including its path and the
        // absence of an added port, since nothing needed correcting.
        assert_eq!(
            super::enforce_https_for_remote("http://localhost:8080/v1"),
            "http://localhost:8080/v1",
            "a loopback proxy never leaves the machine, so nothing is rewritten"
        );
        assert_eq!(
            super::enforce_https_for_remote("https://api.anthropic.com"),
            "https://api.anthropic.com",
            "an already-secure endpoint is untouched"
        );
    }

    /// A loopback Anthropic proxy keeps plaintext, and is a supported setup.
    ///
    /// The first version of the remote test asserted that an Anthropic route is
    /// never loopback, which rejected exactly this configuration — a standing
    /// gate refusing a deployment the product supports.
    #[test]
    fn a_loopback_anthropic_proxy_is_allowed_over_http() {
        let env = RouteEnv::cleared();
        env.set("LEGION_ANTHROPIC_BASE_URL", "http://localhost:8080/v1");
        let (target, _health, _cost, privacy) =
            route_descriptor_for_backend(Some(ProductAiLiveBackend::Anthropic));

        assert_eq!(
            (target.scheme.as_str(), target.host.as_str(), target.port),
            ("http", "localhost", Some(8080)),
            "a loopback proxy never leaves the machine, so TLS is not required of it"
        );
        // Still an egress route as far as the label is concerned: what the
        // proxy does with the excerpt afterwards is not something this can see.
        assert_eq!(privacy, ProposalPrivacyLabel::ExternalEgressMetadata);
    }

    /// An upper-case scheme is still plaintext.
    ///
    /// Schemes are case-insensitive per RFC 3986, and a case-sensitive check was
    /// a way *past* the TLS enforcement rather than a cosmetic gap:
    /// `HTTP://proxy.internal` matched neither prefix, fell to the default arm
    /// and was labelled `https` — so nothing was corrected, and the client sent
    /// the credential over plaintext while the audit record claimed TLS.
    #[test]
    fn an_upper_case_scheme_is_recognised_and_still_upgraded() {
        for configured in [
            "HTTP://proxy.internal/v1",
            "Http://proxy.internal/v1",
            "hTTp://proxy.internal/v1",
        ] {
            let upgraded = super::enforce_https_for_remote(configured);
            assert!(
                upgraded.starts_with("https://"),
                "{configured} was not recognised as plaintext: {upgraded}"
            );
        }

        // The host keeps its case: only the scheme is matched insensitively.
        let target = super::network_target_from_base_url("HTTPS://Proxy.Internal:8443", "fallback");
        assert_eq!(
            (target.scheme.as_str(), target.host.as_str(), target.port),
            ("https", "Proxy.Internal", Some(8443)),
            "matching the scheme case-insensitively must not rewrite the host"
        );
    }

    /// Loopback recognition covers the forms a person actually configures.
    #[test]
    fn loopback_hosts_are_recognised_by_form_not_by_lookup() {
        // The same predicate the broker applies when the request is actually
        // made, so a host accepted here cannot be denied there.
        for host in [
            "localhost",
            "127.0.0.2",
            "LOCALHOST",
            "127.0.0.1",
            "127.0.0.53",
            "::1",
            "[::1]",
        ] {
            assert!(
                super::is_loopback_host(host),
                "{host} should be recognised as loopback"
            );
        }
        for host in ["proxy.internal", "api.anthropic.com", "10.0.0.5", "0.0.0.0"] {
            assert!(
                !super::is_loopback_host(host),
                "{host} is not loopback and must not be treated as one"
            );
        }
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
