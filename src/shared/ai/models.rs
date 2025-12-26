// AI model definitions and configuration

use serde::{Deserialize, Serialize};

/// Supported AI providers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Provider {
    Gemini,
    OpenAI,
    Anthropic,
    Ollama,
    Echo,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::Gemini => write!(f, "Gemini"),
            Provider::OpenAI => write!(f, "OpenAI"),
            Provider::Anthropic => write!(f, "Anthropic"),
            Provider::Ollama => write!(f, "Ollama"),
            Provider::Echo => write!(f, "Echo"),
        }
    }
}

/// Configuration for a specific AI model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub provider: Provider,
    pub model_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl ModelConfig {
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.name, self.provider)
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: "Gemini Flash".to_string(),
            provider: Provider::Gemini,
            model_id: "gemini-2.0-flash".to_string(),
            api_key: None,
            base_url: None,
        }
    }
}

/// Get default model configurations
pub fn default_models() -> Vec<ModelConfig> {
    vec![
        ModelConfig {
            name: "Gemini Flash".to_string(),
            provider: Provider::Gemini,
            model_id: "gemini-2.0-flash".to_string(),
            api_key: None,
            base_url: None,
        },
        ModelConfig {
            name: "GPT-4o Mini".to_string(),
            provider: Provider::OpenAI,
            model_id: "gpt-4o-mini".to_string(),
            api_key: None,
            base_url: None,
        },
        ModelConfig {
            name: "Claude Sonnet".to_string(),
            provider: Provider::Anthropic,
            model_id: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
        },
        ModelConfig {
            name: "Ollama Llama".to_string(),
            provider: Provider::Ollama,
            model_id: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
        },
        ModelConfig {
            name: "Echo (Offline)".to_string(),
            provider: Provider::Echo,
            model_id: "echo".to_string(),
            api_key: None,
            base_url: None,
        },
    ]
}
