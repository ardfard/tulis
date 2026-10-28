//! OpenAI-compatible `/chat/completions` endpoints (OpenAI, OpenRouter, Ollama, …).

use serde_json::{Value, json};

use super::{AiError, CompletionRequest, LlmProvider, post_json};

pub struct OpenAiCompatProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiCompatProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
        }
    }

    pub fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    pub fn request_body(&self, req: &CompletionRequest) -> Value {
        json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": req.user },
            ],
        })
    }

    pub fn parse_response(body: &str) -> Result<String, AiError> {
        let value: Value = serde_json::from_str(body).map_err(|e| AiError::Parse(e.to_string()))?;
        value["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| AiError::Parse("response has no message content".into()))
    }
}

impl LlmProvider for OpenAiCompatProvider {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError> {
        let body = self.request_body(req);
        let auth = format!("Bearer {}", self.api_key);
        let text = post_json(
            &self.url(),
            &[
                ("authorization", auth.as_str()),
                ("content-type", "application/json"),
            ],
            &body,
        )?;
        Self::parse_response(&text)
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_response_shape() {
        let p = OpenAiCompatProvider::new(
            "k".into(),
            "llama3".into(),
            "http://localhost:11434/v1/".into(),
        );
        assert_eq!(p.url(), "http://localhost:11434/v1/chat/completions");
        let req = CompletionRequest {
            system: "sys".into(),
            user: "hi".into(),
            max_tokens: 100,
        };
        let body = p.request_body(&req);
        assert_eq!(body["model"], "llama3");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "hi");
        let resp = r#"{"choices":[{"message":{"role":"assistant","content":"yo"}}]}"#;
        assert_eq!(
            OpenAiCompatProvider::parse_response(resp),
            Ok("yo".to_string())
        );
        assert!(matches!(
            OpenAiCompatProvider::parse_response("{}"),
            Err(AiError::Parse(_))
        ));
    }
}
