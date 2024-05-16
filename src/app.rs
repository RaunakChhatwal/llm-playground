use leptos::{leptos_dom::logging::console_log, *};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[component]
fn Button(class: &'static str, label: &'static str, on_click: Box<dyn Fn()>) -> impl IntoView {
    let class = format!("{class} px-2 py-1 box-border border-2 border-[#2A2A2A] hover:bg-inherit bg-[#2A2A2A] text-[#AAAABB]");
    view! {
        <button class={class} on:click=move |_| on_click()>{label}</button>
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

    // value=prompt entry in the view macro below does not work, for some reason
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
            class="w-full px-2 py-1 bg-[#222222] text-[0.9em] overflow-hidden resize-none"
            type="text"
            placeholder="Enter a prompt here."
            on:input=on_input
        ></textarea>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (prompt, set_prompt) = create_signal("".to_string());

    let on_submit = move || {
        console_log(&prompt());
        set_prompt("".to_string());
    };

    view! {
        <div class="flex flex-col h-full p-4 text-[0.9rem]">
            <div class="flex flex-col">
                <PromptBox prompt set_prompt />
            </div>
            <div class="flex mt-auto p-2">
                <Button class="mr-4" label="New" on_click=Box::new(|| ())/>
                <Button class="" label="Submit" on_click=Box::new(on_submit) />
                <Button class="ml-auto" label="Settings" on_click=Box::new(|| ())/>
            </div>
        </div>
    }
}
