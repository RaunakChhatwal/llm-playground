use leptos::*;

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