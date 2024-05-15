use leptos::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
fn Button(class: &'static str, label: &'static str) -> impl IntoView {
    let class = format!("{class} px-2 py-1 border border-gray-300 hover:bg-[#333333]");
    view! {
        <button class={class} >{label}</button>
    }
}

#[component]
fn PromptBox() -> impl IntoView {
    let prompt = create_rw_signal(String::new());

    let on_input = move |_| {
        let prompt_box = document().get_element_by_id("prompt-box")
            .expect("The current element should always exist.");
        prompt_box.set_attribute("style", "height: auto;")
            .expect("The style attribute is available for every element.");
        prompt_box.set_attribute("style", &format!("height: {}px", prompt_box.scroll_height()))
            .expect("The style attribute is available for every element.");
    };

    view! {
        <textarea
            id="prompt-box"
            class="w-full px-2 py-1 bg-[#222222] text-[0.9em] overflow-hidden resize-none"
            type="text"
            placeholder="Enter a prompt here."
            on:input=on_input
        >{prompt}</textarea>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! {
        <div class="flex flex-col h-full p-4 text-[0.9rem]">
            <div class="flex flex-col">
                <PromptBox />
            </div>
            <div class="flex mt-auto p-2">
                <Button class="mr-4" label="New" />
                <Button class="" label="Submit" />
                <Button class="ml-auto" label="Settings" />
            </div>
        </div>
    }
}
