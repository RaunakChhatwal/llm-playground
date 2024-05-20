use leptos::*;
use gloo_utils::format::JsValueSerdeExt;
use wasm_bindgen::prelude::*;
use crate::{common::{invoke, load_config, Button, ErrorMessage, Menu}, util::Config};

#[component]
fn TemperatureSlider(config: RwSignal<Option<Config>>) -> impl IntoView {
    let on_input = move |event| {
        let temperature = event_target_value(&event).parse::<f64>()
            .expect("The slider only permits numbers.");
        config.update(|config| {
            config.as_mut().map(|config| config.temperature = temperature);
        });
    };

    create_effect(move |_| {
        let input = document().get_element_by_id("temperature-slider")
            .expect("This element exists.")
            .dyn_into::<web_sys::HtmlInputElement>()
            .expect("This is an input element.");

        if let Some(config) = config() {
            let temperature = config.temperature.to_string();
            if input.value() != temperature {
                input.set_value(&temperature.to_string());
            }
        }
    });

    view! {
        <label>"Temperature:"</label>
        <div class="flex items-center">
            <input class="accent-blue-900" id="temperature-slider" type="range"
               min="0" max="1" step="0.1"
               on:input=on_input />
            {move || config().map(|config| view! {
                <span class="mx-2">"|"</span>
                <span>{config.temperature.to_string()}</span>
            })}
        </div>
    }
}

#[component]
fn MaxTokensSlider(config: RwSignal<Option<Config>>) -> impl IntoView {
    let on_input = move |event| {
        let max_tokens = event_target_value(&event).parse::<u32>()
            .expect("The slider has integer step.");
        config.update(|config| {
            config.as_mut().map(|config| config.max_tokens = max_tokens);
        });
    };

    create_effect(move |_| {
        let input = document().get_element_by_id("max-tokens-slider")
            .expect("This element exists.")
            .dyn_into::<web_sys::HtmlInputElement>()
            .expect("This is an input element.");

        if let Some(config) = config() {
            let max_tokens = config.max_tokens.to_string();
            if input.value() != max_tokens {
                input.set_value(&max_tokens);
            }
        }
    });

    view! {
        <label>"Max tokens:"</label>
        <div class="flex items-center">
            <input class="accent-blue-900" id="max-tokens-slider" type="range"
               min="0" max="4096" step="1"
               on:input=on_input />
            {move || config().map(|config| view! {
                <span class="mx-2">"|"</span>
                <span>{config.max_tokens.to_string()}</span>
            })}
        </div>
    }
}

#[component]
fn ModelInput(config: RwSignal<Option<Config>>) -> impl IntoView {
    let on_input = move |event|
        config.update(|config| {
            config.as_mut().map(|config|
                config.model = event_target_value(&event));});

    create_effect(move |_| {
        let input = document().get_element_by_id("model-input")
            .expect("This element exists.")
            .dyn_into::<web_sys::HtmlInputElement>()
            .expect("This is an input element.");

        if let Some(config) = config() {
            if input.value() != config.model {
                input.set_value(&config.model);
            }
        }
    });

    view! {
        <label>"Model:"</label>
        <input class="px-2 py-1 bg-[#222222] border-2 border-[#2A2A2A] text-[0.9em]"
            id="model-input"
            type="text"
            on:input=on_input />
    }
}

// #[component]
// fn KeyMenu(config: RwSignal<Option<Config>>) -> impl InfoView {
//     todo!()
// }

#[component]
pub fn Settings(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    let (error, set_error) = create_signal("".to_string());
    let config = create_rw_signal(None);
    let (saved_config, set_saved_config) = create_signal(None);

    spawn_local(async move {
        match load_config().await {
            Ok(loaded_config) => {
                config.set(Some(loaded_config.clone()));
                set_saved_config(Some(loaded_config));
            },
            Err(error) => set_error(error.to_string())
        }
    });

    let to_hide = Signal::derive(move ||
        config().map_or(false, |config|
            saved_config().map_or(false, |saved_config| config == saved_config)));

    let on_discard = move || config.set(saved_config.get_untracked());

    let on_save = move || spawn_local(async move {
        let Some(config) = config.get_untracked() else { return };
        let config_JsValue = JsValue::from_serde(&serde_json::json!({
            "config": config
        })).expect("Serializing Config should always succeed");
        let error = invoke("save_config", config_JsValue).await;
        if error.is_null() {
            set_saved_config(Some(config));
        } else {
            let error_message = error.as_string().expect("save_config returns Option<String>");
            set_error(error_message);
        }
    });

    view! {
        <div
            class="flex flex-col items-center h-full p-4 overflow-y-hidden text-[0.9rem]"
            style:display=move || (menu.get() != Menu::Settings).then(|| "None")
        >
            <Button class="mr-auto" label="Back"
                to_hide=create_signal(false).0.into()
                on_click=Box::new(move || set_menu(Menu::Chat))
            />
            <h1 class="text-[1.25em]">"Settings"</h1>
            <ErrorMessage error />
            <div class="grid grid-cols-[repeat(2,max-content)] gap-4 items-center my-auto overflow-y-auto">
                <TemperatureSlider config />
                <MaxTokensSlider config />
                <ModelInput config />
            </div>
            <div class="flex justify-end w-full">
                <Button class="mr-4" label="Discard" to_hide on_click=Box::new(on_discard) />
                <Button class="" label="Apply" to_hide on_click=Box::new(on_save) />
            </div>
        </div>
    }
}