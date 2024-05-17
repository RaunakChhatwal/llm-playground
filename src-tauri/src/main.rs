// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;
use anyhow::{anyhow, Context, Result};
use config::Config;

mod config;

fn config_path() -> Result<std::path::PathBuf> {
    let config_dir = dirs::config_dir()
            .ok_or(anyhow!("Unable to find the config directory"))?
            .join("llm-playground");
    if !Path::exists(&Path::new(&config_dir)) {
        std::fs::create_dir(&config_dir)
            .context("Error creating config directory")?;
    }

    return Ok(config_dir.join("config.json"));
}

fn _load_config() -> Result<Config, String> {
    let config: Config;
    let config_path = config_path().map_err(|error| error.to_string())?;
    match std::fs::read_to_string(config_path) {
        Ok(config_str) => {
            config = serde_json::from_str(&config_str)
                .context("Unable to parse config")
                .map_err(|error| error.to_string())?;
        },
        Err(error) => {
            if !matches!(error.kind(), std::io::ErrorKind::NotFound) {
                return Err(error.to_string());
            }

            config = Config {
                temperature: 1.0,
                max_tokens: 1024,
                model: String::new(),
                api_key: None,
                api_keys: vec![]
            };
            save_config(config.clone())?;
        }
    }

    return Ok(config);
}

#[tauri::command]
fn load_config() -> String {
    return serde_json::to_string(&_load_config())
        .expect("Result<Config, String> should always successfully serialize.");
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    std::fs::write(
        config_path().map_err(|error| error.to_string())?,
        &serde_json::to_string(&config).expect("Serializing Config should always succeed")
    ).map_err(|error| error.to_string())?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_config, save_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
