use std::collections::HashMap;
use anyhow::{anyhow, bail, Result};
use common::{Config, Exchange};
use futures::FutureExt;
use gloo_utils::format::JsValueSerdeExt;
use leptos::*;
use tokio::sync::mpsc::UnboundedReceiver;
use wasm_bindgen::{JsValue, prelude::*};
use crate::commands::{add_conversation, delete_conversation, load_exchanges};
use crate::util::{conversation_uuid, get_conversation_uuid_untracked, listen,
    set_conversation_uuid, set_conversation_uuid_untracked, Button, ErrorMessage, Menu};

lazy_static::lazy_static! {
    // anyhow! macro doesn't work if there is a static variable named "error" in the namespace
    pub static ref signal_pair: (ReadSignal<String>, WriteSignal<String>) = create_signal("".into());
    pub static ref set_error: WriteSignal<String> = signal_pair.1;
}

#[component]
fn MessageBox(
    id: String,
    rows: usize,
    class: String,
    placeholder: Option<String>,
    content: Signal<String>,
    set_content: SignalSetter<String>,
) -> impl IntoView {
    let on_input = {
        let id = id.clone();
        move |event| {
            set_content(event_target_value(&event));
            let content_box = document().get_element_by_id(&id)
                .expect("This element exists.");
            content_box.set_attribute("style", "height: auto;")
                .expect("The style attribute is available for every element.");
            let style = format!("height: {}px;", content_box.scroll_height());
            content_box.set_attribute("style", &style)
                .expect("The style attribute is available for every element.");
        }
    };

    // this is because value=content entry in the view macro below does not work
    create_effect({
        let id = id.clone();
        move |_| {
            let content_box = document().get_element_by_id(&id)
                .expect("This element exists.")
                .dyn_into::<web_sys::HtmlTextAreaElement>()
                .expect("This is a textarea element.");

            let content = content();
            if content_box.value() != content {
                // this is different from setting the textarea's value html attribute, which will not work
                content_box.set_value(&content);
                content_box.set_attribute("style", "height: auto;")
                    .expect("The style attribute is available for every element.");
                let style = format!("height: {}px;", content_box.scroll_height());
                content_box.set_attribute("style", &style)
                    .expect("The style attribute is available for every element.");
            }
        }
    });

    let class = format!("{} flex-none w-full min-h-[2em] px-2 pt-1 pb-2 border border-[#303038]
        bg-[#222222] text-[0.9em] overflow-hidden resize-none", class);
    view! {
        <textarea id=id rows=rows class=class type="text"
            placeholder=placeholder on:input=on_input
        ></textarea>
    }
}

#[component]
fn ExchangeComponent(
    key: usize,
    exchange: RwSignal<Exchange>,
    exchanges: RwSignal<Vec<(usize, RwSignal<Exchange>)>>,
) -> impl IntoView {
    let (user_message, set_user_message) = create_slice(
        exchange, 
        |exchange| exchange.user_message.clone(),
        |exchange, user_message| exchange.user_message = user_message
    );
    let (assistant_message, set_assistant_message) = create_slice(
        exchange, 
        |exchange| exchange.assistant_message.clone(),
        |exchange, assistant_message| exchange.assistant_message = assistant_message
    );

    let on_delete = move || {
        exchanges.update(|exchanges| {
            exchanges.retain(|(_key, _)| key != *_key);
            if exchanges.is_empty() {
                if let Some(uuid) = get_conversation_uuid_untracked() {
                    spawn_local(delete_conversation(uuid).map(drop));
                }
            } else {
                let exchanges = exchanges.iter()
                    .map(|(key, exchange)| (*key, exchange.get_untracked()))
                    .collect::<Vec<_>>();
                spawn_local(set_exchanges(exchanges));
            }
        })};

    view! {
        <div class="relative flex flex-col">
            <button
                class="absolute top-[-10px] right-[10px] text-[1.5rem] text-[#AAAABB]"
                on:click=move |_| on_delete()
            >"-"</button>
            <MessageBox id=format!("message-box-{}", 2*key) rows=1 class="".into()
                placeholder=None content=user_message set_content=set_user_message />
            <MessageBox id=format!("message-box-{}", 2*key + 1) rows=1 placeholder=None
                class="mt-[12px]".into() content=assistant_message set_content=set_assistant_message />
        </div>
    }
}

#[component]
fn Exchanges(
    new_exchange: RwSignal<Exchange>,
    exchanges: RwSignal<Vec<(usize, RwSignal<Exchange>)>>,
    streaming: RwSignal<bool>
) -> impl IntoView {
    view! {
        <div class="flex flex-col">
            <For each=exchanges
                key=|(key, _)| *key
                children=move |(key, exchange)| view! {
                    <div style:margin-top=move || exchanges().get(0)
                            .and_then(|(_key, _)| (key != *_key).then(|| "12px"))>
                        <ExchangeComponent key exchange exchanges />
                    </div>
                } />
        </div>
        <p class="px-2 py-1 min-h-[2em] bg-[#222222] border border-[#303038] text-[0.9em]"
            style:margin-top=move || (!exchanges().is_empty()).then(|| "12px")
            style:display=move || (!streaming()).then(|| "None")
        >{move || new_exchange().user_message}</p>
        <p class="mt-[12px] px-2 py-1 min-h-[2em] bg-[#222222] border border-[#303038] text-[0.9em]"
            style:display=move || (!streaming()).then(|| "None")
        >{move || new_exchange().assistant_message}</p>
    }
}

fn deserialize_event(event: JsValue) -> Result<Option<String>> {
    let parsed_event = JsValue::into_serde::<serde_json::Map<String, serde_json::Value>>(&event)?;
    let payload = parsed_event.get("payload").ok_or(anyhow!("Unable to deserialize token."))?;

    if let Some(token) = payload.get("Ok") {
        if token.is_null() {
            return Ok(None);    // signals end of response
        }

        if let Some(token) = token.as_str() {
            return Ok(Some(token.into()));
        }
    } else if let Some(error) = payload.get("Err") {
        if let Ok(error) = serde_json::from_value::<serde_error::Error>(error.clone()) {
            return Err(error.into());
        }
    }

    bail!("Unable to deserialize token.");
}

async fn build_token_stream(prompt: &str, config: Config, exchanges: Vec<Exchange>)
-> Result<UnboundedReceiver<Result<String>>> {
    crate::commands::build_token_stream(prompt, config, exchanges).await?;

    let (sender, recv) = tokio::sync::mpsc::unbounded_channel();
    let close = std::sync::Arc::new(tokio::sync::Notify::new());

    let on_token = {
        let close = close.clone();
        Closure::new(move |event: JsValue| {
            match deserialize_event(event) {
                Ok(Some(token)) => drop(sender.send(Ok(token))),
                Ok(None) => close.notify_one(),
                Err(error) => drop(sender.send(Err(error))),
            }
        })
    };

    let unlisten = listen("token", &on_token).await
        .map_err(|_| anyhow!("Error listening for tokens"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| anyhow!("Error listening for tokens"))?;

    spawn_local(async move {
        close.notified().await;
        let _ = unlisten.call0(&JsValue::null());
        drop(on_token);     // in order to keep on_token alive until closed
    });

    return Ok(recv);
}

async fn collect_tokens(
    mut token_stream: UnboundedReceiver<Result<String>>,
    exchange: RwSignal<Exchange>,
) {
    while let Some(token) = token_stream.recv().await {
        match token {
            Ok(token) => exchange.update(|exchange|
                exchange.assistant_message.push_str(&token)),
            Err(error) => {
                set_error(error.to_string());
                break;
            }
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"])]
    async fn emit(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

async fn set_exchanges(exchanges: Vec<(usize, Exchange)>) {
    if exchanges.is_empty() {
        set_error("A conversation cannot be empty.".into());
    }

    if let Some(uuid) = get_conversation_uuid_untracked() {
        match crate::commands::set_exchanges(uuid, exchanges).await {
            // if a different window deletes the current conversation
            // a new one is created with a new uuid
            Ok(Some(uuid)) => set_conversation_uuid_untracked(Some(uuid)),
            // error saving exchanges
            Err(error) => set_error(error.to_string()),
            // exchanges were successfully saved to conversation
            _ => ()
        }
    } else {
        match add_conversation(exchanges).await {
            Ok(uuid) => set_conversation_uuid(Some(uuid)),
            Err(error) => set_error(error.to_string()),
        }
    }
}

#[component]
fn BottomButtons(
    config: RwSignal<Config>,
    menu: RwSignal<Menu>,
    exchanges: RwSignal<Vec<(usize, RwSignal<Exchange>)>>,
    new_exchange: RwSignal<Exchange>,
    prompt: RwSignal<String>,
    streaming: RwSignal<bool>,
) -> impl IntoView {
    let on_submit = move || {
        streaming.set(true);
        set_error("".to_string());
        let _prompt = prompt();
        prompt.set("".to_string());
        let _exchanges = exchanges.get_untracked()
            .iter()
            .map(|(_, exchange)| exchange.get_untracked())
            .collect::<Vec<_>>();

        new_exchange.set(Exchange {
            user_message: _prompt.clone(),
            assistant_message: "".to_string()
        });

        spawn_local(async move {
            match build_token_stream(&_prompt, config.get_untracked(), _exchanges).await {
                Ok(token_stream) => collect_tokens(token_stream, new_exchange).await,
                Err(error) => set_error(error.to_string())
            }

            let _new_exchange = new_exchange.get_untracked();
            if _new_exchange.assistant_message.is_empty() {     // whether canceled before response
                prompt.set(_prompt);
            } else {
                exchanges.update(|exchanges| {
                    let max_key = exchanges.into_iter().map(|(key, _)| *key + 1).max().unwrap_or(0);
                    exchanges.push((max_key, create_rw_signal(_new_exchange)));
                    let exchanges = exchanges.iter()
                        .map(|(key, exchange)| (*key, exchange.get_untracked()))
                        .collect::<Vec<_>>();
                    spawn_local(set_exchanges(exchanges).map(drop));
                });
                new_exchange.set(Exchange::default());
            }

            streaming.set(false);
        });
    };

    let on_cancel = move || spawn_local(async move {
        if let Err(_) = emit("cancel", JsValue::null()).await {
            set_error("Unable to cancel stream.".into());
        }
    });

    view! {
        <Button class="mr-4 md:mr-8" label="New"
            to_hide=streaming.into() on_click=Box::new(move || set_conversation_uuid(None)) />
        <Button class="" label="Submit"
            to_hide=streaming.into() on_click=Box::new(on_submit) />
        <div class="flex ml-auto">
            <Button class="mr-4 md:mr-8" label="Cancel"
                to_hide=Signal::derive(move || !streaming()) on_click=Box::new(on_cancel) />
            <Button class="" label="Menu"
                to_hide=create_signal(false).0.into()
                on_click=Box::new(move || menu.set(Menu::Menu)) />
        </div>
    }
}

#[component]
pub fn Chat(config: RwSignal<Config>, menu: RwSignal<Menu>) -> impl IntoView {
    let error = signal_pair.0;
    let exchanges = create_rw_signal(Vec::<(usize, RwSignal<common::Exchange>)>::new());
    let new_exchange = create_rw_signal(Exchange::default());
    let prompt = create_rw_signal("".to_string());
    let streaming = create_rw_signal(false);

    create_effect(move |_| {
        let Some(uuid) = conversation_uuid() else {
            exchanges.set(vec![]);
            return;
        };

        spawn_local(async move {
            let new_exchanges = match load_exchanges(uuid).await {
                Ok(exchanges) => exchanges,
                Err(error) => {
                    set_error(error.to_string());
                    return;
                }
            };
            let key_to_exchange = exchanges.get_untracked().into_iter().collect::<HashMap<_, _>>();
            let synchronized_exchanges = new_exchanges.into_iter()
                .map(|(key, new_exchange)| (key, key_to_exchange.get(&key)
                    .map(|exchange| {
                        exchange.set(new_exchange.clone());
                        *exchange
                    })
                    .unwrap_or_else(|| create_rw_signal(new_exchange))))
                .collect();
            exchanges.set(synchronized_exchanges);
        });
    });

    let bottom_if_not_empty = move |classes: &str|
        format!("{} {}", classes, (exchanges().is_empty() && !streaming()).then(|| "mb-auto")
            .unwrap_or("mt-auto mb-4 md:mb-8"));

    view! {
        <div class="flex flex-col md:w-[80vw] md:mx-auto h-full p-4 md:py-[5vh] overflow-y-hidden"
                style:display=move || (menu.get() != Menu::Chat).then(|| "None")>
            <h1 class="hidden md:block mb-6 text-[2em] font-serif">"LLM Playground"</h1>
            <ErrorMessage error />
            <div class="mb-4 md:mx-[15vw] overflow-y-auto"
                    style:display=move || (exchanges().is_empty() && !streaming()).then(|| "None")>
                <Exchanges new_exchange exchanges streaming/>
            </div>
            <div class=move || bottom_if_not_empty("flex-none md:mx-[14.5vw] max-h-[50vh] overflow-y-auto")>
                <div class="flex flex-col">     // scrolling breaks without this useless div
                    <MessageBox id="prompt-box".into() rows=2 class="".into()
                        placeholder=Some("Enter a prompt here.".into())
                        content=prompt.into() set_content=prompt.into() />
                </div>
            </div>
            <div class="flex-none md:mx-[10vw] flex md:mx-8">
                <BottomButtons config menu exchanges new_exchange prompt streaming />
            </div>
        </div>
    }
}