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
pub fn Menu(menu: RwSignal<Menu>) -> impl IntoView {
    let to_hide = create_signal(false).0.into();
    view! {
        <div class="relative flex flex-col h-full"
                style:display=move || (menu.get() != Menu::Menu).then(|| "None")>
            <h1 class="absolute top-0 left-0 my-8 md:my-[5vh] w-full text-center text-[1.25em]"
            >"Menu"</h1>
            <div class="grid grid-cols-[50vw] md:grid-cols-[25vw] gap-12 md:gap-16
                justify-center items-center my-auto">
                <Button class="md:py-[6px]" label="Chat" to_hide
                    on_click=Box::new(move || menu.set(Menu::Chat)) />
                <Button class="md:py-[6px]" label="History" to_hide
                    on_click=Box::new(move || menu.set(Menu::History)) />
                <Button class="md:py-[6px]" label="Settings" to_hide
                    on_click=Box::new(move || menu.set(Menu::Settings)) />
            </div>
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let conversation_uuid = create_rw_signal(None);
    let config = create_rw_signal(common::Config::default());
    let menu = create_rw_signal(Menu::Chat);

    *crate::util::_conversation_uuid.write().unwrap() = conversation_uuid;

    view! {
        <Chat config menu />
        <Menu menu />
        <History menu />
        <Settings active_config=config menu />
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! {
            <App/>
        }
    })
}
