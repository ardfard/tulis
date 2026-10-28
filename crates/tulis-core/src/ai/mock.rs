//! A scripted provider for tests.

use std::sync::Mutex;

use super::{AiError, CompletionRequest, LlmProvider};

pub struct MockProvider {
    responses: Mutex<Vec<Result<String, AiError>>>,
    pub requests: Mutex<Vec<CompletionRequest>>,
}

impl MockProvider {
    pub fn new(responses: Vec<Result<String, AiError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl LlmProvider for MockProvider {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError> {
        self.requests.lock().expect("mock lock").push(req.clone());
        let mut responses = self.responses.lock().expect("mock lock");
        if responses.is_empty() {
            Err(AiError::Network("no mock response queued".into()))
        } else {
            responses.remove(0)
        }
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}
