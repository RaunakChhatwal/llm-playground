use anyhow::{anyhow, bail, Context, Result};
use common::{APIKey, Config, Exchange, Provider, to_serde_err};
use eventsource_stream::{Event, Eventsource};
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_error::Error;
use serde_json::{json, Value};

fn build_openai_request_body(
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str
) -> serde_json::Value {
    let messages = exchanges.iter()
        .flat_map(|exchange| vec![
            json!({
                "role": "user",
                "content": exchange.user_message
            }),
            json!({
                "role": "assistant",
                "content": exchange.assistant_message
            })
        ])
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

// Ok(None) represents response end
fn parse_openai_response(message: Event) -> Result<Option<String>> {
    if message.data.trim() == "[DONE]" {
        return Ok(None);
    }

    let response = serde_json::from_str::<Value>(&message.data)
        .context("Error parsing response.")?;

    if !response["choices"][0]["finish_reason"].is_null() {
        return Ok(None);
    }

    if let Some(tokens) = response["choices"][0]["delta"]["content"].as_str() {
        return Ok(Some(tokens.into()));
    } else {
        bail!("Error parsing response.");
    }
}

fn build_anthropic_request_body(
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str
) -> serde_json::Value {
    let messages = exchanges.iter()
        .flat_map(|exchange| vec![
            json!({
                "role": "user",
                "content": exchange.user_message
            }),
            json!({
                "role": "assistant",
                "content": exchange.assistant_message
            })
        ])
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

// Ok(None) represents response end
fn parse_anthropic_response(message: Event) -> Result<Option<String>> {
    dbg!(&message);
    let response = serde_json::from_str::<Value>(&message.data)
        .context("Error parsing response.")?;

    if message.event != "content_block_delta" {
        return Ok(Some("".into()));
    }

    if let Some(tokens) = response["delta"]["text"].as_str() {
        return Ok(Some(tokens.into()));
    } else {
        bail!("Error parsing response.");        
    }
}

fn build_google_request_body(
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str
) -> serde_json::Value {
    let messages = exchanges.iter()
        .flat_map(|exchange| vec![
            json!({
                "role": "user",
                "parts": [{ "text": exchange.user_message }]
            }),
            json!({
                "role": "model",
                "parts": [{ "text": exchange.assistant_message }]
            })
        ])
        .chain(std::iter::once(json!({
            "role": "user",
            "parts": [{ "text": prompt }]
        })))
        .collect::<Vec<Value>>();

    return json!({
        "generation_config": {
            "temperature": config.temperature,
            "maxOutputTokens": config.max_tokens
        },
        "system_instruction": {
            "parts": [{ "text": config.system_prompt }]
        },
        "safety_settings": [
            {
                "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_HATE_SPEECH",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_HARASSMENT",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                "threshold": "BLOCK_NONE"
            }
        ],
        "contents": messages
    });
}

// Ok(None) represents response end
fn parse_google_response(message: bytes::Bytes) -> Result<Option<String>> {
    let message = String::from_utf8(message.into())?;
    let mut message = message.trim();
    if message.starts_with("[") || message.starts_with(",") {
        message = &message[1..];
    }
    if message.ends_with("]") {
        message = &message[..message.len() - 1];
        if message == "" {
            return Ok(None);
        }
    }

    let response = serde_json::from_str::<Value>(&message)
        .context("Error parsing response.")?;

    if !response["error"].is_null() {
        let error_message = response["error"]["message"].as_str()
            .unwrap_or("Error with request.");
        return Err(anyhow!("{error_message}"));
    }
    
    if let Some(tokens) = response["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        return Ok(Some(tokens.into()));
    } else {
        bail!("Error parsing response.");        
    }
}

async fn rate_limit<T>(
    tokens_stream: &mut (impl Stream<Item = T> + std::marker::Unpin),
    last_event_timestamp: std::time::Instant
) -> Option<T> {
    // tauri can't process events much faster than this
    let bottleneck = 25;

    let elapsed = std::time::Instant::now().duration_since(last_event_timestamp).as_millis();
    if elapsed > bottleneck as u128 {
        return tokens_stream.next().await;
    }
    let elapsed = elapsed as u64;

    let (event, _) = futures::join!(
        tokens_stream.next(),
        tokio::time::sleep(tokio::time::Duration::from_millis(bottleneck - elapsed))
    );

    return event;
}

async fn collect_tokens(
    mut tokens_stream: impl Stream<Item = Result<Option<String>>> + std::marker::Unpin,
    window: tauri::Window
) -> Result<()> {
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let id = window.listen("cancel", {
        let cancel = cancel.clone();
        move |_| cancel.notify_one()
    });

    let mut last_event_timestamp = std::time::Instant::now();
    loop {
        tokio::select! {
            _ = cancel.notified() => {
                if let Err(error) = window.emit("token", Ok::<_, String>(None::<String>)) {
                    eprintln!("{error}");
                }
                break;
            }

            tokens = rate_limit(&mut tokens_stream, last_event_timestamp) => {
                let Some(tokens) = tokens else {
                    if let Err(error) = window.emit("token", Ok::<_, String>(None::<String>)) {
                        eprintln!("{error}");
                    }
                    break;
                };

                // skip if empty token
                if tokens.as_ref().map(|tokens| tokens == &Some("".into())).unwrap_or(false) {
                    continue;
                }

                let tokens = tokens.map_err(to_serde_err);
                match window.emit("token", &tokens) {
                    Ok(_) => last_event_timestamp = std::time::Instant::now(),
                    Err(error) => {
                        eprintln!("{error}");
                        break;
                    }
                }

                // if tokens is Ok(None)
                if tokens.as_ref().map(Option::is_none).unwrap_or(false) {
                    break;
                }
            }
        }
    }

    window.unlisten(id);

    Ok(())
}

fn build_request(
    api_key: &APIKey,
    config: &Config,
    exchanges: Vec<Exchange>,
    prompt: &str,
) -> Result<reqwest::RequestBuilder> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let request_builder = match api_key.provider {
        Provider::OpenAI => {
            headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", api_key.key))?);

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
        Provider::Google => {
            headers.insert("x-goog-api-key", HeaderValue::from_str(&api_key.key)?);

            let domain = "generativelanguage.googleapis.com";
            reqwest::Client::new()
                .post(format!("https://{domain}/v1beta/models/{}:streamGenerateContent", config.model))
                .headers(headers)
                .body(build_google_request_body(config, exchanges, prompt).to_string())
        },
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
    let api_key_index = config.api_key.ok_or(to_serde_err(anyhow!("No API key selected.")))?;
    let api_key = &config.api_keys[api_key_index];

    let request = build_request(api_key, &config, exchanges, prompt).map_err(to_serde_err)?;
    let response = request.send().await
        .map_err(|error| Error::new(&error))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(to_serde_err(anyhow!("Invalid status code: {}", response.status())));
    }

    let tokens_stream: Box<dyn Stream<Item = Result<Option<String>>> + std::marker::Unpin + Send>;
    match api_key.provider {
        Provider::OpenAI => tokens_stream = Box::new(response.bytes_stream()
            .eventsource()
            .map(|event| event.map_err(Into::into)
                .map(parse_openai_response)
                .unwrap_or_else(Err))),
        Provider::Anthropic => tokens_stream = Box::new(response.bytes_stream()
            .eventsource()
            .map(|event| event.map_err(Into::into)
                .map(parse_anthropic_response)
                .unwrap_or_else(Err))),
        Provider::Google => tokens_stream = Box::new(response.bytes_stream()
            .map(|event| event.map_err(Into::into)
                .map(parse_google_response)
                .unwrap_or_else(Err)))
    }

    tokio::spawn(collect_tokens(tokens_stream, window));

    Ok(())
}