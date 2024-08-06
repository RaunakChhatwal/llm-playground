use strum_macros::{EnumString, VariantNames};
use serde::{Deserialize, Serialize};

pub fn to_serde_err(error: anyhow::Error) -> serde_error::Error {
    serde_error::Error::new(&*error)
}

#[derive(Clone, Copy, Debug, Deserialize, strum_macros::Display, EnumString,
    Eq, Hash, PartialEq, Serialize, VariantNames)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Google
}

impl Default for Provider {
    fn default() -> Self {
        Provider::OpenAI
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct APIKey {
    pub name: String,
    pub key: String,
    pub provider: Provider
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Config {
    pub system_prompt: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub model: String,
    pub api_key: Option<usize>,
    pub api_keys: Vec<APIKey>
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system_prompt: "no yapping".into(),
            temperature: 0.8,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Conversation {
    pub uuid: uuid::Uuid,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub title: String
}