use std::str::FromStr;
use leptos::*;
use gloo_utils::format::JsValueSerdeExt;
use strum::VariantNames;
use wasm_bindgen::prelude::*;
use crate::common::{invoke, load_config, Button, ErrorMessage, Menu};
use crate::util::{APIKey, Config, Provider};

#[component]
fn TemperatureSlider(
    config: RwSignal<Option<Config>>,
    set_error: WriteSignal<String>)
-> impl IntoView {
    let on_input = move |event| {
        let Ok(temperature) = event_target_value(&event).parse::<f64>() else {
            set_error("The slider should only permit numbers.".into());
            return;
        };
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
                // this is different from setting the input's value html attribute, which will not work
                input.set_value(&temperature);
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
fn MaxTokensSlider(
    config: RwSignal<Option<Config>>,
    set_error: WriteSignal<String>
) -> impl IntoView {
    let on_input = move |event| {
        let Ok(max_tokens) = event_target_value(&event).parse::<u32>() else {
            set_error("The slider should only permit integers due to its integer step.".into());
            return;
        };
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
                // this is different from setting the input's value html attribute, which will not work
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
        <input id="model-input" type="text" on:input=on_input
            class="px-2 py-1 bg-[#222222] border-2 border-[#33333A] text-[0.9em]" />
    }
}

#[component]
fn KeyEntry(
    api_key: APIKey,
    selected_key: Signal<Option<String>>,
    on_remove: Box<dyn Fn(&str)>
) -> impl IntoView {
    let name = api_key.name.clone();
    create_effect(move |_| {
        let input = document().get_element_by_id(&format!("key-name-{}", name))
            .expect("This element exists.")
            .dyn_into::<web_sys::HtmlInputElement>()
            .expect("This is an input element.");

        // this is different from setting the input's checked html attribute, which will not work
        input.set_checked(selected_key().as_ref() == Some(&name));
    });

    view! {
        <input type="radio" value=api_key.name.clone() name="key_name"
            id=format!("key-name-{}", api_key.name.clone()) />
        <p class="mx-2">{api_key.name.clone()}</p>
        <button class="px-[5px] w-[max-content] h-[max-content] border-2 border-[#33333A]
                bg-[#222222] hover:bg-[#33333A] text-[#AAAABB]"
            on:click=move |_| on_remove(&api_key.name)
        >"-"</button>
    }
}

#[component]
fn KeyInput(new_key: RwSignal<Option<APIKey>>, set_error: WriteSignal<String>) -> impl IntoView {
    create_effect(move |_| {
        let new_provider = new_key().map(|new_key| new_key.provider.to_string());
        for &provider in Provider::VARIANTS {
            let input = document().get_element_by_id(&format!("provider-{}", provider))
                .and_then(|element|
                    element.dyn_into::<web_sys::HtmlInputElement>().ok());
            let Some(input) = input else {
                set_error(format!("Expected checkbox entry for {provider}"));
                continue;
            };

            // this is different from setting the input's checked html attribute, which will not work
            input.set_checked(Some(provider.to_string()) == new_provider);
        }
    });

    let on_change = move |event| new_key.update(|new_key| {
        new_key.as_mut().map(|new_key|
            new_key.provider = Provider::from_str(&event_target_value(&event)).unwrap_or_default());
    });

    view! {
        <div class="grid grid-cols-[repeat(2,max-content)] gap-2 text-[0.9em]"
            on:change=on_change
            style:display=move || new_key().is_none().then(|| "None")
        >
            <label>"Name:"</label>
            <input type="text" class="px-1 bg-[#222222] h-[2em] border-2 border-[#33333A] text-[0.9em]"
                on:input=move |event| new_key.update(|new_key| {
                    new_key.as_mut().map(|new_key|
                        new_key.name = event_target_value(&event));
                }) />
            <label>"Key:"</label>
            <input type="text" class="px-1 bg-[#222222] h-[2em] border-2 border-[#33333A] text-[0.9em]"
                on:input=move |event| new_key.update(|new_key| {
                    new_key.as_mut().map(|new_key|
                        new_key.key = event_target_value(&event));
                }) />
            <label>"Provider:"</label>
            <div class="grid grid-cols-[repeat(2,max-content)] items-center">
                <For
                    each=move || Provider::VARIANTS
                    key=|&provider_name| provider_name
                    children=|&provider_name| view! {
                        <input type="radio" value=provider_name name="provider"
                            id=format!("provider-{provider_name}") />
                        <label class="ml-2">{provider_name}</label>
                    }
                />
            </div>
        </div>
    }
}

#[component]
fn KeyList(config: RwSignal<Option<Config>>, set_error: WriteSignal<String>) -> impl IntoView {
    let (api_keys, set_api_keys) = create_slice(
        config,
        |config| config.as_ref().map_or(Vec::new(), |config| config.api_keys.clone()),
        |config, api_keys| {
            config.as_mut().map(|config| config.api_keys = api_keys);
        }
    );
    let (selected_key, set_selected_key) = create_slice(
        config,
        |config| config.as_ref().and_then(|config| Some(config.api_keys.get(config.api_key?)?.name.clone())),
        |config, selected_key: Option<String>| {
            config.as_mut().map(|config|
                config.api_key = selected_key.and_then(|selected_key|
                    config.api_keys.iter().position(|api_key| api_key.name == selected_key)));
        }
    );
    let new_key = create_rw_signal(None::<APIKey>);

    let on_remove = move |name: &str| {
        config.update(|config| {
            config.as_mut().map(|config| {
                let Some(key_index) = config.api_keys.iter().position(|key| key.name == name) else {
                    return;
                };
                if let Some(api_key) = config.api_key {
                    if api_key == key_index {
                        config.api_key = None;
                    } else if api_key > key_index {
                        config.api_key = Some(api_key - 1);
                    }
                }

                config.api_keys.remove(key_index);
            });
        });
    };

    let on_add = move |_| {
        if let Some(mut new_api_key) = new_key.get_untracked() {
            new_api_key.name = new_api_key.name.trim().into();
            if new_api_key.name.is_empty() {
                set_error("API key name must be non-empty.".into());
                return;
            }
            let mut api_keys = api_keys();
            if api_keys.iter().any(|api_key| api_key.name == new_api_key.name) {
                set_error("New key name must be unique.".into());
                return;
            }
            new_key.set(None);
            api_keys.push(new_api_key);
            set_api_keys(api_keys);
            set_error("".into());
        } else {
            new_key.set(Some(APIKey::default()));
        }
    };

    let button_classes = "px-[9px] py-[3px] w-[max-content] border-2 border-[#33333A]
        bg-[#222222] hover:bg-[#2A2A2A] text-[#AAAABB]";
    view! {
        <div class="col-span-2 grid grid-cols-1 gap-4">
            <h2 class="text-[1.1em] underline">"API Keys"</h2>
            <div
                class="grid grid-cols-[repeat(3,max-content)] gap-2 items-center"
                on:change=move |event| set_selected_key(Some(event_target_value(&event)))
            >
                <For
                    each=api_keys
                    key=|api_key| api_key.name.clone()
                    children=move |api_key| view! {
                        <KeyEntry api_key selected_key on_remove=Box::new(on_remove) />
                    }
                />
            </div>
            <KeyInput new_key set_error />
            <div class="flex">
                <button class=format!("mr-2 {}", button_classes)
                    style:display=move || new_key().is_none().then(|| "None")
                    on:click=move |_| new_key.set(None)>"Cancel"</button>
                <button class=button_classes on:click=on_add>"Add"</button>
            </div>
        </div>
    }
}

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

    let to_hide = Signal::derive(move || config().map_or(false, |config|
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
            set_error("".into());
        } else {
            let error_message = error.as_string().unwrap_or("Error parsing save_config error".into());
            set_error(error_message);
        }
    });

    view! {
        <div
            class="flex flex-col items-center h-full p-4 overflow-y-hidden text-[0.85rem]"
            style:display=move || (menu.get() != Menu::Settings).then(|| "None")
        >
            <Button class="mr-auto" label="Back"
                to_hide=create_signal(false).0.into()
                on_click=Box::new(move || set_menu(Menu::Chat))
            />
            <div class="w-full mt-2"><ErrorMessage error /></div>
            <h1 class="text-[1.25em]">"Settings"</h1>
            <div class="grid grid-cols-[repeat(2,max-content)] gap-8
                items-center my-auto overflow-y-auto"
            >
                <TemperatureSlider config set_error />
                <MaxTokensSlider config set_error />
                <ModelInput config />
                <KeyList config set_error />
            </div>
            <div class="flex justify-end w-full">
                <Button class="mr-4" label="Discard" to_hide on_click=Box::new(on_discard) />
                <Button class="" label="Apply" to_hide on_click=Box::new(on_save) />
            </div>
        </div>
    }
}