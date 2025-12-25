use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Model {
    Gemini,
    Echo,
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::Gemini => write!(f, "Gemini Flash"),
            Model::Echo => write!(f, "Echo (Offline)"),
        }
    }
}

pub async fn send_message(model: Model, _history: &[String], input: &str, api_key: Option<String>) -> Result<String, String> {
    match model {
        Model::Echo => Ok(format!("Echo: {}", input)),
        Model::Gemini => {
            if let Some(key) = api_key {
                send_gemini_message(input, &key).await
            } else {
                Err("API Key missing. Please set it in Settings (Ctrl+S).".to_string())
            }
        },
    }
}

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

async fn send_gemini_message(input: &str, api_key: &str) -> Result<String, String> {
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}", api_key);

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
