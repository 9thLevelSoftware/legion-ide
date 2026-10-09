//! Explicit product profiles. Configuration contains metadata only; credentials
//! stay in the existing SecretStore port and are bound to endpoint identity.

use super::*;
use legion_storage::{SecretReference, SecretStore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// A named route through an existing provider adapter, never a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderProfile {
    /// Stable, user-chosen profile name.
    pub name: String,
    /// Existing adapter: ollama, llama-cpp, anthropic, openai or openai-compatible.
    pub provider_id: String,
    /// Exact base endpoint; HTTPS required except on loopback.
    pub endpoint: String,
    /// Exact model identifier sent on the wire.
    pub model: String,
    /// OpenAI chat limit includes reasoning tokens; false uses legacy max_tokens.
    #[serde(default)]
    pub max_completion_tokens: bool,
    /// Explicitly request thinking.type=disabled on a supporting chat endpoint.
    #[serde(default)]
    pub disable_thinking: bool,
}

/// App-produced metadata for provider settings; never exposes a key or payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderProfileProjection {
    /// Validated configuration.
    pub profile: AiProviderProfile,
    /// Whether this is the explicitly selected route.
    pub selected: bool,
    /// Transport locality, not a claim about a proxy's downstream behavior.
    pub locality: String,
    /// Adapter capabilities; these are not model qualification.
    pub capabilities: Vec<String>,
    /// Stored, missing, or unavailable keyring state.
    pub credential_state: String,
    /// Last explicit check state; configuring a profile makes no network call.
    pub health: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AiProviderConfiguration {
    pub(crate) profiles: Vec<AiProviderProfile>,
    pub(crate) selected: Option<String>,
}

impl AiProviderConfiguration {
    fn parse(json: &str) -> Result<Self, AppCompositionError> {
        if json.len() > 32768 {
            return Err(config_error("provider configuration exceeds limit"));
        }
        let config: Self = serde_json::from_str(json)
            .map_err(|_| config_error("invalid provider configuration"))?;
        if config.profiles.len() > 16 {
            return Err(config_error("provider profile limit reached"));
        }
        let mut names = std::collections::HashSet::new();
        for profile in &config.profiles {
            profile.validate()?;
            if !names.insert(&profile.name) {
                return Err(config_error("duplicate provider profile"));
            }
        }
        if config
            .selected
            .as_ref()
            .is_some_and(|name| !names.contains(name))
        {
            return Err(config_error("selected provider profile is missing"));
        }
        Ok(config)
    }
}

