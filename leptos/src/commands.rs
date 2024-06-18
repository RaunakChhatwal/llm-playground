use common::{Config, Exchange};

#[macros::command]
pub async fn save_config(config: Config) -> Result<(), String> {}

#[macros::command]
pub async fn load_config() -> Result<Config, String> {}

#[macros::command]
pub async fn poll_config_change() -> Result<Config, String> {}

#[macros::command]
pub async fn build_token_stream(
    prompt: &str,
    config: Config,
    exchanges: Vec<Exchange>
) -> Result<(), String> {}

#[macros::command]
pub async fn fetch_tokens() -> Result<Option<String>, String> {}

#[macros::command]
pub async fn cancel() -> Result<(), String> {}