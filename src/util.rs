use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum Provider {
    OpenAI,
    Anthropic
}

#[derive(Clone, Deserialize, Serialize)]
pub struct APIKey {
    pub name: String,
    pub key: String,
    pub provider: Provider
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub temperature: f64,
    pub max_tokens: u32,
    pub model: String,
    pub api_key: Option<usize>,
    pub api_keys: Vec<APIKey>
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Exchange {
    pub user_message: String,
    pub assistant_message: String
}