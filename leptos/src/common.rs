use anyhow::{anyhow, Context, Result};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use leptos::*;
use crate::util::Config;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Clone, Copy, PartialEq)]
pub enum Menu {
    Chat,
    Settings
}

#[component]
pub fn ErrorMessage(error: ReadSignal<String>) -> impl IntoView {
    view! {
        <p
            class="mb-2 text-red-400 text-[0.9em]"
            style:display=move || error().is_empty().then(|| "None")
        >{error}</p>
    }
}

#[component]
pub fn Button(
    class: &'static str,
    label: &'static str,
    to_hide: Signal<bool>,
    on_click: Box<dyn Fn()>)
-> impl IntoView {
    let class = format!("{class}
        px-[9px] py-[3px] border-2 border-[#33333A] bg-[#222222] hover:bg-[#2A2A2A] text-[#AAAABB]");

    view! {
        <button
            class={class}
            on:click=move |_| on_click()
            style:display=move || to_hide().then(|| "None")
        >{label}</button>
    }
}

pub async fn load_config() -> Result<Config> {
    let serialized_config = invoke("_load_config",
        to_value(&serde_json::Value::Object(serde_json::Map::new()))
        .expect("The empty object should successfully serialize"))
        .await
        .as_string()
        .expect("load_config returns String");

    return serde_json::from_str::<Result<Config, String>>(&serialized_config)
        .context("Unable to parse config")?
        .map_err(|error_message| anyhow!("{error_message}"));
}