use std::{fs, path::PathBuf};

use legion_ai::{ChatCompletionRequest, EmbeddingRequest, ModelProvider};
use legion_ai_providers::{
    ANTHROPIC_PROVIDER_ID, AnthropicMessagesClient, DETERMINISTIC_LOCAL_PROVIDER_ID,
    DeterministicLocalProvider,
};
use legion_protocol::PRODUCT_ENV_PREFIX;
use serde_json::Value;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../evals/recorded/provider_smoke_fixture.json")
}

fn load_fixture() -> Value {
    let text = fs::read_to_string(fixture_path()).expect("smoke fixture should be readable");
    serde_json::from_str(&text).expect("smoke fixture should parse")
}

fn have_anthropic_credentials() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok()
        || std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).is_ok()
        || std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).is_ok()
}

#[test]
fn recorded_smoke_local_path_round_trips_from_fixture() {
    let fixture = load_fixture();
    assert_eq!(fixture["suite"], "legion-ai-providers-smoke-v0");

    let local = &fixture["local"];
    let provider_id = local["provider_id"].as_str().expect("local provider id");
    let model = local["model"].as_str().expect("local model");
    let prompt = local["prompt"].as_str().expect("local prompt");
    let embedding_input = local["embedding_input"]
        .as_str()
        .expect("local embedding input");

    let provider = DeterministicLocalProvider::new(provider_id);
    let completion = provider
        .complete(ChatCompletionRequest::new(provider_id, model, prompt))
        .expect("deterministic local completion should succeed");
    let embeddings = provider
        .embed(EmbeddingRequest::new(provider_id, model, embedding_input))
        .expect("deterministic local embedding should succeed");

    assert_eq!(completion.provider, DETERMINISTIC_LOCAL_PROVIDER_ID);
    assert_eq!(completion.model, model);
    assert_eq!(
        completion.metadata.get("redaction"),
        Some(&"metadata-only".to_string())
    );
    assert!(!completion.text.trim().is_empty());
    assert_eq!(embeddings.provider, DETERMINISTIC_LOCAL_PROVIDER_ID);
    assert_eq!(embeddings.model, model);
    assert_eq!(embeddings.vectors.len(), 1);
    assert_eq!(embeddings.vectors[0].len(), 16);
    assert!(embeddings.vectors[0].iter().any(|value| *value != 0.0));
}

#[test]
fn live_smoke_hosted_path_round_trips_when_credentials_are_available() {
    let fixture = load_fixture();
    let hosted = &fixture["hosted"];
    let provider_id = hosted["provider_id"].as_str().expect("hosted provider id");
    let model = hosted["model"].as_str().expect("hosted model");
    let prompt = hosted["prompt"].as_str().expect("hosted prompt");
    assert_eq!(provider_id, ANTHROPIC_PROVIDER_ID);

    if !have_anthropic_credentials() {
        eprintln!(
            "skipping Anthropic live smoke: no API key or auth token in the test environment"
        );
        return;
    }

    let provider = AnthropicMessagesClient::from_env(ANTHROPIC_PROVIDER_ID);
    let request = ChatCompletionRequest::new(provider_id, model, prompt).with_max_tokens(1);

    let tokens = provider
        .count_tokens(request.clone())
        .expect("live Anthropic token count");
    let response = provider
        .complete(request)
        .expect("live Anthropic completion");

    assert!(tokens > 0, "live token count must be non-zero");
    assert_eq!(response.provider, ANTHROPIC_PROVIDER_ID);
    assert_eq!(response.model, model);
    assert!(
        !response.text.trim().is_empty(),
        "live completion must produce text"
    );
}
