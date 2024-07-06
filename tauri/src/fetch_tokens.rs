use anyhow::{anyhow, Result};
use common::{APIKey, Config, Exchange, Provider, to_serde_err};
use futures::StreamExt;
use reqwest::{header::{HeaderMap, HeaderValue, CONTENT_TYPE}, RequestBuilder};
use reqwest_eventsource::{Event, EventSource};
use serde_error::Error;
use serde_json::{json, Value};

fn build_openai_request_body(
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str,
) -> serde_json::Value {
    let messages = exchanges.iter()
        .flat_map(|exchange|
            vec![json!({
                "role": "user",
                "content": exchange.user_message
            }), json!({
                "role": "assistant",
                "content": exchange.assistant_message
            })])
        .chain(std::iter::once(json!({
            "role": "user",
            "content": prompt
        })));

    return json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": true,
        "messages": std::iter::once(json!({
            "role": "system",
            "content": config.system_prompt
        })).chain(messages).collect::<Vec<Value>>()
    });
}

fn build_anthropic_request_body(
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str,
) -> serde_json::Value {
    let messages = exchanges.iter()
        .flat_map(|exchange|
            vec![json!({
                "role": "user",
                "content": exchange.user_message
            }), json!({
                "role": "assistant",
                "content": exchange.assistant_message
            })])
        .chain(std::iter::once(json!({
            "role": "user",
            "content": prompt
        })))
        .collect::<Vec<Value>>();

    return json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": true,
        "system": config.system_prompt,
        "messages": messages
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

fn build_request(
    api_key: &APIKey,
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str,
) -> Result<RequestBuilder> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let request_builder = match api_key.provider {
        Provider::OpenAI => {
            headers.insert("Authorization",
                HeaderValue::from_str(&format!("Bearer {}", api_key.key))?);

            reqwest::Client::new()
                .post("https://api.openai.com/v1/chat/completions")
                .headers(headers)
                .body(build_openai_request_body(config, exchanges, prompt).to_string())
        },
        Provider::Anthropic => {
            headers.insert("x-api-key", HeaderValue::from_str(&api_key.key)?);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

            reqwest::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .headers(headers)
                .body(build_anthropic_request_body(config, exchanges, prompt).to_string())
        }
    };

    return Ok(request_builder);
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

    let request_builder = build_request(&api_key, &config, exchanges, prompt).map_err(to_serde_err)?;

    let event_source = EventSource::new(request_builder)
        .map_err(|error| Error::new(&error))?;

    tokio::spawn(collect_tokens(api_key.provider, event_source, window));

    Ok(())
}