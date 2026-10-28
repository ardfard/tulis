//! LLM providers for AI alternates and AI trim.

mod anthropic;
mod mock;
mod openai;
pub mod prompts;

use std::sync::Arc;
use std::time::Duration;

pub use anthropic::{API_VERSION, AnthropicProvider, DEFAULT_BASE_URL};
pub use mock::MockProvider;
pub use openai::OpenAiCompatProvider;

use crate::config::{AiConfig, ProviderKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    #[error("Set ${0} to use AI")]
    MissingKey(String),
    #[error("AI request failed ({0}): {1}")]
    Http(u16, String),
    #[error("AI network error: {0}")]
    Network(String),
    #[error("Could not read AI response: {0}")]
    Parse(String),
    #[error("The model declined this request")]
    Refused,
    #[error("AI config: {0}")]
    Config(String),
}

pub trait LlmProvider: Send + Sync {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError>;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
}

/// Build the configured provider. `env` looks up environment variables (injected for tests).
pub fn provider_from_config(
    config: &AiConfig,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Arc<dyn LlmProvider>, AiError> {
    let key = env(&config.api_key_env)
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| AiError::MissingKey(config.api_key_env.clone()))?;
    match config.provider {
        ProviderKind::Anthropic => {
            let base = if config.base_url.is_empty() {
                DEFAULT_BASE_URL.to_string()
            } else {
                config.base_url.clone()
            };
            Ok(Arc::new(AnthropicProvider::new(
                key,
                config.model.clone(),
                base,
            )))
        }
        ProviderKind::Openai => {
            if config.base_url.is_empty() {
                return Err(AiError::Config(
                    "base_url is required when provider = \"openai\"".into(),
                ));
            }
            Ok(Arc::new(OpenAiCompatProvider::new(
                key,
                config.model.clone(),
                config.base_url.clone(),
            )))
        }
    }
}

/// POST a JSON body and return the response text; non-2xx statuses become `AiError::Http`.
pub(crate) fn post_json(
    url: &str,
    headers: &[(&str, &str)],
    body: &serde_json::Value,
) -> Result<String, AiError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.post(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request
        .send_json(body)
        .map_err(|e| AiError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| AiError::Network(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(AiError::Http(status, text.chars().take(200).collect()));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiConfig;

    #[test]
    fn missing_key_is_reported_with_env_name() {
        let err = provider_from_config(&AiConfig::default(), |_| None)
            .err()
            .unwrap();
        assert_eq!(err, AiError::MissingKey("OPENROUTER_API_KEY".into()));
        assert_eq!(err.to_string(), "Set $OPENROUTER_API_KEY to use AI");
    }

    #[test]
    fn builds_openrouter_by_default() {
        let p = provider_from_config(&AiConfig::default(), |_| Some("k".into())).unwrap();
        assert_eq!((p.name(), p.model()), ("openai", "openrouter/auto"));
    }

    #[test]
    fn builds_anthropic_when_configured() {
        let cfg = AiConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-haiku-5-5".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            base_url: String::new(),
            ..AiConfig::default()
        };
        let p = provider_from_config(&cfg, |_| Some("k".into())).unwrap();
        assert_eq!((p.name(), p.model()), ("anthropic", "claude-haiku-5-5"));
    }

    #[test]
    fn openai_requires_base_url() {
        let mut cfg = AiConfig {
            base_url: String::new(),
            ..AiConfig::default()
        };
        assert!(matches!(
            provider_from_config(&cfg, |_| Some("k".into())),
            Err(AiError::Config(_))
        ));
        cfg.base_url = "http://localhost:11434/v1".into();
        assert_eq!(
            provider_from_config(&cfg, |_| Some("k".into()))
                .unwrap()
                .name(),
            "openai"
        );
    }

    #[test]
    fn mock_returns_queued_responses_and_records_requests() {
        let mock = MockProvider::new(vec![Ok("a".into())]);
        let req = CompletionRequest {
            system: "s".into(),
            user: "u".into(),
            max_tokens: 1,
        };
        assert_eq!(mock.complete(&req), Ok("a".into()));
        assert!(mock.complete(&req).is_err());
        assert_eq!(mock.requests.lock().unwrap().len(), 2);
    }
}
