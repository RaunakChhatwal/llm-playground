// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs, path::Path};
use anyhow::{anyhow, Context, Result};
use common::Config;
use fetch_tokens::{build_token_stream, cancel, fetch_tokens};

mod fetch_tokens;

fn config_path() -> Result<std::path::PathBuf> {
    let config_dir = dirs::config_dir()
            .ok_or(anyhow!("Unable to find the config directory"))?
            .join("llm-playground");
    if !Path::exists(&Path::new(&config_dir)) {
        fs::create_dir(&config_dir)
            .context("Error creating config directory")?;
    }

    return Ok(config_dir.join("config.json"));
}

// TODO: change to async filesystem operations
#[tauri::command]
fn load_config() -> Result<Config, String> {
    let config: Config;
    let config_path = config_path().map_err(|error| error.to_string())?;
    match fs::read_to_string(config_path) {
        Ok(config_str) => {
            config = serde_json::from_str(&config_str)
                .context("Unable to parse config")
                .map_err(|error| error.to_string())?;
        },
        Err(error) => {
            if matches!(error.kind(), std::io::ErrorKind::NotFound) {
                config = Config::default();
                save_config(config.clone())?;
            } else {
                return Err(error.to_string());
            }
        }
    }

    return Ok(config);
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    let config_path = config_path().map_err(|error| error.to_string())?;
    let serialized_config = serde_json::to_string(&config)
        .expect("Config should always successfully serialize");
    fs::write(config_path, &serialized_config)
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            build_token_stream,
            cancel,
            fetch_tokens,
            load_config,
            save_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
