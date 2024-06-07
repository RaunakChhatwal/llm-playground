use std::sync::Mutex;
use anyhow::{anyhow, bail, Result};
use futures::{channel::mpsc::UnboundedReceiver, FutureExt, SinkExt, StreamExt};
use gloo_utils::format::JsValueSerdeExt;
use leptos::*;
use wasm_bindgen::prelude::*;
use crate::common::{invoke, load_config, Button, ErrorMessage, Menu};
use crate::util::Exchange;

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

    let class = format!("{} flex-none w-full min-h-[2em] px-2 pt-1 pb-2 border-2 border-[#303038]
        bg-[#222222] text-[0.9em] overflow-hidden resize-none", class);
    view! {
        <textarea id=id rows=rows class=class type="text"
            placeholder=placeholder on:input=on_input
        ></textarea>
    }
}

#[component]
fn ExchangeComponent(
    id: usize,
    exchange: RwSignal<Exchange>,
    set_exchanges: WriteSignal<Vec<(usize, RwSignal<Exchange>)>>
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
        set_exchanges.update(|exchanges|
            exchanges.retain(|(exchange_id, _)| id != *exchange_id))};

    view! {
        <div class="relative flex flex-col">
            <button
                class="absolute top-[-10px] right-[10px] text-[1.5rem] text-[#AAAABB]"
                on:click=move |_| on_delete()
            >"-"</button>
            <MessageBox id=format!("message-box-{}", 2*id) rows=1
                class="".into()
                placeholder=None
                content=user_message set_content=set_user_message />
            <MessageBox id=format!("message-box-{}", 2*id + 1)
                rows=1 placeholder=None
                class="mt-[12px]".into() content=assistant_message
                set_content=set_assistant_message />
        </div>
    }
}

#[component]
fn Exchanges(
    exchanges: ReadSignal<Vec<(usize, RwSignal<Exchange>)>>,
    set_exchanges: WriteSignal<Vec<(usize, RwSignal<Exchange>)>>
) -> impl IntoView {
    view! {
        <div class="flex flex-col">
            <For each=exchanges
                key=|exchange| exchange.0
                children=move |(id, exchange)| view! {
                    <div style:margin-top=move || (id != exchanges()[0].0).then(|| "12px")>
                        <ExchangeComponent id exchange set_exchanges />
                    </div>
                } />
        </div>
    }
}

async fn build_token_stream(prompt: String, exchanges: Vec<Exchange>)
-> Result<UnboundedReceiver<Result<String, String>>> {
    let args = JsValue::from_serde(&serde_json::json!({
        "prompt": prompt,
        "config": load_config().await?,
        "exchanges": exchanges
    })).map_err(|_| anyhow!("Error serializing fetch_token arguments"))?;
    let error = invoke("_build_token_stream", args).await;
    if !error.is_null() {
        bail!("{}", error.as_string().unwrap_or("Request failed.".into()));
    }

    let (mut sender, recv) = futures::channel::mpsc::unbounded();

    spawn_local(async move {
        loop {
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
        }
    });

    return Ok(recv);
}

fn fn_mut_to_fn(f: Mutex::<Box<dyn FnMut()>>) -> Box<dyn Fn()> {
    return Box::new(move || match f.try_lock() {
        Ok(mut f) => f(),
        Err(_) => return
    });
}

async fn collect_tokens(
    mut token_stream: UnboundedReceiver<Result<String, String>>,
    set_exchange: WriteSignal<Exchange>,
    set_error: WriteSignal<String>
) {
    while let Some(token) = token_stream.next().await {
        match token {
            Ok(token) => set_exchange.update(|exchange|
                exchange.assistant_message.push_str(&token)),
            Err(error) => {
                set_error(error.to_string());
                break;
            }
        }
    }
}

