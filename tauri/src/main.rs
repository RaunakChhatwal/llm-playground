// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::Path, sync::mpsc::{Receiver, Sender}};
use anyhow::{anyhow, Context, Result};
use common::{Config, Conversation};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sea_orm::{Database, EntityTrait};
use serde_error::Error;
use tokio::sync::Mutex;
use fetch_tokens::{build_token_stream, cancel, fetch_tokens};

mod fetch_tokens;

fn to_serde_err(error: anyhow::Error) -> Error {
    Error::new(&*error)
}

async fn config_dir() -> Result<std::path::PathBuf, Error> {
    let config_dir = dirs::config_dir()
            .ok_or(to_serde_err(anyhow!("Unable to find the config directory")))?
            .join("llm-playground");
    if !Path::exists(&Path::new(&config_dir)) {
        tokio::fs::create_dir(&config_dir).await
            .context("Error creating config directory")
            .map_err(to_serde_err)?;
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
                .map_err(to_serde_err)?;
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
        .map_err(|error| Error::new(&error))?;
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
        let recv = CONFIG_CHANNEL.1.lock().await;
        loop {
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

lazy_static::lazy_static! {
    static ref CONVERSATIONS_CHANNEL: (
        Sender<Result<notify::Event, notify::Error>>,
        Mutex<Receiver<Result<notify::Event, notify::Error>>>
    ) = {
        // std::sync::mpsc because tokio::sync::mpsc doesn't implement notify::EventHandler
        let (sender, recv) = std::sync::mpsc::channel();
        (sender, Mutex::new(recv))
    };
}

async fn _load_conversations() -> Result<Vec<Conversation>> {
    let db_path = config_dir().await?
        .join("conversations.db")
        .to_str()
        .map(str::to_string)
        .ok_or(anyhow!("Unable to connect to database."))?;
    let connection = Database::connect(&format!("sqlite://{}", db_path)).await?;

    let conversations: Vec<(
        entity::conversations::Model,
        Option<entity::exchanges::Model>
    )> = entity::conversations::Entity::find()
        .find_also_related(entity::exchanges::Entity)
        .all(&connection).await?;

    return Ok(conversations.into_iter()
        .filter_map(|(conversation, exchange)| Some(Conversation {
            uuid: uuid::Uuid::from_slice(&conversation.uuid).ok()?,
            time: chrono::DateTime::from_timestamp(conversation.last_updated, 0)?,
            title: exchange?.user_message,
        }))
        .collect());
}

#[tauri::command]
async fn load_conversations() -> Result<Vec<Conversation>, Error> {
    _load_conversations().await
        .context("Unable to connect to conversations.db")
        .map_err(to_serde_err)
}

#[tauri::command]
async fn poll_conversations_change() -> Result<Vec<Conversation>, Error> {
    let poll_conversations_change = || async {
        let recv = CONVERSATIONS_CHANNEL.1.lock().await;
        loop {
            let event = match recv.recv() {
                Ok(Ok(event)) => event,
                Ok(Err(error)) => return Err(Error::new(&error)),
                Err(error) => return Err(Error::new(&error))
            };
            return match event.kind {
                notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_) => load_conversations().await,
                // ignore miscellaneous events
                _ => continue
            };
        }
    };

    loop {
        // spawning because recv is sync
        if let Ok(conversations_result) = tokio::spawn(poll_conversations_change()).await {
            return conversations_result;
        }
    }
}

fn watch_file(sender: Sender<Result<notify::Event, notify::Error>>, file: &Path) -> Result<()> {
    let mut watcher = RecommendedWatcher::new(sender, Default::default())?;
    watcher.watch(file, RecursiveMode::Recursive).map_err(Into::into)
}

#[tokio::main]
async fn main() -> Result<()> {
    watch_file(CONFIG_CHANNEL.0.clone(), &config_dir().await?.join("config.json"))?;
    watch_file(CONVERSATIONS_CHANNEL.0.clone(), &config_dir().await?.join("conversations.db"))?;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            build_token_stream,
            cancel,
            fetch_tokens,
            load_config,
            load_conversations,
            poll_config_change,
            poll_conversations_change,
            save_config
        ])
        .run(tauri::generate_context!())
        .map_err(Into::into)
}
