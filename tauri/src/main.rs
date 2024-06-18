// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs, path::Path, sync::mpsc::{Receiver, Sender}};
use anyhow::{Context, Result};
use common::Config;
use fetch_tokens::{build_token_stream, cancel, fetch_tokens};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Mutex;

mod fetch_tokens;

fn config_dir() -> Result<std::path::PathBuf, String> {
    let config_dir = dirs::config_dir()
            .ok_or("Unable to find the config directory".to_string())?
            .join("llm-playground");
    if !Path::exists(&Path::new(&config_dir)) {
        fs::create_dir(&config_dir)
            .map_err(|error| format!("Error creating config directory: {error}"))?;
    }

    return Ok(config_dir);
}

// TODO: change to async filesystem operations
#[tauri::command]
fn load_config() -> Result<Config, String> {
    let config: Config;
    let config_path = config_dir()?.join("config.json");
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
    let config_path = config_dir()?.join("config.json");
    let serialized_config = serde_json::to_string(&config)
        .expect("Config should always successfully serialize");
    fs::write(config_path, &serialized_config)
        .map_err(|error| error.to_string())?;

    Ok(())
}

lazy_static::lazy_static! {
    static ref CONFIG_CHANNEL: (
        Sender<Result<notify::Event, notify::Error>>,
        Mutex<Receiver<Result<notify::Event, notify::Error>>>
    ) = {
        // std::sync::mpsc because tokio::sync::mpsc doesn't implement notify::EventHandler
        let (sender, recv) = std::sync::mpsc::channel();
        (sender, Mutex::new(recv))
    };
}

#[tauri::command]
async fn poll_config_change() -> Result<Config, String> {
    let poll_config_change = || async {
        loop {
            let recv = CONFIG_CHANNEL.1.lock().await;
            let event = match recv.recv()  {
                Ok(Ok(event)) => event,
                Ok(Err(error)) => return Err(error.to_string()),
                Err(error) => return Err(error.to_string())
            };
            return match event.kind {
                notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_) => load_config(),
                // ignore miscellaneous events
                _ => continue
            };
        }
    };

    loop {
        // spawning because recv is sync
        if let Ok(config_result) = tokio::spawn(poll_config_change()).await {
            return config_result;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let sender = CONFIG_CHANNEL.0.clone();
    let mut watcher = RecommendedWatcher::new(sender, Default::default())
        .map_err(|error| error.to_string())?;
    watcher.watch(&config_dir()?.join("config.json"), RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            build_token_stream,
            cancel,
            fetch_tokens,
            load_config,
            poll_config_change,
            save_config
        ])
        .run(tauri::generate_context!())
        .map_err(|_| "Error running tauri application.".to_string())
}