/// Immutable route snapshot retained only for the authorized in-flight request.
pub(crate) struct ConfiguredProvider {
    pub(crate) profile: AiProviderProfile,
    pub(crate) credential: Option<Zeroizing<String>>,
    pub(crate) health: Arc<Mutex<HashMap<String, String>>>,
    pub(crate) live_snapshots: Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for ConfiguredProvider {
    fn drop(&mut self) {
        self.live_snapshots
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl std::fmt::Debug for ConfiguredProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfiguredProvider")
            .field("profile", &self.profile)
            .field("credential", &"[redacted]")
            .finish()
    }
}

impl ConfiguredProvider {
    pub(crate) fn set_health(&self, health: &str) {
        if let Ok(mut states) = self.health.lock() {
            states.insert(self.profile.name.clone(), health.into());
        }
    }
}

impl AiProviderProfile {
    fn validate(&self) -> Result<(), AppCompositionError> {
        let identifier = |s: &str, max: usize| {
            !s.is_empty()
                && s.len() <= max
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
        };
        if !identifier(&self.name, 64)
            || self.name.contains(['/', ':'])
            || !identifier(&self.model, 128)
            || !matches!(
                self.provider_id.as_str(),
                "ollama" | "llama-cpp" | "anthropic" | "openai" | "openai-compatible"
            )
        {
            return Err(config_error("invalid profile name, adapter or model"));
        }
        crate::ai_route_descriptor::validate_profile_endpoint(&self.endpoint)?;
        if (self.max_completion_tokens || self.disable_thinking)
            && self.provider_id != "openai-compatible"
        {
            return Err(config_error(
                "chat wire options require the OpenAI-compatible adapter",
            ));
        }
        for value in [&self.name, &self.model, &self.endpoint] {
            if !legion_security::secrets::scan_text_for_secrets(value)
                .findings
                .is_empty()
            {
                return Err(config_error(
                    "credential-like material is forbidden in provider configuration",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn target(&self) -> legion_protocol::NetworkTarget {
        crate::ai_route_descriptor::profile_network_target(&self.endpoint)
    }

    pub(crate) fn local(&self) -> bool {
        crate::ai_route_descriptor::is_loopback_host(&self.target().host)
    }

    pub(crate) fn requires_credential(&self) -> bool {
        matches!(
            self.provider_id.as_str(),
            "anthropic" | "openai" | "openai-compatible"
        ) || !self.local()
    }

    fn reference(&self) -> SecretReference {
        use sha2::{Digest, Sha256};
        // Endpoint/model edits do not inherit a credential for a different route.
        let digest = Sha256::digest(format!(
            "{}\n{}\n{}",
            self.provider_id, self.endpoint, self.model
        ));
        legion_storage::provider_api_key_reference(&format!(
            "profile:{}:{}",
            self.name,
            hex::encode(digest)
        ))
    }
}

fn config_error(message: &str) -> AppCompositionError {
    AppCompositionError::AiRuntime(message.to_string())
}

impl AppComposition {
    pub(crate) fn invalidate_provider_predictions(&mut self) {
        self.ai_provider_revision = uuid::Uuid::now_v7();
        self.assist_inline_prediction_state = Default::default();
    }

    /// Inject the existing secret-store port; production uses the OS keyring.
    pub fn with_provider_secret_store(store: Arc<dyn SecretStore + Send + Sync>) -> Self {
        let mut app = Self::new();
        app.provider_secret_store = store;
        app
    }

    fn require_idle_provider_configuration(&self) -> Result<(), AppCompositionError> {
        if self.provider_configuration_busy() {
            return Err(config_error(
                "provider configuration is busy; stop or finish the current operation first",
            ));
        }
        Ok(())
    }

    /// Includes cancelled workers whose transport has not actually returned.
    pub fn provider_configuration_busy(&self) -> bool {
        self.product_ai_stream_in_flight()
            || self
                .named_provider_snapshots
                .load(std::sync::atomic::Ordering::SeqCst)
                != 0
    }

    fn profile(&self, name: &str) -> Result<&AiProviderProfile, AppCompositionError> {
        self.ai_provider_configuration
            .profiles
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| config_error("unknown provider profile"))
    }

    /// Add or replace a validated metadata-only profile without probing a host.
    pub fn configure_ai_provider_profile(
        &mut self,
        profile: AiProviderProfile,
    ) -> Result<(), AppCompositionError> {
        self.require_idle_provider_configuration()?;
        profile.validate()?;
        if let Some(index) = self
            .ai_provider_configuration
            .profiles
            .iter()
            .position(|p| p.name == profile.name)
        {
            if self.ai_provider_configuration.profiles[index] != profile {
                if self.ai_provider_configuration.selected.as_deref() == Some(&profile.name) {
                    self.invalidate_provider_predictions();
                }
                self.ai_profile_health
                    .lock()
                    .expect("provider health lock")
                    .remove(&profile.name);
                self.ai_provider_configuration.profiles[index] = profile;
            }
        } else {
            if self.ai_provider_configuration.profiles.len() >= 16 {
                return Err(config_error("provider profile limit reached"));
            }
            self.ai_provider_configuration.profiles.push(profile);
        }
        Ok(())
    }

    /// Explicitly select a named route; no connection or credential is implied.
    pub fn select_ai_provider_profile(&mut self, name: &str) -> Result<(), AppCompositionError> {
        self.require_idle_provider_configuration()?;
        self.profile(name)?;
        self.ai_provider_configuration.selected = Some(name.to_string());
        self.invalidate_provider_predictions();
        Ok(())
    }

    /// Replace a route-bound key in secure storage; config and errors omit it.
    pub fn replace_ai_profile_credential(
        &mut self,
        name: &str,
        secret: &str,
    ) -> Result<(), AppCompositionError> {
        self.require_idle_provider_configuration()?;
        if secret.trim().is_empty() || secret.len() > 8192 || secret.chars().any(char::is_control) {
            return Err(config_error("invalid provider credential"));
        }
        let reference = self.profile(name)?.reference();
        self.provider_secret_store
            .store(&reference, secret)
            .map_err(|_| config_error("secure credential replacement failed"))?;
        self.ai_profile_health
            .lock()
            .expect("provider health lock")
            .remove(name);
        self.invalidate_provider_predictions();
        Ok(())
    }

    /// Revoke this profile's key, with no environment or provider-wide fallback.
    pub fn revoke_ai_profile_credential(&mut self, name: &str) -> Result<(), AppCompositionError> {
        self.require_idle_provider_configuration()?;
        let reference = self.profile(name)?.reference();
        if self
            .provider_secret_store
            .load(&reference)
            .map_err(|_| config_error("secure credential revocation failed"))?
            .map(Zeroizing::new)
            .is_some()
        {
            self.provider_secret_store
                .delete(&reference)
                .map_err(|_| config_error("secure credential revocation failed"))?;
        }
        self.ai_profile_health
            .lock()
            .expect("provider health lock")
            .remove(name);
        self.invalidate_provider_predictions();
        Ok(())
    }

    /// Bounded config encoding without secret fields or health/consent claims.
    pub fn ai_provider_configuration_json(&self) -> Result<String, AppCompositionError> {
        serde_json::to_string(&self.ai_provider_configuration)
            .map_err(|_| config_error("provider configuration encoding failed"))
    }

    /// Validate metadata for the existing settings store without reading keys,
    /// changing application state, or contacting a provider.
    pub fn validate_ai_provider_configuration_json(json: &str) -> Result<(), AppCompositionError> {
        AiProviderConfiguration::parse(json).map(|_| ())
    }

    /// Restore metadata atomically; malformed or secret-bearing records fail closed.
    pub fn restore_ai_provider_configuration_json(
        &mut self,
        json: &str,
    ) -> Result<(), AppCompositionError> {
        self.require_idle_provider_configuration()?;
        let config = AiProviderConfiguration::parse(json)?;
        self.ai_provider_configuration = config;
        self.ai_profile_health
            .lock()
            .expect("provider health lock")
            .clear();
        self.invalidate_provider_predictions();
        Ok(())
    }

    /// Project configuration/credential metadata without health checks or egress.
    pub fn ai_provider_profiles(&self) -> Vec<AiProviderProfileProjection> {
        self.ai_provider_configuration
            .profiles
            .iter()
            .map(|profile| {
                let credential_state = match self.provider_secret_store.load(&profile.reference()) {
                    Ok(Some(secret)) => {
                        if Zeroizing::new(secret).trim().is_empty() {
                            "missing"
                        } else {
                            "stored"
                        }
                    }
                    Ok(_) => "missing",
                    Err(_) => "unavailable",
                };
                AiProviderProfileProjection {
                    profile: profile.clone(),
                    selected: self.ai_provider_configuration.selected.as_deref()
                        == Some(&profile.name),
                    locality: if profile.local() {
                        "loopback"
                    } else {
                        "remote"
                    }
                    .into(),
                    capabilities: vec![
                        "chat completion".into(),
                        "inline via chat (unqualified)".into(),
                    ],
                    credential_state: credential_state.into(),
                    health: self
                        .ai_profile_health
                        .lock()
                        .expect("provider health lock")
                        .get(&profile.name)
                        .cloned()
                        .unwrap_or_else(|| "not checked".into()),
                }
            })
            .collect()
    }

    pub(crate) fn selected_product_ai_selection(
        &self,
    ) -> Result<
        (
            Option<ProductAiLiveBackend>,
            Option<local_ai_diagnosis::AnthropicKeyState>,
        ),
        AppCompositionError,
    > {
        if let Some(name) = &self.ai_provider_configuration.selected {
            #[cfg(not(feature = "ai"))]
            {
                let _ = name;
                return Err(config_error(
                    "selected AI provider unavailable in this offline build",
                ));
            }
            #[cfg(feature = "ai")]
            {
                let profile = self.profile(name)?.clone();
                let credential = self
                    .provider_secret_store
                    .load(&profile.reference())
                    .map_err(|_| {
                        config_error(
                            "selected AI provider unavailable: secure credential store unavailable",
                        )
                    })?
                    .map(Zeroizing::new)
                    .filter(|s| !s.trim().is_empty());
                if profile.requires_credential() && credential.is_none() {
                    return Err(config_error(
                        "selected AI provider unavailable: profile credential missing",
                    ));
                }
                self.named_provider_snapshots
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Ok((
                    Some(ProductAiLiveBackend::Configured(Arc::new(
                        ConfiguredProvider {
                            profile,
                            credential,
                            health: self.ai_profile_health.clone(),
                            live_snapshots: self.named_provider_snapshots.clone(),
                        },
                    ))),
                    None,
                ));
            }
        }
        // Auto is legacy configuration, not authorization to change routes.
        if self.preferred_ai_provider == ProductAiProviderPreference::Auto {
            return Err(config_error(
                "AI provider unavailable: explicitly select a named profile",
            ));
        }
        let selection = product_ai_selection(self.preferred_ai_provider);
        if selection.0.is_none()
            && self.preferred_ai_provider != ProductAiProviderPreference::Deterministic
        {
            return Err(config_error(
                "selected AI provider unavailable; configure its endpoint/model/credentials",
            ));
        }
        Ok(selection)
    }
}
