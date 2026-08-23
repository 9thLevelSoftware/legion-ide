//! Why the deterministic fixture answered instead of a model.
//!
//! Extracted verbatim from `lib.rs` under ADR-0049 cross-cutting rule 1: the
//! chokepoint grows only by being moved out of. Nothing here changed in the
//! move.
//!
//! Everything is an `AppComposition`-free free function, so `lib.rs` calls it
//! by name and nothing else moves with it.

use crate::*;

/// Resolve Anthropic API key from env (preferred) or OS keyring BYOK storage.
///
/// Desktop `SetProviderApiKey` writes to the keyring; this path loads that secret
/// when `ANTHROPIC_API_KEY` (and Legion prefixes) are unset.
#[cfg(feature = "ai")]
pub(crate) fn resolve_anthropic_api_key() -> Option<String> {
    let (key, state) = match anthropic_api_key_from_env() {
        Some(key) => (Some(key), AnthropicKeyState::Present),
        None => {
            // Desktop SetProviderApiKey stores `anthropic:api_key`; also accept
            // legacy `ANTHROPIC_API_KEY` account names.
            match load_provider_api_key(&OsKeyringSecretStore, "anthropic") {
                Ok(Some(key)) => (Some(key), AnthropicKeyState::Present),
                Ok(None) => (None, AnthropicKeyState::Absent),
                Err(error) => (
                    None,
                    AnthropicKeyState::KeyringUnreadable(error.to_string()),
                ),
            }
        }
    };
    record_anthropic_key_state(state);
    key
}

/// What the credential lookup found, kept so the diagnosis need not ask again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnthropicKeyState {
    /// A key was resolved, from the environment or the keyring.
    Present,
    /// The keyring answered, and holds no key.
    Absent,
    /// The keyring could not be read, so nothing is known either way.
    KeyringUnreadable(String),
}

/// The most recent credential lookup, for the diagnosis that follows it.
///
/// The diagnosis explains a backend selection that has *already happened*, and
/// looking the credential up a second time to explain the first is two ways
/// wrong. The answers can disagree -- a keyring unlocked in between turns "it
/// failed" into "there is no key", which is the false message this whole change
/// removes -- and each lookup can put a system prompt in front of the operator,
/// so the second one asks them to authorise a read taken purely to write a
/// sentence.
static LAST_ANTHROPIC_KEY_STATE: std::sync::Mutex<Option<AnthropicKeyState>> =
    std::sync::Mutex::new(None);

/// The lock, with the poison rule in one place.
///
/// A test that fails while holding this poisons it, and the next reader still
/// needs the value: the guard is a plain `Option`, so a panic cannot have left
/// it half-written.
fn lock_state() -> std::sync::MutexGuard<'static, Option<AnthropicKeyState>> {
    LAST_ANTHROPIC_KEY_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Remember what a lookup found.
fn record_anthropic_key_state(state: AnthropicKeyState) {
    *lock_state() = Some(state);
}

/// Forget the last lookup, so a test's diagnosis cannot read another's answer.
///
/// The cache is process-global by design -- the selection and the diagnosis
/// that explains it are separate calls with no value passed between them -- and
/// that makes it shared state between tests in the same binary. `RouteEnv`
/// clears it alongside the route variables for the same reason it clears those.
#[cfg(test)]
pub(crate) fn forget_anthropic_key_state() {
    *lock_state() = None;
}

/// What the last lookup found, if one has happened in this process.
fn last_anthropic_key_state() -> Option<AnthropicKeyState> {
    lock_state().clone()
}

