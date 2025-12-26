use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::ai::{ModelConfig, default_models};
use crate::theme::ThemeMode;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default = "default_models")]
    pub models: Vec<ModelConfig>,
    #[serde(default)]
    pub selected_model_idx: usize,
    // Legacy field for backward compatibility
    #[serde(skip_serializing, default)]
    pub gemini_api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            models: default_models(),
            selected_model_idx: 0,
            gemini_api_key: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(&config_path) {
            let mut config: Config = serde_json::from_str(&content).unwrap_or_default();

            // Migrate legacy gemini_api_key to new model system
            if let Some(key) = config.gemini_api_key.take() {
                if let Some(gemini_model) = config.models.iter_mut()
                    .find(|m| m.provider == crate::ai::Provider::Gemini) {
                    if gemini_model.api_key.is_none() {
                        gemini_model.api_key = Some(key);
                    }
                }
            }

            // Ensure we have at least the default models
            if config.models.is_empty() {
                config.models = default_models();
            }

            config
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let config_path = Self::get_config_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)
    }

    fn get_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".nterm_config.json")
    }

    pub fn get_selected_model(&self) -> &ModelConfig {
        self.models.get(self.selected_model_idx).unwrap_or(&self.models[0])
    }

    pub fn get_selected_model_mut(&mut self) -> &mut ModelConfig {
        let idx = self.selected_model_idx.min(self.models.len().saturating_sub(1));
        &mut self.models[idx]
    }

    pub fn cycle_model(&mut self) {
        if !self.models.is_empty() {
            self.selected_model_idx = (self.selected_model_idx + 1) % self.models.len();
        }
    }
}
