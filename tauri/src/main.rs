// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{ops::Deref, path::Path};
use anyhow::{anyhow, Context, Result};
use common::{to_serde_err, Config, Conversation, Exchange};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde_error::Error;
use tauri::Manager;
use fetch_tokens::build_token_stream;

mod fetch_tokens;

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

// the database connection to <config-dir>/conversations.db
// I chose sqlite over json for consistency
lazy_static::lazy_static! {
    static ref CONN: Result<sea_orm::DatabaseConnection> = futures::executor::block_on(async {
        let db_path = config_dir().await?
            .join("conversations.db")
            .to_str()
            .map(str::to_string)
            .ok_or(anyhow!("Unable to connect to database."))?;

        Database::connect(&format!("sqlite://{}", db_path)).await
            .map_err(Into::into)
    });
}

async fn _load_conversations() -> Result<Vec<Conversation>> {
    let conn = CONN.as_ref().map_err(Deref::deref)?;
    let conversations: Vec<(
        entity::conversations::Model,
        Option<entity::exchanges::Model>
    )> = entity::conversations::Entity::find()
        .find_also_related(entity::exchanges::Entity)
        .all(conn).await?;

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
        .context("Unable to load conversations")
        .map_err(to_serde_err)
}

async fn add_exchanges(
    conversation_id: i32,
    exchanges: Vec<(usize, Exchange)>,
    txn: &sea_orm::DatabaseTransaction
) -> Result<Vec<entity::exchanges::Model>> {
    futures::future::join_all(exchanges.into_iter().map(|(key, exchange)| async move {
        entity::exchanges::ActiveModel {
            key: Set(key.try_into()?),
            user_message: Set(exchange.user_message),
            assistant_message: Set(exchange.assistant_message),
            conversation: Set(conversation_id),
            ..Default::default()
        }.insert(txn).await.map_err(anyhow::Error::from)
    })).await.into_iter().collect::<Result<Vec<_>, _>>()
}

async fn _add_conversation(mut exchanges: Vec<(usize, Exchange)>) -> Result<uuid::Uuid> {
    let txn = CONN.as_ref().map_err(Deref::deref)?.begin().await?;

    if exchanges.is_empty() {
        anyhow::bail!("Conversation cannot be set empty.");
    }
    let (first_exchange_key, first_exchange) = exchanges.remove(0);
    let first_exchange = entity::exchanges::ActiveModel {
        key: Set(first_exchange_key.try_into()?),
        user_message: Set(first_exchange.user_message),
        assistant_message: Set(first_exchange.assistant_message),
        conversation: Set(-1),
        ..Default::default()
    }.insert(&txn).await?;

    let conversation_uuid = uuid::Uuid::new_v4();
    let conversation = entity::conversations::ActiveModel {
        uuid: Set(conversation_uuid.into()),
        last_updated: Set(chrono::Utc::now().timestamp()),
        first_exchange: Set(first_exchange.id),
        ..Default::default()
    }.insert(&txn).await?;

    add_exchanges(conversation.id, exchanges, &txn).await?;
    let mut first_exchange = entity::exchanges::ActiveModel::from(first_exchange);
    first_exchange.conversation = Set(conversation.id);
    first_exchange.update(&txn).await?;

    txn.commit().await?;

    return Ok(conversation_uuid);
}

#[tauri::command]
async fn add_conversation(exchanges: Vec<(usize, Exchange)>) -> Result<uuid::Uuid, Error> {
    _add_conversation(exchanges).await.map_err(to_serde_err)
}

async fn _set_exchanges(
    conversation_uuid: uuid::Uuid,
    exchanges: Vec<(usize, Exchange)>
) -> Result<Option<uuid::Uuid>> {
    let txn = CONN.as_ref().map_err(Deref::deref)?.begin().await?;

    let conversation = entity::conversations::Entity::find()
        .filter(entity::conversations::Column::Uuid.eq(conversation_uuid))
        .one(&txn).await?;
    let Some(conversation) = conversation else {
        return Ok(Some(_add_conversation(exchanges).await?));
    };

    // remove exchanges from database
    entity::exchanges::Entity::delete_many()
        .filter(entity::exchanges::Column::Conversation.eq(conversation.id))
        .exec(&txn).await?;

    let exchanges = add_exchanges(conversation.id, exchanges, &txn).await?;
    let first_exchange = exchanges.get(0).ok_or(anyhow!("Conversation cannot be set empty."))?;

    let mut conversation = entity::conversations::ActiveModel::from(conversation);
    conversation.first_exchange = Set(first_exchange.id);
    conversation.last_updated = Set(chrono::Utc::now().timestamp());
    conversation.update(&txn).await?;

    txn.commit().await?;

    return Ok(None);
}

#[tauri::command(rename_all = "snake_case")]
async fn set_exchanges(
    conversation_uuid: uuid::Uuid,
    exchanges: Vec<(usize, Exchange)>
) -> Result<Option<uuid::Uuid>, Error> {
    _set_exchanges(conversation_uuid, exchanges).await.map_err(to_serde_err)
}

fn watch_file(app: tauri::AppHandle, event_name: &'static str, file: &Path) -> Result<()> {
    let (sender, recv) = std::sync::mpsc::channel::<Result<notify::Event, notify::Error>>();

    let emit = move || app.emit_all(event_name, ()).unwrap_or_else(|error|
        eprintln!("Error triggering {event_name}: {error}"));

    std::thread::spawn(move || loop {
        let event= match recv.recv() {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                eprintln!("Error listening for {event_name}: {error}");
                emit();
                continue;
            },
            // this means the recv is closed, should never happen
            Err(_) => {
                eprintln!("Watcher disconnected!");
                // not breaking will result in an infinite loop
                break;
            }
        };
        match event.kind {
            notify::EventKind::Create(_)
            | notify::EventKind::Modify(_)
            | notify::EventKind::Remove(_) => emit(),
            // ignore miscellaneous events
            _ => continue
        }
    });

    let mut watcher = RecommendedWatcher::new(sender, Default::default())?;
    watcher.watch(file, RecursiveMode::Recursive)?;
    std::mem::forget(watcher);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let app = app.handle();
            futures::executor::block_on(tokio::spawn(async {
                watch_file(app.clone(), "config_updated", &config_dir().await?.join("config.json"))?;
                watch_file(app, "conversations_updated", &config_dir().await?.join("conversations.db"))?;

                Ok::<(), anyhow::Error>(())
            })).unwrap_or_else(|error| Err(error.into())).map_err(Into::into)
        })
        .invoke_handler(tauri::generate_handler![
            add_conversation,
            build_token_stream,
            load_config,
            load_conversations,
            save_config,
            set_exchanges
        ])
        .run(tauri::generate_context!())
        .map_err(Into::into)
}
