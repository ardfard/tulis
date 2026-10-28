//! Anthropic Messages API (raw HTTP; there is no official Rust SDK).

use serde_json::{Value, json};

use super::{AiError, CompletionRequest, LlmProvider, post_json};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2026-10-21";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
        }
    }

    pub fn url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    pub fn request_body(&self, req: &CompletionRequest) -> Value {
        json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": [{ "role": "user", "content": req.user }],
        })
    }

    pub fn parse_response(body: &str) -> Result<String, AiError> {
        let value: Value = serde_json::from_str(body).map_err(|e| AiError::Parse(e.to_string()))?;
        if value["stop_reason"] == "refusal" {
            return Err(AiError::Refused);
        }
        value["content"]
            .as_array()
            .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
            .and_then(|block| block["text"].as_str())
            .map(str::to_string)
            .ok_or_else(|| AiError::Parse("response has no text block".into()))
    }
}

impl LlmProvider for AnthropicProvider {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError> {
        let body = self.request_body(req);
        let text = post_json(
            &self.url(),
            &[
                ("x-api-key", self.api_key.as_str()),
                ("anthropic-version", API_VERSION),
                ("content-type", "application/json"),
            ],
            &body,
        )?;
        Self::parse_response(&text)
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AnthropicProvider {
        AnthropicProvider::new(
            "k".into(),
            "claude-haiku-5-5".into(),
            "https://api.anthropic.com/".into(),
        )
    }

    #[test]
    fn request_body_shape() {
        let req = CompletionRequest {
            system: "sys".into(),
            user: "hi".into(),
            max_tokens: 4096,
        };
        let body = provider().request_body(&req);
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["system"], "sys");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert!(body.get("temperature").is_none());
        assert_eq!(provider().url(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn parse_skips_thinking_blocks() {
        let body = r#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"hello"}],"stop_reason":"end_turn"}"#;
        assert_eq!(
            AnthropicProvider::parse_response(body),
            Ok("hello".to_string())
        );
    }

    #[test]
    fn parse_refusal_and_garbage() {
        let refusal = r#"{"content":[],"stop_reason":"refusal"}"#;
        assert_eq!(
            AnthropicProvider::parse_response(refusal),
            Err(AiError::Refused)
        );
        assert!(matches!(
            AnthropicProvider::parse_response("nope"),
            Err(AiError::Parse(_))
        ));
        assert!(matches!(
            AnthropicProvider::parse_response(r#"{"content":[]}"#),
            Err(AiError::Parse(_))
        ));
    }
}
