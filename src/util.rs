use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Provider {
    OpenAI,
    Anthropic
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct APIKey {
    pub name: String,
    pub key: String,
    pub provider: Provider
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Config {
    pub temperature: f64,
    pub max_tokens: u32,
    pub model: String,
    pub api_key: Option<usize>,
    pub api_keys: Vec<APIKey>
}

impl Default for Config {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 1024,
            model: "".into(),
            api_key: None,
            api_keys: vec![]
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Exchange {
    pub user_message: String,
    pub assistant_message: String
}