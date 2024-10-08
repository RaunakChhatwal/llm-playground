use leptos::*;
use wasm_bindgen::{JsValue, prelude::*};

#[derive(Clone, Copy, PartialEq)]
pub enum Menu {
    Chat,
    Menu,
    History,
    Settings
}

#[component]
pub fn ErrorMessage(error: ReadSignal<String>) -> impl IntoView {
    view! {
        <p class="mb-2 text-red-400 text-[0.9em]"
            style:display=move || error().is_empty().then(|| "None")
        >{error}</p>
    }
}

pub fn button() -> String {
    " px-[9px] py-[3px] border border-[#33333A] bg-[#222222] hover:bg-[#2A2A2A] text-[#AAAABB] ".into()
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"])]
    pub async fn listen(
        cmd: &str,
        cb: &Closure<dyn Fn(JsValue)>
    ) -> Result<JsValue, JsValue>;
}

lazy_static::lazy_static! {
    pub static ref _conversation_uuid: std::sync::RwLock<RwSignal<Option<uuid::Uuid>>> =
        std::sync::RwLock::new(create_rw_signal(None));
}

pub fn conversation_uuid() -> Option<uuid::Uuid> {
    _conversation_uuid.read().ok()?.get()
}

pub fn get_conversation_uuid_untracked() -> Option<uuid::Uuid> {
    _conversation_uuid.read().ok()?.get_untracked()
}

pub fn set_conversation_uuid(uuid: Option<uuid::Uuid>) {
    if let Ok(conversation_uuid) = _conversation_uuid.read().as_mut() {
        conversation_uuid.set(uuid);
    }
}

pub fn set_conversation_uuid_untracked(uuid: Option<uuid::Uuid>) {
    if let Ok(conversation_uuid) = _conversation_uuid.read().as_mut() {
        conversation_uuid.set_untracked(uuid);
    }
}

pub fn update_textarea_height(textarea: &web_sys::HtmlTextAreaElement) {
    // textarea.set_attribute("style", "height: auto;").expect("Textareas support the style attribute");
    // let style = format!("height: {}px;", textarea.scroll_height());
    // textarea.set_attribute("style", &style).expect("Textareas support the style attribute");
    let _ = textarea.set_attribute("style", "height: auto;");
    let style = format!("height: {}px;", textarea.scroll_height());
    let _ = textarea.set_attribute("style", &style);
}