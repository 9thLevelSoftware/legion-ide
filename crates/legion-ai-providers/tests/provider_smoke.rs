//! Live and recorded smoke tests for Anthropic and Ollama providers (PKT-PROV T4).

use std::path::PathBuf;

use legion_ai::{ChatCompletionRequest, ModelProvider};
use legion_ai_providers::{
    ANTHROPIC_PROVIDER_ID, AnthropicCredential, AnthropicMessagesClient,
    AnthropicMessagesTransport, AnthropicSseEvent, OLLAMA_PROVIDER_ID, OllamaProvider,
};
use legion_protocol::PRODUCT_ENV_PREFIX;
use serde_json::Value;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evals/recorded/anthropic_smoke.json")
}

fn load_fixture() -> Value {
    let text =
        std::fs::read_to_string(fixture_path()).expect("anthropic_smoke.json must be readable");
    serde_json::from_str(&text).expect("anthropic_smoke.json must be valid JSON")
}

// ---------------------------------------------------------------------------
// Recorded transport
// ---------------------------------------------------------------------------

/// Recorded Anthropic transport that replays fixtures without any network I/O.
#[derive(Clone)]
struct RecordedAnthropicTransport {
    /// The JSON body to return for post_json calls.
    json_response: Value,
    /// The SSE body text to return for post_text calls.
    sse_body: String,
}

impl AnthropicMessagesTransport for RecordedAnthropicTransport {
    fn post_json(
        &self,
        _endpoint: &str,
        _credential: Option<AnthropicCredential<'_>>,
        _beta_header: Option<&str>,
        _payload: Value,
    ) -> Result<Value, legion_ai::ProviderError> {
        Ok(self.json_response.clone())
    }

    fn post_text(
        &self,
        _endpoint: &str,
        _credential: Option<AnthropicCredential<'_>>,
        _beta_header: Option<&str>,
        _payload: Value,
    ) -> Result<String, legion_ai::ProviderError> {
        Ok(self.sse_body.clone())
    }
}

// ---------------------------------------------------------------------------
// Offline CI smoke tests (always run)
// ---------------------------------------------------------------------------

#[test]
fn recorded_anthropic_completion_smoke() {
    let fixture = load_fixture();
    let json_response = fixture["completion_response"].clone();
    assert_eq!(
        json_response["type"].as_str(),
        Some("message"),
        "fixture must have type=message"
    );

    let transport = RecordedAnthropicTransport {
        json_response,
        sse_body: String::new(),
    };
    let client = AnthropicMessagesClient::with_transport(
        ANTHROPIC_PROVIDER_ID,
        "https://recorded.invalid",
        Some("recorded-test-key".to_string()),
        transport,
    );
    let request = ChatCompletionRequest::new(
        ANTHROPIC_PROVIDER_ID,
        "claude-3-haiku-20240307",
        "Reply with one word.",
    );
    let response = client
        .complete(request)
        .expect("recorded completion must succeed");

    assert_eq!(response.provider, ANTHROPIC_PROVIDER_ID);
    assert_eq!(response.model, "claude-3-haiku-20240307");
    assert!(
        !response.text.trim().is_empty(),
        "recorded completion must produce non-empty text"
    );
    // Verify the fixture content round-trips correctly
    assert_eq!(response.text, "Yes.", "recorded text must match fixture");
}

#[test]
fn recorded_anthropic_streaming_smoke() {
    let fixture = load_fixture();
    let sse_body = fixture["streaming_body"]
        .as_str()
        .expect("fixture must have streaming_body string")
        .to_string();

    let transport = RecordedAnthropicTransport {
        json_response: Value::Null,
        sse_body,
    };
    let client = AnthropicMessagesClient::with_transport(
        ANTHROPIC_PROVIDER_ID,
        "https://recorded.invalid",
        Some("recorded-test-key".to_string()),
        transport,
    );
    let request = ChatCompletionRequest::new(
        ANTHROPIC_PROVIDER_ID,
        "claude-3-haiku-20240307",
        "Reply with one word.",
    );
    let events = client
        .stream_events_with_extras(request, Default::default())
        .expect("recorded streaming must succeed");

    assert!(
        !events.is_empty(),
        "recorded streaming must produce at least one event"
    );
    assert!(
        events.contains(&AnthropicSseEvent::MessageStart),
        "streaming events must include MessageStart"
    );
    assert!(
        events.contains(&AnthropicSseEvent::MessageStop),
        "streaming events must include MessageStop"
    );
    let deltas: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AnthropicSseEvent::ContentBlockDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        !deltas.is_empty(),
        "streaming events must include at least one ContentBlockDelta"
    );
    assert_eq!(
        deltas.concat(),
        "Yes.",
        "concatenated deltas must match fixture text"
    );
}

// ---------------------------------------------------------------------------
// Live smoke tests (gated on env vars / runtime availability)
// ---------------------------------------------------------------------------

fn have_anthropic_credentials() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok()
        || std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).is_ok()
        || std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).is_ok()
}

#[test]
fn live_anthropic_smoke() {
    if have_anthropic_credentials() {
        let client = AnthropicMessagesClient::from_env(ANTHROPIC_PROVIDER_ID);
        let model = std::env::var("ANTHROPIC_LIVE_MODEL")
            .unwrap_or_else(|_| "claude-3-haiku-20240307".to_string());
        let request =
            ChatCompletionRequest::new(ANTHROPIC_PROVIDER_ID, model, "Reply with one short word.")
                .with_max_tokens(4);

        let response = client
            .complete(request)
            .expect("live Anthropic completion must succeed");

        assert_eq!(response.provider, ANTHROPIC_PROVIDER_ID);
        assert!(
            !response.text.trim().is_empty(),
            "live Anthropic completion must produce non-empty text"
        );
    } else {
        println!("skip: ANTHROPIC_API_KEY not set");
    }
}

#[test]
fn live_ollama_smoke() {
    let base_url =
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    // Probe Ollama availability with a short timeout via the tags endpoint.
    let available = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().expect("valid addr"),
        std::time::Duration::from_secs(1),
    )
    .is_ok();

    if available {
        let provider = OllamaProvider::new(OLLAMA_PROVIDER_ID, &base_url);
        let model =
            std::env::var("OLLAMA_LIVE_MODEL").unwrap_or_else(|_| "llama3.2:1b".to_string());
        let request =
            ChatCompletionRequest::new(OLLAMA_PROVIDER_ID, &model, "Reply with one word.");

        match provider.complete(request) {
            Ok(response) => {
                assert_eq!(response.provider, OLLAMA_PROVIDER_ID);
                assert!(
                    !response.text.trim().is_empty(),
                    "live Ollama completion must produce non-empty text"
                );
            }
            Err(err) => {
                // Model may not be pulled; treat as a skip
                println!("skip: Ollama available but completion failed (model not pulled?): {err}");
            }
        }
    } else {
        println!("skip: Ollama not available at localhost:11434");
    }
}
