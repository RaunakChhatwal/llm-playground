use leptos::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Menu {
    Chat,
    Settings
}

#[component]
pub fn Button(
    class: &'static str,
    label: &'static str,
    to_hide: Signal<bool>,
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