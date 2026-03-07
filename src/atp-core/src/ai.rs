//! AI/LLM integration — natural language to AQL, semantic search, and AI-assisted transforms.
//!
//! Supports both local (Ollama) and remote (OpenAI-compatible) LLM backends
//! for translating natural language queries into AQL, explaining code, and
//! performing AI-assisted text transformations.
//!
//! # Environment Variables
//!
//! - `ATP_AI_BACKEND`: `ollama` (default) or `openai`
//! - `ATP_AI_MODEL`: Model name (default: `llama3.2` for Ollama, `gpt-4o` for OpenAI)
//! - `ATP_AI_URL`: Base URL for the API (default: `http://localhost:11434` for Ollama)
//! - `OPENAI_API_KEY`: API key for OpenAI backend

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// AI backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Backend type: "ollama" or "openai".
    pub backend: AiBackend,
    /// Model name.
    pub model: String,
    /// Base URL for the API.
    pub base_url: String,
    /// API key (required for OpenAI).
    pub api_key: Option<String>,
    /// Temperature for generation (0.0 = deterministic, 1.0 = creative).
    pub temperature: f64,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
}

/// Supported AI backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiBackend {
    Ollama,
    OpenAi,
}

impl Default for AiConfig {
    fn default() -> Self {
        let backend_str = std::env::var("ATP_AI_BACKEND").unwrap_or_else(|_| "ollama".to_string());
        let backend = match backend_str.to_lowercase().as_str() {
            "openai" => AiBackend::OpenAi,
            _ => AiBackend::Ollama,
        };

        let (default_model, default_url) = match &backend {
            AiBackend::Ollama => ("llama3.2".to_string(), "http://localhost:11434".to_string()),
            AiBackend::OpenAi => (
                "gpt-4o".to_string(),
                "https://api.openai.com/v1".to_string(),
            ),
        };

        Self {
            backend,
            model: std::env::var("ATP_AI_MODEL").unwrap_or(default_model),
            base_url: std::env::var("ATP_AI_URL").unwrap_or(default_url),
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            temperature: 0.1,
            max_tokens: 1024,
        }
    }
}

/// Chat message for LLM APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// AI engine — wraps LLM interaction for ATP use cases.
pub struct AiEngine {
    config: AiConfig,
    client: reqwest::Client,
}

impl Default for AiEngine {
    fn default() -> Self {
        Self::with_config(AiConfig::default())
    }
}

impl AiEngine {
    /// Create a new AI engine with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new AI engine with specific configuration.
    pub fn with_config(config: AiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Translate a natural language query into AQL.
    ///
    /// Example: "find all TODO comments case insensitive" → `find "TODO" ignore_case`
    pub async fn nl_to_aql(&self, natural_language: &str) -> Result<String> {
        let system = r#"You are an AQL (ATP Query Language) expert. Convert natural language queries into valid AQL syntax.

AQL syntax reference:
- find "pattern" [ignore_case] [invert] [max N] — search for text
- replace "old" with "new" [all] [ignore_case] — substitute text
- filter field N <op> <value> — filter by field value (ops: contains, ==, !=, >, <, >=, <=, matches)
- extract field N — extract a specific field
- sort [asc|desc] [by field N] — sort lines
- unique — deduplicate lines
- count — count lines
- head N — first N lines
- tail N — last N lines
- format template "..." — format output
- let x = <expr> — bind a variable
- if <cond> then <pipeline> [else <pipeline>] end — conditional execution
- group by field N aggregate count[, sum N][, avg N][, min N][, max N] — group and aggregate
- def name <pipeline> end — define a reusable function
- call name — invoke a defined function

Stages are chained with | (pipe).

Reply with ONLY the AQL query, no explanation."#;

        let response = self
            .chat(&[
                ChatMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: natural_language.to_string(),
                },
            ])
            .await?;

        Ok(response.trim().to_string())
    }

