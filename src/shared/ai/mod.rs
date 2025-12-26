// AI module - model definitions and API clients

pub mod client;
pub mod models;

// Re-export commonly used types
pub use client::send_message;
pub use models::{default_models, ModelConfig, Provider};
