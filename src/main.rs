use leptos::*;
use crate::chat::Chat;
use crate::settings::Settings;
use crate::common::Menu;

mod common;
mod chat;
mod settings;
mod util;

#[component]
fn App() -> impl IntoView {
    let (menu, set_menu) = create_signal(Menu::Chat);

    view! {
        <Chat menu set_menu />
        <Settings menu set_menu />
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