    /// Explain what an AQL query does in natural language.
    pub async fn explain_aql(&self, query: &str) -> Result<String> {
        let system = "You are an AQL expert. Explain what the given AQL query does in plain English. Be concise (1-3 sentences).";

        self.chat(&[
            ChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("Explain this AQL query: {query}"),
            },
        ])
        .await
    }

    /// Suggest an AQL query based on sample data and a goal description.
    pub async fn suggest_query(&self, sample_data: &str, goal: &str) -> Result<String> {
        let system = r#"You are an AQL expert. Given sample data and a user's goal, suggest the best AQL query. Reply with ONLY the AQL query."#;

        self.chat(&[
            ChatMessage {
                role: "system".to_string(),
                content: system.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Sample data:\n```\n{sample_data}\n```\n\nGoal: {goal}\n\nSuggest an AQL query:"
                ),
            },
        ])
        .await
        .map(|s| s.trim().to_string())
    }

    /// Perform a semantic search — describe what to find in natural language.
    pub async fn semantic_search(&self, description: &str, content: &str) -> Result<Vec<String>> {
        let system = "You are a code/text analyst. Given content and a description of what to find, return matching lines (one per line). Return ONLY the matching lines, nothing else. If no matches, return an empty response.";

        let response = self
            .chat(&[
                ChatMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("Find: {description}\n\nContent:\n```\n{content}\n```"),
                },
            ])
            .await?;

        Ok(response
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Send a chat completion request to the configured backend.
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        match &self.config.backend {
            AiBackend::Ollama => self.chat_ollama(messages).await,
            AiBackend::OpenAi => self.chat_openai(messages).await,
        }
    }

    async fn chat_ollama(&self, messages: &[ChatMessage]) -> Result<String> {
        #[derive(Serialize)]
        struct OllamaRequest<'a> {
            model: &'a str,
            messages: &'a [ChatMessage],
            stream: bool,
            options: OllamaOptions,
        }

        #[derive(Serialize)]
        struct OllamaOptions {
            temperature: f64,
            num_predict: u32,
        }

        #[derive(Deserialize)]
        struct OllamaResponse {
            message: OllamaMessage,
        }

        #[derive(Deserialize)]
        struct OllamaMessage {
            content: String,
        }

        let url = format!("{}/api/chat", self.config.base_url);
        let body = OllamaRequest {
            model: &self.config.model,
            messages,
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens,
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to connect to Ollama at {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error {status}: {body}");
        }

        let parsed: OllamaResponse = resp
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(parsed.message.content)
    }

    async fn chat_openai(&self, messages: &[ChatMessage]) -> Result<String> {
        #[derive(Serialize)]
        struct OpenAiRequest<'a> {
            model: &'a str,
            messages: &'a [ChatMessage],
            temperature: f64,
            max_tokens: u32,
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<OpenAiChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessage,
        }

        #[derive(Deserialize)]
        struct OpenAiMessage {
            content: String,
        }

        let api_key = self
            .config
            .api_key
            .as_deref()
            .context("OPENAI_API_KEY not set")?;

        let url = format!("{}/chat/completions", self.config.base_url);
        let body = OpenAiRequest {
            model: &self.config.model,
            messages,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to connect to OpenAI at {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {status}: {body}");
        }

        let parsed: OpenAiResponse = resp
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("No choices returned from OpenAI")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_config_default() {
        let config = AiConfig::default();
        // Without env vars, defaults to Ollama
        assert!(config.temperature >= 0.0 && config.temperature <= 1.0);
        assert!(config.max_tokens > 0);
    }

    #[test]
    fn test_ai_backend_serde() {
        let json = serde_json::to_string(&AiBackend::Ollama).unwrap();
        assert_eq!(json, "\"ollama\"");
        let json = serde_json::to_string(&AiBackend::OpenAi).unwrap();
        assert_eq!(json, "\"openai\"");
    }

    #[test]
    fn test_chat_message_serialize() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "hello");
    }

    #[test]
    fn test_ai_engine_creation() {
        let engine = AiEngine::new();
        assert!(matches!(
            engine.config.backend,
            AiBackend::Ollama | AiBackend::OpenAi
        ));
    }

    #[test]
    fn test_ai_config_custom() {
        let config = AiConfig {
            backend: AiBackend::OpenAi,
            model: "gpt-4-turbo".into(),
            base_url: "https://custom.api.com/v1".into(),
            api_key: Some("test-key".into()),
            temperature: 0.5,
            max_tokens: 2048,
        };
        assert_eq!(config.model, "gpt-4-turbo");
        assert_eq!(config.api_key.as_deref(), Some("test-key"));
    }
}
