use anyhow::{anyhow, Context, Result};
use futures::{Stream, StreamExt};
use leptos::{leptos_dom::logging::console_log, *};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;

use crate::{config::Config, fetch_tokens::fetch_tokens};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
fn Button(
    class: &'static str,
    label: &'static str,
    to_hide: ReadSignal<bool>,
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
fn PromptBox(prompt: ReadSignal<String>, set_prompt: WriteSignal<String>) -> impl IntoView {
    let on_input = move |event| {
        set_prompt(event_target_value(&event));
        let prompt_box = document().get_element_by_id("prompt-box")
            .expect("This element exists.");
        prompt_box.set_attribute("style", "height: auto;")
            .expect("The style attribute is available for every element.");
        prompt_box.set_attribute("style", &format!("height: {}px", prompt_box.scroll_height()))
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
            class="px-2 py-1 bg-[#222222] text-[0.9em] overflow-hidden resize-none"
            type="text"
            placeholder="Enter a prompt here."
            on:input=on_input
        ></textarea>
    }
}

async fn build_token_stream(
    prompt: &str,
    exchanges: Vec<(String, String)>
) -> Result<impl Stream<Item = Result<Option<String>>>> {
    let serialized_config = invoke("load_config",
        to_value(&serde_json::Value::Object(serde_json::Map::new()))
        .expect("The empty object should successfully serialize"))
        .await
        .as_string()
        .expect("load_config returns String");
    console_log(&format!("Config: {serialized_config:?}"));
    let config = serde_json::from_str::<Result<Config, String>>(&serialized_config)
        .context("Unable to parse config")?
        .map_err(|error_message| anyhow!("{error_message}"))?;
    
   return fetch_tokens(&prompt, &config, exchanges);
}

#[component]
pub fn App() -> impl IntoView {
    let (error, set_error) = create_signal("".to_string());
    let (prompt, set_prompt) = create_signal("".to_string());
    let (response, set_response) = create_signal("".to_string());
    let (streaming, set_streaming) = create_signal(false);

    let on_submit = move || {
        set_streaming(true);
        set_error("".to_string());
        set_response("".to_string());
        let prompt = prompt();
        spawn_local(async move {
            let mut token_stream;
            match build_token_stream(&prompt, vec![]).await {
                Ok(stream) => token_stream = stream,
                Err(error) => {
                    set_error(error.to_string());
                    return;
                }
            }

            while let Some(token) = token_stream.next().await {
                match token {
                    Ok(Some(token)) => set_response.update(|response|
                        response.push_str(&token)),
                    Ok(None) => break,
                    Err(error) => {
                        set_error(error.to_string());
                        break;
                    }
                }
            }

            set_streaming(false);
        });
    };

    view! {
        <div class="flex flex-col h-full p-4 text-[0.9rem]">
            <div class="flex flex-col">
                <p
                    class="mb-2 text-red-400"
                    style:display=move || error().is_empty().then(|| "None")
                >{error}</p>
                <PromptBox prompt set_prompt />
                <p
                    class="mt-2 px-2 py-1 min-h-6 bg-[#222222] text-[0.9em]"
                    style:display=move || (!streaming() && response().is_empty()).then(|| "None")
                >{response}</p>
            </div>
            <div class="flex mt-auto p-2">
                <Button class="mr-4" label="New" to_hide=streaming on_click=Box::new(|| ())/>
                <Button class="" label="Submit" to_hide=streaming on_click=Box::new(on_submit) />
                <Button class="ml-auto" label="Settings" to_hide=streaming on_click=Box::new(|| ())/>
            </div>
        </div>
    }
}
