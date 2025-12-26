use serde::{Deserialize, Serialize};
use reqwest::Client;

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

pub async fn send_message(config: &ModelConfig, _history: &[String], input: &str) -> Result<String, String> {
    match config.provider {
        Provider::Echo => Ok(format!("Echo: {}", input)),
        Provider::Gemini => {
            if let Some(key) = &config.api_key {
                send_gemini_message(input, key, &config.model_id).await
            } else {
                Err("Gemini API Key missing. Please set it in Settings (Ctrl+S).".to_string())
            }
        },
        Provider::OpenAI => {
            if let Some(key) = &config.api_key {
                send_openai_message(input, key, &config.model_id, config.base_url.as_deref()).await
            } else {
                Err("OpenAI API Key missing. Please set it in Settings (Ctrl+S).".to_string())
            }
        },
        Provider::Anthropic => {
            if let Some(key) = &config.api_key {
                send_anthropic_message(input, key, &config.model_id).await
            } else {
                Err("Anthropic API Key missing. Please set it in Settings (Ctrl+S).".to_string())
            }
        },
        Provider::Ollama => {
            send_ollama_message(input, &config.model_id, config.base_url.as_deref()).await
        },
    }
}

// ============ Gemini ============

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    role: String,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
}

#[derive(Deserialize)]
struct GeminiContentResponse {
    parts: Vec<GeminiPartResponse>,
}

#[derive(Deserialize)]
struct GeminiPartResponse {
    text: String,
}

#[derive(Deserialize)]
struct GeminiError {
    message: String,
}

async fn send_gemini_message(input: &str, api_key: &str, model_id: &str) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, api_key
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_body = GeminiRequest {
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart { text: input.to_string() }],
        }],
    };

    let response = client.post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_text));
    }

    let response_text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    let gemini_resp: GeminiResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response: {} - Body: {}", e, &response_text[..response_text.len().min(200)]))?;

    if let Some(error) = gemini_resp.error {
        return Err(format!("Gemini API error: {}", error.message));
    }

    if let Some(candidates) = gemini_resp.candidates {
        if let Some(candidate) = candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                return Ok(part.text.clone());
            }
        }
    }

    Err("No response content found in Gemini response".to_string())
}

// ============ OpenAI ============

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Option<Vec<OpenAIChoice>>,
    error: Option<OpenAIError>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessageResponse,
}

#[derive(Deserialize)]
struct OpenAIMessageResponse {
    content: String,
}

#[derive(Deserialize)]
struct OpenAIError {
    message: String,
}

async fn send_openai_message(input: &str, api_key: &str, model_id: &str, base_url: Option<&str>) -> Result<String, String> {
    let base = base_url.unwrap_or("https://api.openai.com/v1");
    let url = format!("{}/chat/completions", base);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_body = OpenAIRequest {
        model: model_id.to_string(),
        messages: vec![OpenAIMessage {
            role: "user".to_string(),
            content: input.to_string(),
        }],
    };

    let response = client.post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_text));
    }

    let response_text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    let openai_resp: OpenAIResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response: {} - Body: {}", e, &response_text[..response_text.len().min(200)]))?;

    if let Some(error) = openai_resp.error {
        return Err(format!("OpenAI API error: {}", error.message));
    }

    if let Some(choices) = openai_resp.choices {
        if let Some(choice) = choices.first() {
            return Ok(choice.message.content.clone());
        }
    }

    Err("No response content found in OpenAI response".to_string())
}

// ============ Anthropic ============

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Option<Vec<AnthropicContent>>,
    error: Option<AnthropicError>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize)]
struct AnthropicError {
    message: String,
}

async fn send_anthropic_message(input: &str, api_key: &str, model_id: &str) -> Result<String, String> {
    let url = "https://api.anthropic.com/v1/messages";

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_body = AnthropicRequest {
        model: model_id.to_string(),
        max_tokens: 4096,
        messages: vec![AnthropicMessage {
            role: "user".to_string(),
            content: input.to_string(),
        }],
    };

    let response = client.post(url)
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_text));
    }

    let response_text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    let anthropic_resp: AnthropicResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response: {} - Body: {}", e, &response_text[..response_text.len().min(200)]))?;

    if let Some(error) = anthropic_resp.error {
        return Err(format!("Anthropic API error: {}", error.message));
    }

    if let Some(content) = anthropic_resp.content {
        if let Some(block) = content.first() {
            return Ok(block.text.clone());
        }
    }

    Err("No response content found in Anthropic response".to_string())
}

// ============ Ollama ============

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: Option<String>,
    error: Option<String>,
}

async fn send_ollama_message(input: &str, model_id: &str, base_url: Option<&str>) -> Result<String, String> {
    let base = base_url.unwrap_or("http://localhost:11434");
    let url = format!("{}/api/generate", base);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_body = OllamaRequest {
        model: model_id.to_string(),
        prompt: input.to_string(),
        stream: false,
    };

    let response = client.post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Network error (is Ollama running?): {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_text));
    }

    let response_text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    let ollama_resp: OllamaResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response: {} - Body: {}", e, &response_text[..response_text.len().min(200)]))?;

    if let Some(error) = ollama_resp.error {
        return Err(format!("Ollama error: {}", error));
    }

    if let Some(response) = ollama_resp.response {
        return Ok(response);
    }

    Err("No response content found in Ollama response".to_string())
}
