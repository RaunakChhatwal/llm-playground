use anyhow::{anyhow, Result};
use futures::{stream::Stream, StreamExt};
use reqwest::{header::{HeaderMap, HeaderValue, CONTENT_TYPE}, RequestBuilder};
use reqwest_eventsource::{Event, EventSource};
use serde_json::{json, Value};

use crate::util::{Config, Exchange, Provider};

fn build_openai_request(key: &str) -> Result<RequestBuilder> {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", key))?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let request_builder = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .headers(headers);

    return Ok(request_builder);
}

fn build_anthropic_request(key: &str) -> Result<RequestBuilder> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(key)?);
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let request_builder = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .headers(headers);

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
) -> Result<Option<String>> {   // Ok(None) represents a stop message
    match provider {
        Provider::OpenAI => {
            if message.data.trim() == "[DONE]" {
                return Ok(None);
            }

            serde_json::from_str::<serde_json::Value>(&message.data)
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
                .map(|token| Some(token))
                .ok_or(anyhow!("Error parsing response."))
        },
        Provider::Anthropic => {
            if message.event != "content_block_delta" {
                return Ok(Some("".to_string()));
            }

            serde_json::from_str::<serde_json::Value>(&message.data)
                .ok()
                .and_then(|data| {
                    data["delta"]["text"]
                        .as_str()
                        .map(|token|
                            token.to_string())
                })
                .map(|token| Some(token))
                .ok_or(anyhow!("Error parsing response."))
        }
    }
}

pub fn fetch_tokens(
    prompt: &str,
    config: &Config,
    exchanges: Vec<Exchange>,
) -> Result<impl Stream<Item = Result<Option<String>>>> {
    let api_key = config.api_keys[
        config.api_key.ok_or(anyhow!("No API Key selected."))?
    ].clone();

    let body = build_request_body(prompt, config, exchanges);
    let request_builder = match api_key.provider {
        Provider::OpenAI => build_openai_request(&api_key.key)?
            .body(body.to_string()),
        Provider::Anthropic => build_anthropic_request(&api_key.key)?
            .body(body.to_string()),
    };

    let event_source = EventSource::new(request_builder)?;
    return Ok(event_source.map(move |event|
        match event {
            Ok(Event::Open) => Ok(Some("".to_string())),
            Ok(Event::Message(message)) =>
                interpret_message(message, api_key.provider),
            Err(reqwest_eventsource::Error::StreamEnded) => Ok(None),
            Err(error) => Err(anyhow!("{error}"))
        }
    ));
}