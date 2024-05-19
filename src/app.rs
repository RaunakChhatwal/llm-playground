use std::sync::Mutex;
use anyhow::{anyhow, Context, Result};
use futures::{channel::mpsc::UnboundedReceiver, SinkExt, StreamExt};
use leptos::*;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use crate::util::{Config, Exchange};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
fn Button(
    class: &'static str,
    label: &'static str,
    to_hide: Signal<bool>,
    on_click: Box<dyn Fn()>)
-> impl IntoView {
    let class = format!("{class}
        px-[6px] py-[2px] border-4 border-[#2A2A2A] bg-[#222222] hover:bg-[#2A2A2A] text-[#AAAABB]");

    view! {
        <button
            class={class}
            on:click=move |_| on_click()
            style:display=move || to_hide().then(|| "None")
        >{label}</button>
    }
}

#[component]
fn PromptBox(
    prompt: ReadSignal<String>,
    set_prompt: WriteSignal<String>,
) -> impl IntoView {
    let on_input = move |event| {
        set_prompt(event_target_value(&event));
        let prompt_box = document().get_element_by_id("prompt-box")
            .expect("This element exists.");
        prompt_box.set_attribute("style", "height: auto;")
            .expect("The style attribute is available for every element.");
        prompt_box.set_attribute("style", &format!("height: {}px;", prompt_box.scroll_height()))
            .expect("The style attribute is available for every element.");
    };

    // this is a hack because value=prompt entry in the view macro below does not work
    create_effect(move |_| {
        document().get_element_by_id("prompt-box")
            .expect("This element exists.")
            .dyn_into::<web_sys::HtmlTextAreaElement>()
            .expect("prompt-box is a textarea element.")
            .set_value(&prompt());
    });

    view! {
        <textarea
            id="prompt-box"
            class="flex-none w-full px-2 py-1 bg-[#222222] text-[0.9em] overflow-hidden resize-none"
            type="text"
            placeholder="Enter a prompt here."
            on:input=on_input
        ></textarea>
    }
}

#[derive(Deserialize, Serialize)]
struct FetchTokenArguments {
    prompt: String,
    config: Config,
    exchanges: Vec<Exchange>
}

async fn build_token_stream(prompt: String, exchanges: Vec<Exchange>)
-> Result<UnboundedReceiver<Result<String, String>>> {
    let serialized_config = invoke("_load_config",
        to_value(&serde_json::Value::Object(serde_json::Map::new()))
        .expect("The empty object should successfully serialize"))
        .await
        .as_string()
        .expect("load_config returns String");
    let config = serde_json::from_str::<Result<Config, String>>(&serialized_config)
        .context("Unable to parse config")?
        .map_err(|error_message| anyhow!("{error_message}"))?;

    let args = serde_wasm_bindgen::to_value(&FetchTokenArguments {
        prompt,
        config,
        exchanges
    }).map_err(|_| anyhow!("Error serializing fetch_token arguments"))?;
    invoke("_build_token_stream", args).await;

    let (mut sender, recv) = futures::channel::mpsc::unbounded();

    spawn_local(async move { loop {
        let token = invoke("fetch_tokens", JsValue::null()).await;
        if token.is_null() {
            return;
        }

        let Some(result_str) = token.as_string() else {
            let _ = sender.send(Err("Error parsing response.".into()));
            return;
        };

        match serde_json::from_str::<Result<String, String>>(&result_str) {
            Ok(token_result) => {
                if let Err(_) = sender.send(token_result).await {
                    return;
                }
            },
            Err(error) => {
                let _ = sender.send(Err(error.to_string())).await;
                return;
            }
        };
    }});

    return Ok(recv);
}

#[component]
pub fn App() -> impl IntoView {
    let (error, set_error) = create_signal("".to_string());
    let mut counter = 0usize;
    let (exchanges, set_exchanges) = create_signal(Vec::<(usize, RwSignal<Exchange>)>::new());
    let (prompt, set_prompt) = create_signal("".to_string());
    let (streaming, set_streaming) = create_signal(false);

    // wrapped around Mutex because on_submit must be Fn
    let on_submit_FnMut = Mutex::<Box<dyn FnMut()>>::new(Box::new(move || {
        set_streaming(true);
        set_error("".to_string());
        let prompt = prompt();
        set_prompt("".to_string());
        let exchanges = exchanges()
            .iter()
            .map(|(_, exchange)|
                exchange())
            .collect::<Vec<Exchange>>();

        let new_exchange = create_rw_signal(Exchange {
            user_message: prompt.clone(),
            assistant_message: "".to_string()
        });
        set_exchanges.update(|exchanges|
            exchanges.push((counter, new_exchange)));
        counter += 1;

        spawn_local(async move {
            let mut token_stream = match build_token_stream(prompt, exchanges).await {
                Ok(token_stream) => token_stream,
                Err(error) => {
                    set_error(error.to_string());
                    return;
                }
            };

            while let Some(token) = token_stream.next().await {
                match token {
                    Ok(token) => new_exchange.update(|exchange|
                        exchange.assistant_message.push_str(&token)),
                    Err(error) => {
                        set_error(error.to_string());
                        break;
                    }
                }
            }

            set_streaming(false);
        });
    }));

    let on_submit = move || {
        match on_submit_FnMut.try_lock() {
            Ok(mut on_submit) => on_submit(),
            Err(_) => return
        }
    };

    view! {
        <div class="flex flex-col h-full p-4 overflow-y-hidden text-[0.9rem]">
            <p
                class="mb-2 text-red-400 text-[0.9em]"
                style:display=move || error().is_empty().then(|| "None")
            >{error}</p>
            <div
                class="mb-4 overflow-y-auto"
                style:display=move || exchanges().is_empty().then(|| "None")
            >
                <div class="flex flex-col">
                    <For
                        each=exchanges
                        key=|exchange| exchange.0
                        children=move |(id, exchange)| view! {
                            <p
                                class="px-2 py-1 bg-[#222222] text-[0.9em]"
                                style:margin-top=move || (id > 0).then(|| "12px")
                            >{move || exchange().user_message}</p>
                            <p class="mt-[12px] px-2 py-1 min-h-6 bg-[#222222] text-[0.9em]">
                                {move || exchange().assistant_message}
                            </p>
                        }
                    />
                </div>
            </div>
            <div
                class=move || (exchanges().is_empty() && !streaming())
                    .then(|| "mb-auto").unwrap_or("mt-auto mb-2")
            >
                <PromptBox prompt set_prompt />
            </div>
            <div class="flex">
                <Button class="mr-4" label="New"
                    to_hide=streaming.into() on_click=Box::new(|| ())/>
                <Button class="" label="Submit"
                    to_hide=streaming.into() on_click=Box::new(on_submit) />
                <div class="flex ml-auto">
                    <Button class="mr-4" label="Cancel"
                        to_hide=Signal::derive(move || !streaming()) on_click=Box::new(|| ())/>
                    <Button class="" label="Settings"
                        to_hide=create_signal(false).0.into() on_click=Box::new(|| ())/>
                </div>
            </div>
        </div>
    }
}
