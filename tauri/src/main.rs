// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::Path, sync::mpsc::{Receiver, Sender}};
use anyhow::{anyhow, Context, Result};
use common::Config;
use fetch_tokens::{build_token_stream, cancel, fetch_tokens};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_error::Error;
use tokio::sync::Mutex;

mod fetch_tokens;

async fn config_dir() -> Result<std::path::PathBuf, Error> {
    let config_dir = dirs::config_dir()
            .ok_or(Error::new(&*anyhow!("Unable to find the config directory")))?
            .join("llm-playground");
    if !Path::exists(&Path::new(&config_dir)) {
        tokio::fs::create_dir(&config_dir).await
            .context("Error creating config directory")
            .map_err(|error| Error::new(&*error))?;
    }

    return Ok(config_dir);
}

#[tauri::command]
async fn load_config() -> Result<Config, Error> {
    let config: Config;
    let config_path = config_dir().await?.join("config.json");
    match tokio::fs::read_to_string(config_path).await {
        Ok(config_str) => {
            config = serde_json::from_str(&config_str)
                .context("Unable to parse config")
                .map_err(|error| Error::new(&*error))?;
        },
        Err(error) => {
            if matches!(error.kind(), std::io::ErrorKind::NotFound) {
                config = Config::default();
                save_config(config.clone()).await?;
            } else {
                return Err(Error::new(&error));
            }
        }
    }

    return Ok(config);
}

#[tauri::command]
async fn save_config(config: Config) -> Result<(), Error> {
    let config_path = config_dir().await?.join("config.json");
    let serialized_config = serde_json::to_string(&config)
        .expect("Config should always successfully serialize");
    tokio::fs::write(config_path, &serialized_config).await
        .map_err(|error| Error::new(&error))
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
async fn poll_config_change() -> Result<Config, Error> {
    let poll_config_change = || async {
        loop {
            let recv = CONFIG_CHANNEL.1.lock().await;
            let event = match recv.recv()  {
                Ok(Ok(event)) => event,
                Ok(Err(error)) => return Err(Error::new(&error)),
                Err(error) => return Err(Error::new(&error))
            };
            return match event.kind {
                notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_) => load_config().await,
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
async fn main() -> Result<()> {
    let sender = CONFIG_CHANNEL.0.clone();
    let mut watcher = RecommendedWatcher::new(sender, Default::default())?;
    watcher.watch(&config_dir().await?.join("config.json"), RecursiveMode::Recursive)?;

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
        .map_err(|_| anyhow!("Error running tauri application."))
}
