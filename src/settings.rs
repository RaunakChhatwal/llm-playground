use leptos::*;
use crate::common::{Button, Menu};

#[component]
pub fn Settings(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    view! {
        <div
            class="flex flex-col items-center h-full p-4 overflow-y-hidden text-[0.9rem]"
            style:display=move || (menu.get() != Menu::Settings).then(|| "None")
        >
            <Button class="mr-auto" label="Back"
                to_hide=create_signal(false).0.into()
                on_click=Box::new(move || set_menu(Menu::Chat))/>
            <h1 class="text-[1.25em]">"Settings"</h1>
        </div>
    }
}