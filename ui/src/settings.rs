use std::str::FromStr;
use common::{APIKey, Config, Provider};
use leptos::*;
use strum::VariantNames;
use wasm_bindgen::prelude::*;
use crate::commands::{load_config, save_config};
use crate::util::{listen, Button, ErrorMessage, Menu};

lazy_static::lazy_static! {
    // anyhow! macro doesn't work if there is a static variable named "error" in the namespace
    pub static ref signal_pair: (ReadSignal<String>, WriteSignal<String>) = create_signal("".into());
    pub static ref set_error: WriteSignal<String> = signal_pair.1;
}

pub fn update_textarea_height(textarea: &HtmlElement<html::Textarea>) {
    textarea.set_attribute("style", "height: auto;")
        .unwrap_or_else(|error| set_error(format!("{error:?}")));
    let style = format!("height: {}px;", textarea.scroll_height());
    textarea.set_attribute("style", &style)
        .unwrap_or_else(|error| set_error(format!("{error:?}")));
}

#[component]
fn SystemPromptInput(config: RwSignal<Config>, menu: RwSignal<Menu>) -> impl IntoView {
    let class = "flex-none w-full min-h-[2em] px-2 pt-1 pb-2 border border-[#303038]
        bg-[#222222] text-[0.9em] overflow-hidden resize-none";
    let system_prompt_input = view! {
        <textarea rows=2 class=class type="text"></textarea>
    };

    let on_input = Closure::<dyn Fn(web_sys::Event) + 'static>::new({
        let system_prompt_input = system_prompt_input.clone();
        move |event| {
            config.update(|config| config.system_prompt = event_target_value(&event));
            update_textarea_height(&system_prompt_input);
        }
    });
    system_prompt_input.set_oninput(Some(on_input.as_ref().unchecked_ref()));
    std::mem::forget(on_input);

    // this is because the value element attribute would not work
    create_effect({
        let system_prompt_input = system_prompt_input.clone();
        move |_| {
            menu.track();       // this is because textarea.scroll_height() defends on the current menu
            config.with(|config| {
                // this is different from setting the textarea's value html attribute, which will not work
                system_prompt_input.set_value(&config.system_prompt);
                update_textarea_height(&system_prompt_input);
            });
        }
    });

    view! {
        <div class="col-span-2 flex flex-col">
            <label class="mb-2">"System prompt:"</label>
            {system_prompt_input}
        </div>
    }
}

#[component]
fn TemperatureSlider(config: RwSignal<Config>) -> impl IntoView {
    let on_input = move |event| {
        let Ok(temperature) = event_target_value(&event).parse::<f64>() else {
            set_error("The slider should only permit numbers.".into());
            return;
        };
        config.update(|config| config.temperature = temperature);
    };

    let temperature_slider = view! {
        <input class="accent-blue-900" id="temperature-slider"
            on:input=on_input type="range" min="0" max="1" step="0.1" />
    };

    create_effect({
        let temperature_slider = temperature_slider.clone();
        move |_| {
            let temperature = config().temperature.to_string();
            if temperature_slider.value() != temperature {
                // this is different from setting the input's value html attribute, which will not work
                temperature_slider.set_value(&temperature);
            }
        }
    });

    return view! {
        <label>"Temperature:"</label>
        <div class="flex items-center">
            {temperature_slider}
            <span class="mx-2">"|"</span>
            <span>{move || config().temperature.to_string()}</span>
        </div>
    };
}

#[component]
fn MaxTokensInput(max_tokens: RwSignal<String>,) -> impl IntoView {
    let on_input = move |event| max_tokens.set(event_target_value(&event));

    let max_tokens_input = view! {
        <input type="text" on:input=on_input
            class="px-2 py-1 bg-[#222222] border border-[#33333A] text-[0.9em]" />
    };

    create_effect({
        let max_tokens_input = max_tokens_input.clone();
        move |_| max_tokens.with(|max_tokens|
            if &max_tokens_input.value() != max_tokens {
                max_tokens_input.set_value(max_tokens);
            }
        )
    });

    view! {
        <label>"Max tokens:"</label>
        {max_tokens_input}
    }
}

