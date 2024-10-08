use std::{collections::HashMap, sync::Arc, time::Duration};
use anyhow::{anyhow, bail, Result};
use common::{Config, Exchange};
use futures::{FutureExt, join, stream, Stream};
use gloo_utils::format::JsValueSerdeExt;
use leptos::{*, leptos_dom::log};
use tokio_stream::{StreamExt, wrappers::UnboundedReceiverStream};
use wasm_bindgen::{JsValue, prelude::*};
use crate::commands::{add_conversation, delete_conversation, load_exchanges};
use crate::util::{button, conversation_uuid, get_conversation_uuid_untracked, listen, update_textarea_height};
use crate::util::{set_conversation_uuid, set_conversation_uuid_untracked, ErrorMessage, Menu};

lazy_static::lazy_static! {
    // anyhow! macro doesn't work if there is a static variable named "error" in the namespace
    pub static ref signal_pair: (ReadSignal<String>, WriteSignal<String>) = create_signal("".into());
    pub static ref set_error: WriteSignal<String> = signal_pair.1;
}

async fn sleep(duration: Duration) {
    let (send, recv) = tokio::sync::oneshot::channel();

    set_timeout(move || {
        let _ = send.send(());
    }, duration);

    recv.await.unwrap_or_else(|error| log!("Unable to sleep: {error}"));
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
    let class = format!("{} flex-none w-full min-h-[2em] px-2 pt-1 pb-2 border border-[#303038]
        bg-[#222222] text-[0.9em] overflow-hidden resize-none", class);
    let message_box = view! {
        <textarea id=id.clone() rows=rows class=class type="text" placeholder=placeholder></textarea>
    };

    let on_input = Closure::<dyn Fn(web_sys::Event) + 'static>::new({
        let message_box = message_box.clone();
        move |event| {
            set_content(event_target_value(&event));
            update_textarea_height(&message_box);
        }
    });
    message_box.set_oninput(Some(on_input.as_ref().unchecked_ref()));
    std::mem::forget(on_input);

    // this is because value=content entry in the view macro below does not work
    create_effect({
        let message_box = message_box.clone();
        move |_| content.with(|message| {
            // this is different from setting the textarea's value html attribute, which will not work
            message_box.set_value(&message);
            update_textarea_height(&message_box);
        })
    });

    return message_box;
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
        |exchange| exchange.assistant_message.trim().to_string(),
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
            <button on:click=move |_| on_delete()
                class="absolute top-[-10px] right-[10px] text-[1.5rem] text-[#AAAABB]"
            >"-"</button>
            <MessageBox id=format!("message-box-{}", 2*key) rows=1 class="".into()
                placeholder=None content=user_message set_content=set_user_message />
            <MessageBox id=format!("message-box-{}", 2*key + 1) rows=1 placeholder=None
                class="mt-[12px]".into() content=assistant_message set_content=set_assistant_message />
        </div>
    }
}

fn get_message_box_by_id(id: usize) -> Result<web_sys::HtmlTextAreaElement> {
    document().get_element_by_id(&format!("message-box-{id}"))
        .ok_or(anyhow!("Element with id {id} not found"))?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .map_err(|_| anyhow!("Element with id {id} not a text area element"))
}

#[component]
fn Exchanges(
    new_exchange: RwSignal<Exchange>,
    exchanges: RwSignal<Vec<(usize, RwSignal<Exchange>)>>,
    update_heights: Arc<tokio::sync::Notify>,
    response_textbox: HtmlElement<html::P>,
    streaming: RwSignal<bool>
) -> impl IntoView {
    let on_resize = Closure::<dyn Fn() + 'static>::new({
        let update_heights = Arc::clone(&update_heights);
        move || update_heights.notify_one()
    });

    spawn_local(async move {
        loop {
            join!(update_heights.notified(), sleep(Duration::from_millis(250)));
            exchanges.with_untracked(|exchanges| exchanges.iter()
                .flat_map(|(key, _)| vec![2*key, 2*key + 1])
                .map(|id| Ok(update_textarea_height(&get_message_box_by_id(id)?)))
                .collect::<Result<()>>()
            ).unwrap_or_else(|error| log!("Unable to update message box sizes: {error}"));
        }
    });

    window().set_onresize(Some(on_resize.as_ref().unchecked_ref()));
    std::mem::forget(on_resize);

    let margin_top = move |key| exchanges().get(0).and_then(|(_key, _)| (key != *_key).then(|| "12px"));
    view! {
        <div class="flex flex-col">
            <For each=exchanges
                key=|(key, _)| *key
                children=move |(key, exchange)| view! {
                    <div style:margin-top=move || margin_top(key)>
                        <ExchangeComponent key exchange exchanges />
                    </div>
                } />
        </div>
        <p class="px-2 py-1 min-h-[2em] bg-[#222222] border border-[#303038] text-[0.9em]"
            style:margin-top=move || (!exchanges().is_empty()).then(|| "12px")
            style:display=move || (!streaming()).then(|| "None")
        >{move || new_exchange().user_message}</p>
        {response_textbox}
    }
}

