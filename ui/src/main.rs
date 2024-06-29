use leptos::*;
use crate::chat::Chat;
use crate::util::{Button, Menu};
use crate::history::History;
use crate::settings::Settings;

mod chat;
mod commands;
mod util;
mod history;
mod settings;

#[component]
pub fn Menu(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    let to_hide = create_signal(false).0.into();
    view! {
        <div class="relative flex flex-col h-full"
                style:display=move || (menu.get() != Menu::Menu).then(|| "None")>
            <h1 class="absolute top-0 left-0 my-8 md:my-[5vh] w-full text-center text-[1.25em]"
            >"Menu"</h1>
            <div class="grid grid-cols-[50vw] md:grid-cols-[25vw] gap-12 md:gap-16
                justify-center items-center my-auto">
                <Button class="md:py-[6px]" label="Chat" to_hide
                    on_click=Box::new(move || set_menu(Menu::Chat)) />
                <Button class="md:py-[6px]" label="History" to_hide
                    on_click=Box::new(move || set_menu(Menu::History)) />
                <Button class="md:py-[6px]" label="Settings" to_hide
                    on_click=Box::new(move || set_menu(Menu::Settings)) />
            </div>
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let (menu, set_menu) = create_signal(Menu::Chat);
    let (config, set_config) = create_signal(common::Config::default());

    view! {
        <Chat config menu set_menu />
        <Menu menu set_menu />
        <History menu set_menu />
        <Settings active_config=config set_active_config=set_config menu set_menu />
    }
}

// #[tokio::main]
fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! {
            <App/>
        }
    })
}
