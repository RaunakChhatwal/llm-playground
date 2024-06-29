use anyhow::{anyhow, Result};
use common::{APIKey, Config, Exchange, Provider, to_serde_err};
use futures::StreamExt;
use reqwest::{header::{HeaderMap, HeaderValue, CONTENT_TYPE}, RequestBuilder};
use reqwest_eventsource::{Event, EventSource};
use serde_error::Error;
use serde_json::{json, Value};

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
) -> Result<Option<String>, Error> {   // Ok(None) represents response end
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
                .ok_or(to_serde_err(anyhow!("Error parsing response.")));

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
                .ok_or(to_serde_err(anyhow!("Error parsing response.")));

            return token_result.map(|token| Some(token));
        }
    }
}

async fn collect_tokens(
    provider: Provider,
    mut event_source: EventSource,
    window: tauri::Window
) -> Result<()> {
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let id = window.listen("cancel", {
        let cancel = cancel.clone();
        move |_| cancel.notify_one()
    });

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                let _ = window.emit("token", Ok::<_, String>(None::<String>));
                break;
            }
            event = event_source.next() => {
                let Some(event) = event else {
                    let _ = window.emit("token", Ok::<_, String>(None::<String>));
                    break;
                };

                let token = match event {
                    Ok(Event::Open) => Ok(Some("".into())),
                    Ok(Event::Message(message)) =>
                        interpret_message(message, provider),
                    Err(reqwest_eventsource::Error::StreamEnded) => Ok(None),
                    Err(error) => Err(Error::new(&error))
                };

                if let Err(_) = window.emit("token", &token) {
                    break;
                }

                if token.as_ref().map(Option::is_none).unwrap_or(false) {
                    break;
                }
            }
        }
    }

    event_source.close();
    window.unlisten(id);

    Ok(())
}

#[tauri::command]
pub async fn build_token_stream(
    window: tauri::Window,
    prompt: &str,
    config: Config,
    exchanges: Vec<Exchange>
) -> Result<(), Error> {
    let api_key = config.api_keys[config.api_key
        .ok_or(to_serde_err(anyhow!("No API Key selected.")))?].clone();

    let request_builder = build_request(&api_key)
        .map_err(to_serde_err)?
        .body(build_request_body(prompt, &config, exchanges).to_string());

    let event_source = EventSource::new(request_builder)
        .map_err(|error| Error::new(&error))?;

    tokio::spawn(collect_tokens(api_key.provider, event_source, window));

    Ok(())
}