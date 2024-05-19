use anyhow::{anyhow, Result};
use futures::{channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender}, SinkExt, StreamExt};
use lazy_static::lazy_static;
use reqwest::{header::{HeaderMap, HeaderValue, CONTENT_TYPE}, RequestBuilder};
use reqwest_eventsource::{Event, EventSource};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use crate::util::{APIKey, Config, Exchange, Provider};

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
) -> String {
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
    }).to_string();
}

fn interpret_message(
    message: eventsource_stream::Event,
    provider: Provider
) -> Option<Result<String>> {   // None represents response end
    match provider {
        Provider::OpenAI => {
            if message.data.trim() == "[DONE]" {
                return None;
            }

            let token_result = serde_json::from_str::<serde_json::Value>(&message.data)
                .ok()
                .and_then(|data| {
                    if !data["choices"][0]["finish_reason"].is_null() {
                        return Some("".to_string());
                    }

                    data["choices"][0]["delta"]["content"]
                        .as_str()
                        .map(|token|
                            token.to_string())
                })
                .ok_or(anyhow!("Error parsing response."));

            return Some(token_result);
        },
        Provider::Anthropic => {
            if message.event != "content_block_delta" {
                return Some(Ok("".to_string()));
            }

            let token_result = serde_json::from_str::<serde_json::Value>(&message.data)
                .ok()
                .and_then(|data| {
                    data["delta"]["text"]
                        .as_str()
                        .map(|token|
                            token.to_string())
                })
                .ok_or(anyhow!("Error parsing response."));

            return Some(token_result);
        }
    }
}

lazy_static! {
    pub static ref CHANNEL: (
        UnboundedSender<Option<Result<String>>>,
        Mutex<UnboundedReceiver<Option<Result<String>>>>
    ) = {
        let (sender, recv) = unbounded();
        (sender, Mutex::new(recv))
    };
}

pub fn build_token_stream(
    prompt: &str,
    config: &Config,
    exchanges: Vec<Exchange>
) -> Result<()> {
    let api_key = config.api_keys[config.api_key
        .ok_or(anyhow!("No API Key selected."))?].clone();

    let request_builder = build_request(&api_key)?
        .body(build_request_body(prompt, config, exchanges));

    let mut event_source = EventSource::new(request_builder)?;
    tokio::spawn(async move {
        while let Some(event) = event_source.next().await {
            let token = match event {
                Ok(Event::Open) => Some(Ok("".into())),
                Ok(Event::Message(message)) => interpret_message(message, api_key.provider),
                Err(reqwest_eventsource::Error::StreamEnded) => None,
                Err(error) => Some(Err(error.into()))
            };
            let whether_stop = token.is_none();

            let mut sender = CHANNEL.0.clone();
            if let Err(_) = sender.send(token).await {
                break;
            }

            if whether_stop {
                event_source.close();
                break;
            }
        }
    });

    Ok(())
}