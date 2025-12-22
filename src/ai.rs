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
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}", api_key);
    
    let client = Client::new();
    
    let request_body = GeminiRequest {
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart { text: input.to_string() }],
        }],
    };

    let response = client.post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
        
    if !response.status().is_success() {
         return Err(format!("API Request failed: {}", response.status()));
    }

    let gemini_resp: GeminiResponse = response.json().await.map_err(|e| e.to_string())?;
    
    if let Some(error) = gemini_resp.error {
        return Err(error.message);
    }

    if let Some(candidates) = gemini_resp.candidates {
        if let Some(candidate) = candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                return Ok(part.text.clone());
            }
        }
    }
    
    Err("No response content found".to_string())
}