/// An Anthropic key supplied by the environment, in the order the client reads.
///
/// Separated from the keyring lookup because the diagnosis needs the two apart:
/// a key in the environment makes the keyring's state irrelevant, and a keyring
/// that cannot be read is a different problem from one holding nothing.
#[cfg(feature = "ai")]
fn anthropic_api_key_from_env() -> Option<String> {
    [
        "ANTHROPIC_API_KEY",
        "LEGION_ANTHROPIC_API_KEY",
        "DEVIL_ANTHROPIC_API_KEY",
    ]
    .into_iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

/// Why no live model answered, phrased for the person who has to fix it.
///
/// The product falls back to the deterministic fixture whenever no live
/// backend is reachable, and until now said only "no live credentials". For a
/// remote provider that is the reason. For Ollama and llama.cpp it is not: they
/// take no credential, and somebody who has never run one is sent looking for
/// an API key that does not exist while a one-line install would have fixed it.
///
/// Returns `None` when a fixture is what was actually asked for, because that
/// is a configuration rather than a failure and reporting it as one trains
/// people to ignore the message.
///
/// Naming the endpoint matters as much as the cause: the probe follows
/// `OLLAMA_BASE_URL` and the llama.cpp base-url names, so "not running" and
/// "running somewhere else" look identical from the outside and have different
/// fixes.
pub(crate) fn local_ai_unavailable_reason(
    preference: ProductAiProviderPreference,
) -> Option<String> {
    match preference {
        ProductAiProviderPreference::Deterministic => None,
        ProductAiProviderPreference::Anthropic => Some(anthropic_unavailable_reason()),
        ProductAiProviderPreference::Ollama => Some(ollama_unavailable_reason()),
        ProductAiProviderPreference::LlamaCpp => Some(llama_cpp_unavailable_reason()),
        ProductAiProviderPreference::Auto => Some(format!(
            "No local model server answered, so the deterministic fixture \
             answered instead.\n- {}\n- {}",
            ollama_unavailable_reason(),
            llama_cpp_unavailable_reason(),
        )),
    }
}

/// One local backend's line: not running, or configured somewhere unreachable.
///
/// A non-loopback endpoint is a different problem with a different fix, and
/// saying "it did not answer, try setting the URL" for one is actively
/// misleading -- the URL is already set, Legion never contacted it, and the
/// local-provider policy would refuse it if it had. `loopback_target_reachable`
/// filters every non-loopback address before connecting, so "unreachable" and
/// "not permitted" arrive here indistinguishable unless this asks.
///
/// Takes the parsed target rather than the configured string, because this text
/// becomes a proposal's `PreviewSummary.details` and is retained under a
/// metadata-only redaction hint. A loopback service behind an authenticated
/// proxy is configured as `http://user:token@127.0.0.1:11434`, and interpolating
/// the environment value verbatim wrote that token into proposal history --
/// while the route parser was carefully stripping the same credential out of
/// the audit record three lines away.
fn local_backend_reason(
    name: &str,
    target: &legion_protocol::NetworkTarget,
    default_port: u16,
    remedy: &str,
) -> String {
    let endpoint = crate::ai_route_descriptor::displayable_endpoint(target);
    match probe_reach(target, default_port) {
        ProbeReach::Loopback => format!("{name} did not answer at {endpoint}. {remedy}"),
        ProbeReach::NotLoopback => format!(
            "{name} is configured at {endpoint}, which is not a loopback address. \
             Legion's local-provider policy only reaches this machine, so that \
             endpoint was never contacted. Point it at localhost, or choose a \
             remote provider deliberately in settings."
        ),
        ProbeReach::Unresolvable => format!(
            "{name} is configured at {endpoint}, and that host name does not \
             resolve, so nothing was contacted. Check the spelling, or point it \
             at localhost."
        ),
    }
}

/// Ollama's port when the configured URL names none.
///
/// Shared with the probe rather than written twice: a diagnosis that assumed a
/// different port from the one that was tried would describe a different
/// endpoint from the one that failed.
pub(crate) const OLLAMA_DEFAULT_PORT: u16 = 11434;

/// `llama-server`'s port when the configured URL names none.
pub(crate) const LLAMA_CPP_DEFAULT_PORT: u16 = 8080;

/// What the probe would have found to connect to.
enum ProbeReach {
    /// At least one resolved address is loopback, so the probe tried it.
    Loopback,
    /// It resolves, but to nothing on this machine, so the probe skipped it.
    NotLoopback,
    /// The name does not resolve at all.
    Unresolvable,
}

/// Ask the question `loopback_target_reachable` asks, without connecting.
///
/// The probe resolves the host and keeps only the loopback addresses, so an
/// alias like `ollama.local` pointing at `127.0.0.1` **is** contacted. A textual
/// check on the host name says otherwise, and the diagnosis then told somebody
/// their endpoint had never been reached when it had been -- while their server
/// was simply down, which is the one thing the message did not say.
///
/// The same resolution the probe performs, so the two cannot disagree about
/// what counts as this machine.
fn probe_reach(target: &legion_protocol::NetworkTarget, default_port: u16) -> ProbeReach {
    use std::net::{SocketAddr, ToSocketAddrs};

    let port = target.port.unwrap_or(default_port);
    let Ok(mut addrs) = (target.host.as_str(), port).to_socket_addrs() else {
        return ProbeReach::Unresolvable;
    };
    let mut resolved_any = false;
    for addr in addrs.by_ref() {
        resolved_any = true;
        let loopback = match addr {
            SocketAddr::V4(v4) => v4.ip().is_loopback(),
            SocketAddr::V6(v6) => v6.ip().is_loopback(),
        };
        if loopback {
            return ProbeReach::Loopback;
        }
    }
    if resolved_any {
        ProbeReach::NotLoopback
    } else {
        ProbeReach::Unresolvable
    }
}

/// Why Anthropic did not serve this run.
///
/// "No credential is configured" was asserted for both of the ways
/// `resolve_anthropic_api_key` can return `None`. It discards the keyring's
/// error with `.ok().flatten()`, so a locked or unavailable OS keyring is
/// indistinguishable there from an empty one -- and somebody who stored a key
/// months ago was being told to go and add one, which is advice for a problem
/// they do not have about a credential that is already there.
///
/// The keyring is only consulted when the environment supplied nothing, because
/// an environment key makes its state irrelevant. That is the same order
/// `resolve_anthropic_api_key` reads them in, so this cannot report a keyring
/// failure for a run the keyring was never asked about.
#[cfg(feature = "ai")]
fn anthropic_unavailable_reason() -> String {
    // The selection that fell back has already asked, and its answer is the one
    // this is explaining. Asking again would be a second keyring read that can
    // disagree with the first and can prompt the operator a second time.
    //
    // Nothing recorded means no selection has run in this process -- a test
    // calling the diagnosis directly -- and then asking is the only way to have
    // anything to say.
    let state = last_anthropic_key_state().unwrap_or_else(|| {
        resolve_anthropic_api_key();
        // Not `unwrap_or(Absent)`. A lookup that returns without recording is a
        // refactor that dropped the call, and defaulting to Absent would print
        // "no credential is configured" for a run whose real failure nobody
        // asked about -- which is the exact false message this exists to remove.
        last_anthropic_key_state().expect("resolve_anthropic_api_key must record its result")
    });
    anthropic_reason(&state)
}

/// Without the provider there is no keyring lookup to have failed.
#[cfg(not(feature = "ai"))]
fn anthropic_unavailable_reason() -> String {
    anthropic_reason(&AnthropicKeyState::Absent)
}

/// The wording, given what the lookup found.
///
/// Split out and pure so every branch can be tested without arranging for a
/// locked keyring on the machine running the tests -- which is the reason the
/// keyring branch did not exist for as long as it did not.
pub(crate) fn anthropic_reason(state: &AnthropicKeyState) -> String {
    match state {
        AnthropicKeyState::KeyringUnreadable(error) => format!(
            "The OS keyring could not be read ({error}), so Legion cannot tell \
             whether an Anthropic credential is stored, and the deterministic \
             fixture answered instead. Unlock the keyring, or set \
             ANTHROPIC_API_KEY for this session."
        ),
        // Reached when the credential resolved but the run still fell back --
        // the provider was unreachable, or the route was refused downstream.
        // Saying "add a key" here would send somebody to fix the one thing that
        // is demonstrably working.
        AnthropicKeyState::Present => "An Anthropic credential is configured, so \
             something other than the credential stopped this run and the \
             deterministic fixture answered instead. Check the route detail for \
             the refusal or the provider error."
            .to_string(),
        AnthropicKeyState::Absent => "No Anthropic credential is configured, so \
             the deterministic fixture answered instead. Add a key in settings, \
             or choose a local provider."
            .to_string(),
    }
}

/// Why Ollama did not serve this run.
fn ollama_unavailable_reason() -> String {
    local_backend_reason(
        "Ollama",
        &crate::ai_route_descriptor::ollama_network_target(),
        OLLAMA_DEFAULT_PORT,
        "Start Ollama, or set OLLAMA_BASE_URL if yours listens elsewhere.",
    )
}

/// Why llama.cpp did not serve this run.
fn llama_cpp_unavailable_reason() -> String {
    local_backend_reason(
        "A llama.cpp server",
        &crate::ai_route_descriptor::llama_cpp_network_target(),
        LLAMA_CPP_DEFAULT_PORT,
        "Start `llama-server`, or set LEGION_LLAMA_CPP_BASE_URL if yours listens \
         elsewhere.",
    )
}

#[cfg(test)]
mod probe_reach_tests {
    use super::*;

    fn target(host: &str) -> legion_protocol::NetworkTarget {
        legion_protocol::NetworkTarget {
            scheme: "http".to_string(),
            host: host.to_string(),
            port: Some(11434),
        }
    }

    /// The probe's own question, answered without connecting.
    ///
    /// `loopback_target_reachable` resolves the host and keeps only loopback
    /// addresses, so the diagnosis has to ask the same question the same way.
    /// It used to ask `is_loopback_host`, which reads the host *as text* --
    /// true for `localhost` and for any loopback literal, and false for a name
    /// like `ollama.local` that resolves to `127.0.0.1`. That endpoint is
    /// genuinely probed, and the diagnosis said it had never been contacted.
    ///
    /// The alias itself is not asserted here: it needs a resolver entry this
    /// test cannot create, and inventing one would test the developer's `hosts`
    /// file. What is asserted is that the answer comes from resolution -- the
    /// unresolvable arm cannot exist under a textual check at all, and
    /// `an_unresolvable_host_is_reported_as_unresolvable` fails when the textual
    /// check is restored.
    #[test]
    fn probe_reach_reports_what_resolution_found() {
        assert!(
            matches!(
                probe_reach(&target("127.0.0.1"), 11434),
                ProbeReach::Loopback
            ),
            "a loopback literal is contacted"
        );
        assert!(
            matches!(probe_reach(&target("::1"), 11434), ProbeReach::Loopback),
            "and so is the IPv6 one"
        );
        // TEST-NET-1, reserved by RFC 5737 and routable nowhere.
        assert!(
            matches!(
                probe_reach(&target("192.0.2.1"), 11434),
                ProbeReach::NotLoopback
            ),
            "an address off this machine is not contacted"
        );
        assert!(
            matches!(
                probe_reach(&target("legion-no-such-host.invalid"), 11434),
                ProbeReach::Unresolvable
            ),
            "and a name that does not resolve is neither"
        );
    }
}
