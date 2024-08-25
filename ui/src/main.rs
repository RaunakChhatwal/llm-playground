use leptos::*;
use crate::chat::Chat;
use crate::util::{button, Menu};
use crate::history::History;
use crate::settings::Settings;

mod chat;
mod commands;
mod util;
mod history;
mod settings;

#[component]
pub fn Menu(menu: RwSignal<Menu>) -> impl IntoView {
    view! {
        <div class="relative flex flex-col h-full"
                style:display=move || (menu.get() != Menu::Menu).then(|| "None")>
            <h1 class="absolute top-0 left-0 my-8 md:my-[5vh] w-full text-center text-[1.25em]"
            >"Menu"</h1>
            <div class="grid grid-cols-[50vw] md:grid-cols-[25vw] gap-12 md:gap-16
                justify-center items-center my-auto">
                <button class=button() + "md:py-[6px]" on:click=move |_| menu.set(Menu::Chat)>
                    "Chat"
                </button>
                <button class=button() + "md:py-[6px]" on:click=move |_| menu.set(Menu::History)>
                    "History"
                </button>
                <button class=button() + "md:py-[6px]" on:click=move |_| menu.set(Menu::Settings)>
                    "Settings"
                </button>
            </div>
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let conversation_uuid = create_rw_signal(None);
    let config = create_rw_signal(common::Config::default());
    let menu = create_rw_signal(Menu::Chat);

    match crate::util::_conversation_uuid.write() {
        Ok(mut _conversation_uuid) => *_conversation_uuid = conversation_uuid,
        Err(error) => eprintln!("{error}")      // this is unreachable so not handling error
    }

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
