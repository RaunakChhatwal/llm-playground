use anyhow::Result;
use common::{Config, Exchange};

#[macros::command]
pub async fn save_config(config: Config) -> Result<()> {}

#[macros::command]
pub async fn load_config() -> Result<Config> {}

#[macros::command]
pub async fn poll_config_change() -> Result<Config> {}

#[macros::command]
pub async fn build_token_stream(
    prompt: &str,
    config: Config,
    exchanges: Vec<Exchange>
) -> Result<()> {}

#[macros::command]
pub async fn fetch_tokens() -> Result<Option<String>> {}

#[macros::command]
pub async fn cancel() -> Result<()> {}