fn deserialize_event(event: JsValue) -> Result<Option<String>> {
    let mut parsed_event = JsValue::into_serde::<serde_json::Map<String, serde_json::Value>>(&event)?;
    let Some(serde_json::Value::Object(mut payload)) = parsed_event.remove("payload") else {
        bail!("Unable to deserialize token.");
    };

    if let Some(token) = payload.remove("Ok") {
        if token.is_null() {
            return Ok(None);    // signals end of response
        }

        if let Some(token) = token.as_str() {
            return Ok(Some(token.into()));
        }
    } else if let Some(error) = payload.remove("Err") {
        if let Ok(error) = serde_json::from_value::<serde_error::Error>(error) {
            return Err(error.into());
        }
    }

    bail!("Unable to deserialize token.");
}

async fn build_token_stream(prompt: &str, config: Config, exchanges: Vec<Exchange>)
-> Result<Box<dyn Stream<Item = Result<String>> + Unpin>> {
    let canceled = crate::commands::build_token_stream(prompt, config, exchanges).await?;
    if canceled {
        // the cancel button was clicked before the token stream could be built
        return Ok(Box::new(stream::empty()));
    }

    let (sender, recv) = tokio::sync::mpsc::unbounded_channel();
    let close = std::sync::Arc::new(tokio::sync::Notify::new());

    let on_token = {
        let close = close.clone();
        Closure::new(move |event: JsValue| {
            match deserialize_event(event) {
                Ok(Some(token)) => drop(sender.send(Ok(token))),
                Ok(None) => close.notify_one(),
                Err(error) => drop(sender.send(Err(error)))
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
        drop(on_token);     // move on_tokens into this closure to keep it alive
    });

    return Ok(Box::new(UnboundedReceiverStream::new(recv)));
}

const TOLERANCE: i32 = 5;
fn is_scrollbar_bottom(exchanges_div: &web_sys::HtmlDivElement) -> bool {
    let height_hidden = exchanges_div.scroll_height() - exchanges_div.client_height();
    return (height_hidden - exchanges_div.scroll_top()).abs() < TOLERANCE;
} 

// the bezier defined by (3/4, 3/4), (1, 1), and (5/4, 1), not parameterized
fn deceleration_bezier(x: f64) -> f64 {
    -x*x + 2.5*x - 0.5625
}

// the percentage of visible height taken up by the response text box
fn calculate_visibility(
    exchanges_div: &web_sys::HtmlDivElement,
    response_textbox: &web_sys::HtmlParagraphElement
) -> f64 {
    let mut visible_height = exchanges_div.scroll_top() + exchanges_div.client_height();
    visible_height -= exchanges_div.scroll_height() - response_textbox.client_height();
    (visible_height as f64)/(exchanges_div.client_height() as f64)
}

async fn collect_tokens(
    exchange: RwSignal<Exchange>,
    exchanges_div: &web_sys::HtmlDivElement,
    response_textbox: &web_sys::HtmlParagraphElement,
    mut token_stream: impl Stream<Item = Result<String>> + Unpin,
) {
    let mut visibility = calculate_visibility(exchanges_div, response_textbox);
    while let Some(token) = token_stream.next().await {
        let token = match token {
            Ok(token) => token,
            Err(error) => {
                set_error(error.to_string());
                break;
            }
        };
        let is_scrollbar_bottom = is_scrollbar_bottom(&exchanges_div);
        // detatch if the current visibility isn't what it last was - i.e. if the user scrolls off
        let autoscroll = approx::AbsDiffEq::abs_diff_eq(&visibility,
            &calculate_visibility(exchanges_div, response_textbox),
            (TOLERANCE as f64)/(exchanges_div.client_height() as f64)
        );

        exchange.update(|exchange| exchange.assistant_message.push_str(&token));

        let x = (response_textbox.scroll_height() as f64)/(exchanges_div.client_height() as f64);
        if x < 0.75 {
            visibility = x;
            if autoscroll {
                // autoscroll normally
                let scroll_top = exchanges_div.scroll_height() - exchanges_div.client_height();
                exchanges_div.set_scroll_top(scroll_top);
            }
        } else if x < 1.25 && autoscroll {
            // autoscroll with deceleration
            let response_scroll_top =
                exchanges_div.scroll_height() - response_textbox.client_height();
            let offset = (exchanges_div.client_height() as f64)*(1.0 - deceleration_bezier(x));
            let scroll_top = i32::clamp(response_scroll_top - offset.round() as i32, 0,
                exchanges_div.scroll_height() - exchanges_div.client_height());
            exchanges_div.set_scroll_top(scroll_top);
            visibility = calculate_visibility(exchanges_div, response_textbox);
        } else if is_scrollbar_bottom {
            // autoscroll normally
            let scroll_top = exchanges_div.scroll_height() - exchanges_div.client_height();
            exchanges_div.set_scroll_top(scroll_top);
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"])]
    async fn emit(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

// update this conversation's exchanges in the conversation history database
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
fn Buttons(
    config: RwSignal<Config>,
    exchanges: RwSignal<Vec<(usize, RwSignal<Exchange>)>>,
    exchanges_div: HtmlElement<html::Div>,
    menu: RwSignal<Menu>,
    new_exchange: RwSignal<Exchange>,
    prompt: RwSignal<String>,
    response_textbox: HtmlElement<html::P>,
    streaming: RwSignal<bool>,
) -> impl IntoView {
    let exchanges_div = std::rc::Rc::new(exchanges_div);
    let response_textbox = std::rc::Rc::new(response_textbox);

    let on_submit = move |_| {
        let is_scrollbar_bottom = is_scrollbar_bottom(&exchanges_div);
        // let is_scrollbar_bottom = (height_hidden - exchanges_div.scroll_top()).abs() < TOLERANCE;

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

        if is_scrollbar_bottom {
            exchanges_div.set_scroll_top(exchanges_div.scroll_height() - exchanges_div.client_height());
        }

        let exchanges_div = exchanges_div.clone();
        let response_textbox = response_textbox.clone();
        spawn_local(async move {
            match build_token_stream(&_prompt, config.get_untracked(), _exchanges).await {
                Ok(token_stream) => collect_tokens(
                    new_exchange,
                    exchanges_div.as_ref(),
                    response_textbox.as_ref(),
                    token_stream
                ).await,
                Err(error) => set_error(error.to_string())
            }

            let scroll_top = exchanges_div.scroll_top();

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
                    // update this conversation's exchanges to the database
                    spawn_local(set_exchanges(exchanges));
                });
                new_exchange.set(Exchange::default());
            }

            streaming.set(false);

            sleep(Duration::from_millis(25)).await;     // don't know why this is necessary
            exchanges_div.set_scroll_top(scroll_top);
        });
    };

    let on_cancel = move |_| spawn_local(async move {
        if let Err(_) = emit("cancel", JsValue::null()).await {
            set_error("Unable to cancel stream.".into());
        }
    });

    view! {
        <button class=button() + "mr-4 md:mr-8"
            on:click=move |_| set_conversation_uuid(None)
            style:display=move || streaming().then(|| "None")
        >"New"</button>
        <button class=button() on:click=on_submit
            style:display=move || streaming().then(|| "None")
        >"Submit"</button>
        <div class="flex ml-auto">
            <button class=button() + "mr-4 md:mr-8" on:click=on_cancel
                style:display=move || (!streaming()).then(|| "None")
            >"Cancel"</button>
            <button class=button() on:click=move |_| menu.set(Menu::Menu)>"Menu"</button>
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

    let update_heights = Arc::new(tokio::sync::Notify::new());
    create_effect({
        let update_heights = Arc::clone(&update_heights);
        move |_| matches!(menu(), Menu::Chat).then(|| update_heights.notify_one())
    });

    let bottom_if_not_empty = move |classes: &str|
        format!("{} {}", classes, (exchanges().is_empty() && !streaming()).then(|| "mb-auto")
            .unwrap_or("mt-auto mb-4 md:mb-8"));

    let response_textbox = view! {
        <p style:display=move || (!streaming()).then(|| "None")
            class="mt-[12px] px-2 py-1 min-h-[2em] bg-[#222222] border border-[#303038] text-[0.9em]"
        >{move || new_exchange().assistant_message}</p>
    };

    let exchanges_div = view! {
        <div id="exchanges" class="mb-4 md:mx-[15vw] overflow-y-auto"
                style:display=move || (exchanges().is_empty() && !streaming()).then(|| "None")>
            <Exchanges new_exchange exchanges update_heights
                response_textbox=response_textbox.clone() streaming />
        </div>
    };

    view! {
        <div class="flex flex-col md:w-[80vw] md:mx-auto h-full p-4 md:py-[5vh] overflow-y-hidden"
                style:display=move || (menu.get() != Menu::Chat).then(|| "None")>
            <h1 class="hidden md:block mb-6 text-[2em] font-serif">"LLM Playground"</h1>
            <ErrorMessage error />
            {exchanges_div.clone()}
            <div class=move || bottom_if_not_empty("flex-none md:mx-[14.5vw] max-h-[50vh] overflow-y-auto")>
                <div class="flex flex-col">     // scrolling breaks without this useless div
                    <MessageBox id="prompt-box".into() rows=2 class="".into()
                        placeholder=Some("Enter a prompt here.".into())
                        content=prompt.into() set_content=prompt.into() />
                </div>
            </div>
            <div class="flex-none md:mx-[10vw] flex md:mx-8">
                <Buttons config exchanges exchanges_div menu new_exchange prompt response_textbox streaming />
            </div>
        </div>
    }
}