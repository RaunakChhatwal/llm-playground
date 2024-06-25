use anyhow::{anyhow, Result};
use common::{APIKey, Config, Exchange, Provider, to_serde_err};
use futures::StreamExt;
use reqwest::{header::{HeaderMap, HeaderValue, CONTENT_TYPE}, RequestBuilder};
use reqwest_eventsource::EventSource;
use serde_error::Error;
use serde_json::{json, Value};
use tokio::sync::{mpsc::{UnboundedSender, UnboundedReceiver}, Mutex, Notify};

fn build_request(api_key: &APIKey) -> Result<RequestBuilder> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let request_builder = match api_key.provider {
        Provider::OpenAI => {
            headers.insert("Authorization",
                HeaderValue::from_str(&format!("Bearer {}", api_key.key))?);

            reqwest::Client::new()
                .post("https://api.openai.com/v1/chat/completions")
                .headers(headers)
        },
        Provider::Anthropic => {
            headers.insert("x-api-key", HeaderValue::from_str(&api_key.key)?);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

            reqwest::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .headers(headers)
        }
    };

    return Ok(request_builder);
}

fn build_request_body(
    prompt: &str,
    config: &Config,
    exchanges: Vec<Exchange>,
) -> serde_json::Value {
    return json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": true,
        "messages": exchanges
            .iter()
            .flat_map(|Exchange { user_message, assistant_message }|
                vec![json!({
                    "role": "user",
                    "content": user_message
                }), json!({
                    "role": "assistant",
                    "content": assistant_message
                })])
            .chain(std::iter::once(json!({
                "role": "user",
                "content": prompt
            })))
            .collect::<Vec<Value>>()
    });
}

fn interpret_message(
    message: eventsource_stream::Event,
    provider: Provider
) -> Result<Option<String>> {   // Ok(None) represents response end
    match provider {
        Provider::OpenAI => {
            if message.data.trim() == "[DONE]" {
                return Ok(None);
            }

            let token_result = serde_json::from_str::<Value>(&message.data)
                .ok()
                .and_then(|data| {
                    if !data["choices"][0]["finish_reason"].is_null() {
                        return Some("".to_string());
                    }

                    data["choices"][0]["delta"]["content"]
                        .as_str()
                        .map(|token| token.to_string())
                })
                .ok_or(anyhow!("Error parsing response."));

            return token_result.map(|token| Some(token));
        },
        Provider::Anthropic => {
            if message.event != "content_block_delta" {
                return Ok(Some("".to_string()));
            }

            let token_result = serde_json::from_str::<Value>(&message.data)
                .ok()
                .and_then(|data| data["delta"]["text"]
                    .as_str()
                    .map(|token| token.to_string())
                )
                .ok_or(anyhow!("Error parsing response."));

            return token_result.map(|token| Some(token));
        }
    }
}

lazy_static::lazy_static! {
    pub static ref CHANNEL: (
        UnboundedSender<Result<Option<String>>>,
        Mutex<UnboundedReceiver<Result<Option<String>>>>
    ) = {
        let (sender, recv) = tokio::sync::mpsc::unbounded_channel();
        (sender, Mutex::new(recv))
    };
}

async fn clear_channel() {
    let mut recv = CHANNEL.1.lock().await;
    while let Ok(Ok(Some(token))) = recv.try_recv() {
        drop(token);
    }
}

lazy_static::lazy_static! {
    static ref CANCEL_NOTIFY: Notify = Notify::new();
}

async fn collect_tokens(
    mut event_source: reqwest_eventsource::EventSource,
    sender: UnboundedSender<Result<Option<String>>>,
    provider: Provider
) {
    use reqwest_eventsource::Event;

    loop {
        tokio::select! {
            _ = CANCEL_NOTIFY.notified() => {
                let _ = sender.send(Ok(None));
                event_source.close();
                break;
            }
            event = event_source.next() => {
                let Some(event) = event else {
                    let _ = sender.send(Ok(None));
                    event_source.close();
                    break;
                };

                let token = match event {
                    Ok(Event::Open) => Ok(Some("".into())),
                    Ok(Event::Message(message)) =>
                        interpret_message(message, provider),
                    Err(reqwest_eventsource::Error::StreamEnded) => Ok(None),
                    Err(error) => Err(error.into())
                };
                let whether_stop = token
                    .as_ref()
                    .map(Option::is_none)
                    .unwrap_or(false);

                if let Err(_) = sender.send(token) {
                    event_source.close();
                    break;
                }

                if whether_stop {
                    event_source.close();
                    break;
                }
            }
        }
    }
}

#[tauri::command]
pub async fn build_token_stream(
    prompt: &str,
    config: Config,
    exchanges: Vec<Exchange>
) -> Result<(), Error> {
    clear_channel().await;

    let api_key = config.api_keys[config.api_key
        .ok_or(to_serde_err(anyhow!("No API Key selected.")))?].clone();

    let request_builder = build_request(&api_key)
        .map_err(to_serde_err)?
        .body(build_request_body(prompt, &config, exchanges).to_string());

    let event_source = EventSource::new(request_builder)
        .map_err(|error| Error::new(&error))?;
    let sender = CHANNEL.0.clone();
    tokio::spawn(collect_tokens(event_source, sender, api_key.provider));

    Ok(())
}

#[tauri::command]
pub async fn fetch_tokens() -> Result<Option<String>, Error> {
    let mut recv = CHANNEL.1.lock().await;
    return recv.recv().await.unwrap_or(Ok(None)).map_err(to_serde_err);
}

#[tauri::command]
pub fn cancel() {
    CANCEL_NOTIFY.notify_waiters();
}