#[component]
pub fn Chat(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    let (error, set_error) = create_signal("".to_string());
    let counter = create_rw_signal(0);
    let (exchanges, set_exchanges) = create_signal(Vec::<(usize, RwSignal<Exchange>)>::new());
    let (new_exchange, set_new_exchange) = create_signal(Exchange::default());
    let (prompt, set_prompt) = create_signal("".to_string());
    let (streaming, set_streaming) = create_signal(false);

    let on_new = move || {
        counter.set(0);
        set_exchanges(Vec::new());
    };

    // casting the closure to FnMut because on_submit isn't logically reentrant
    let on_submit = Mutex::<Box<dyn FnMut()>>::new(Box::new(move || {
        set_streaming(true);
        set_error("".to_string());
        let prompt = prompt();
        set_prompt("".to_string());
        let exchanges = exchanges.get_untracked()
            .iter()
            .map(|(_, exchange)|
                exchange())
            .collect::<Vec<Exchange>>();

        set_new_exchange(Exchange {
            user_message: prompt.clone(),
            assistant_message: "".to_string()
        });

        spawn_local(async move {
            match build_token_stream(prompt.clone(), exchanges).await {
                Ok(token_stream) => collect_tokens(token_stream, set_new_exchange, set_error).await,
                Err(error) => set_error(error.to_string())
            }

            let new_exchange = new_exchange.get_untracked();
            if !new_exchange.assistant_message.is_empty() {     // whether canceled before response
                set_exchanges.update(|exchanges|
                    exchanges.push((counter.get_untracked(), create_rw_signal(new_exchange))));
                counter.update(|counter| *counter += 1);
                set_new_exchange(Exchange::default());
            } else {
                set_prompt(prompt);
            }

            set_streaming(false);
        });
    }));

    view! {
        <div class="flex flex-col md:w-[80vw] md:mx-auto h-full p-4 md:py-[5vh] overflow-y-hidden"
                style:display=move || (menu.get() != Menu::Chat).then(|| "None")>
            // <h1 class="hidden md:block ml-12 mb-6 text-[2em] font-serif">"LLM Playground"</h1>
            <h1 class="hidden md:block mb-6 text-[2em] font-serif">"LLM Playground"</h1>
            <ErrorMessage error />
            <div class="mb-4 md:mx-[15vw] overflow-y-auto"
                    style:display=move || (exchanges().is_empty() && !streaming()).then(|| "None")>
                <Exchanges exchanges set_exchanges />
                <p class="px-2 py-1 min-h-[2em] bg-[#222222] border-2 border-[#303038] text-[0.9em]"
                    style:margin-top=move || (!exchanges().is_empty()).then(|| "12px")
                    style:display=move || (!streaming()).then(|| "None")
                >{move || new_exchange().user_message}</p>
                <p class="mt-[12px] px-2 py-1 min-h-[2em] bg-[#222222] border-2 border-[#303038] text-[0.9em]"
                    style:display=move || (!streaming()).then(|| "None")
                >{move || new_exchange().assistant_message}</p>        
            </div>
            <div class=move || format!("flex-none {} md:mx-[14.5vw] max-h-[50vh] overflow-y-auto",
                    (exchanges().is_empty() && !streaming())
                        .then(|| "mb-auto")
                        .unwrap_or("mt-auto mb-4 md:mb-8"))>
                <div class="flex flex-col">     // scrolling breaks without this useless div
                    <MessageBox id="prompt-box".into() rows=2 class="".into()
                        placeholder=Some("Enter a prompt here.".into())
                        content=prompt.into() set_content=set_prompt.into() />
                </div>
            </div>
            <div class="flex-none md:mx-[10vw] flex md:mx-8">
                <Button class="mr-4 md:mr-8" label="New"
                    to_hide=streaming.into() on_click=Box::new(on_new) />
                <Button class="" label="Submit"
                    to_hide=streaming.into() on_click=fn_mut_to_fn(on_submit) />
                <div class="flex ml-auto">
                    <Button class="mr-4 md:mr-8" label="Cancel"
                        to_hide=Signal::derive(move || !streaming()) on_click=Box::new(||
                            spawn_local(invoke("cancel", JsValue::null()).map(|_| ()))) />
                    <Button class="" label="Menu"
                        to_hide=create_signal(false).0.into()
                        on_click=Box::new(move || set_menu(Menu::Menu)) />
                </div>
            </div>
        </div>
    }
}