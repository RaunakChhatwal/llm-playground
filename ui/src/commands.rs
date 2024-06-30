use anyhow::Result;
use common::{Config, Exchange};

#[macros::command]
pub async fn add_conversation(exchanges: Vec<(usize, Exchange)>) -> Result<uuid::Uuid> {}

#[macros::command]
pub async fn build_token_stream(
    prompt: &str,
    config: Config,
    exchanges: Vec<Exchange>
) -> Result<()> {}

#[macros::command]
pub async fn load_config() -> Result<Config> {}

#[macros::command]
pub async fn save_config(config: Config) -> Result<()> {}

#[macros::command]
pub async fn set_exchanges(
    conversation_uuid: uuid::Uuid,
    exchanges: Vec<(usize, Exchange)>
) -> Result<Option<uuid::Uuid>> {}