#[component]
fn ModelInput(config: RwSignal<Config>) -> impl IntoView {
    let on_input = move |event| config.update(|config|
        config.model = event_target_value(&event));

    let model_input = view! {
        <input type="text" on:input=on_input
            class="px-2 py-1 bg-[#222222] border border-[#33333A] text-[0.9em]" />
    };

    create_effect({
        let model_input = model_input.clone();
        move |_| {
            let config = config();
            if model_input.value() != config.model {
                model_input.set_value(&config.model);
            }
        }
    });

    view! {
        <label>"Model:"</label>
        {model_input}
    }
}

#[component]
fn KeyEntry(
    api_key: APIKey,
    selected_key: Signal<Option<String>>,
    on_remove: Box<dyn Fn(&str)>
) -> impl IntoView {
    let key_entry = view! {
        <input type="radio" value=api_key.name.clone() name="key_name" />
    };

    create_effect({
        let name = api_key.name.clone();
        let key_entry = key_entry.clone();
        move |_| {
            // this is different from setting the input's checked html attribute, which will not work
            key_entry.set_checked(selected_key().as_ref() == Some(&name));
        }
    });

    view! {
        {key_entry}
        <p class="mx-2">{api_key.name.clone()}</p>
        <button class="px-[5px] w-[max-content] h-[max-content] border border-[#33333A]
                bg-[#222222] hover:bg-[#33333A] text-[#AAAABB]"
            on:click=move |_| on_remove(&api_key.name)
        >"-"</button>
    }
}

#[component]
fn KeyInput(new_key: RwSignal<Option<APIKey>>) -> impl IntoView {
    create_effect(move |_| {
        let new_provider = new_key().map(|new_key| new_key.provider.to_string());
        for &provider in Provider::VARIANTS {
            let input = document().get_element_by_id(&format!("provider-{}", provider))
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok());
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
            <input type="text" class="px-1 bg-[#222222] h-[2em] border border-[#33333A] text-[0.9em]"
                on:input=move |event| new_key.update(|new_key| {
                    new_key.as_mut().map(|new_key|
                        new_key.name = event_target_value(&event));
                }) />
            <label>"Key:"</label>
            <input type="text" class="px-1 bg-[#222222] h-[2em] border border-[#33333A] text-[0.9em]"
                on:input=move |event| new_key.update(|new_key| {
                    new_key.as_mut().map(|new_key|
                        new_key.key = event_target_value(&event));
                }) />
            <label>"Provider:"</label>
            <div class="grid grid-cols-[repeat(2,max-content)] items-center">
                <For each=move || Provider::VARIANTS
                    key=|&provider_name| provider_name
                    children=|&provider_name| view! {
                        <input type="radio" value=provider_name name="provider"
                            id=format!("provider-{provider_name}") />
                        <label class="ml-2">{provider_name}</label>
                    } />
            </div>
        </div>
    }
}

#[component]
fn KeyList(config: RwSignal<Config>) -> impl IntoView {
    let (api_keys, set_api_keys) = create_slice(
        config,
        |config| config.api_keys.clone(),
        |config, api_keys| config.api_keys = api_keys
    );
    let (selected_key, set_selected_key) = create_slice(
        config,
        |config| Some(config.api_keys.get(config.api_key?)?.name.clone()),
        |config, selected_key: Option<String>|
            config.api_key = selected_key.and_then(|selected_key|
                config.api_keys.iter().position(|api_key| api_key.name == selected_key))
    );
    let new_key = create_rw_signal(None::<APIKey>);

    let on_remove = move |name: &str| {
        config.update(|config| {
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

    let button_classes = "px-[9px] py-[3px] w-[max-content] border border-[#33333A]
        bg-[#222222] hover:bg-[#2A2A2A] text-[#AAAABB]";
    view! {
        <div class="col-span-2 grid grid-cols-1 gap-4">
            <h2 class="text-[1.1em] underline">"API Keys"</h2>
            <div class="grid grid-cols-[repeat(3,max-content)] gap-2 items-center"
                    on:change=move |event| set_selected_key(Some(event_target_value(&event)))>
                <For each=api_keys
                    key=|api_key| api_key.name.clone()
                    children=move |api_key| view! {
                        <KeyEntry api_key selected_key on_remove=Box::new(on_remove) />
                    } />
            </div>
            <KeyInput new_key />
            <div class="flex">
                <button class=format!("mr-2 {}", button_classes)
                    style:display=move || new_key().is_none().then(|| "None")
                    on:click=move |_| new_key.set(None)
                >"Cancel"</button>
                <button class=button_classes on:click=on_add>"Add"</button>
            </div>
        </div>
    }
}

#[component]
pub fn Settings(active_config: RwSignal<Config>, menu: RwSignal<Menu>) -> impl IntoView {
    let error = signal_pair.0;
    let config = create_rw_signal(Config::default());
    let max_tokens = create_rw_signal("".into());
    let saved_config = create_rw_signal(None);

    spawn_local(async move {
        match load_config().await {
            Ok(loaded_config) => {
                config.set(loaded_config.clone());
                max_tokens.set(loaded_config.max_tokens.to_string());
                active_config.set(loaded_config.clone());
                saved_config.set(Some(loaded_config));
            },
            Err(error) => set_error(error.to_string())
        }
    });

    spawn_local(async move {
        // listen for when the user/another window/this window changes the config
        let on_update = Closure::new(move |_| spawn_local(async move {
            match load_config().await {
                Ok(config) => saved_config.set(Some(config)),
                Err(error) => set_error(error.to_string())
            }
        }));

        if let Err(_) = listen("config_updated", &on_update).await {
            set_error("Error listening for config updates".into());
        }

        // keep on_update alive forever
        std::mem::forget(on_update);
    });

    let to_hide = Signal::derive(move || {
        let config = config();
        let active_config = active_config();
        return config == active_config &&
            max_tokens() == active_config.max_tokens.to_string() &&
            Some(config) == saved_config();
    });

    let on_discard = move || {
        if let Some(saved_config) = saved_config.get_untracked() {
            config.set(saved_config.clone());
            max_tokens.set(saved_config.max_tokens.to_string());
            active_config.set(saved_config);
        } else {
            set_error("Bad config.".into());
        };
    };

    let on_apply = move || {
        let max_tokens = match max_tokens.get_untracked().parse::<u32>() {
            Ok(max_tokens) => max_tokens,
            Err(error) => {
                set_error(error.to_string());
                return;
            }
        };
        config.update(|config| config.max_tokens = max_tokens);
        let config = config.get_untracked();
        active_config.set(config.clone());
        spawn_local(async move {
            if let Err(error) = save_config(config.clone()).await {
                set_error(error.to_string());
            } else {
                saved_config.set(Some(config));
                set_error("".into());
            }
        });
    };

    view! {
        <div class="relative flex flex-col items-center mx-auto md:w-[max-content] md:min-w-[60vw]
                    h-full p-4 md:p-[5vh] overflow-y-hidden text-[0.95em]"
                style:display=move || (menu.get() != Menu::Settings).then(|| "None")>
            <Button class="mr-auto" label="Back"
                to_hide=create_signal(false).0.into()
                on_click=Box::new(move || menu.set(Menu::Menu)) />
            <h1 class="text-[1.25em]">"Settings"</h1>
            <div class="w-full mt-2"><ErrorMessage error /></div>
            <div class="grid grid-cols-[repeat(2,max-content)] gap-[6vh] items-center my-auto overflow-y-auto">
                <SystemPromptInput config menu />
                <TemperatureSlider config />
                <MaxTokensInput max_tokens />
                <ModelInput config />
                <KeyList config />
            </div>
            <div class="flex justify-end mb-[4vh] md:mb-[8vh] w-full">
                <Button class="mr-4" label="Discard" to_hide on_click=Box::new(on_discard) />
                <Button class="" label="Apply" to_hide on_click=Box::new(on_apply) />
            </div>
        </div>
    }
}