use leptos::*;
use crate::common::{Menu, Button};

#[component]
pub fn History(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    let to_hide = create_signal(false).0.into();
    view! {
        <div class="relative flex flex-col h-full text-[0.9rem]"
            style:display=move || (menu.get() != Menu::History).then(|| "None")
        >
            <div class="absolute top-4 left-[3vw] flex flex-col items-center w-[94vw]">
                <Button class="mr-auto" label="Back" to_hide
                    on_click=Box::new(move || set_menu(Menu::Menu)) />
                <h1 class="text-[1.25em]">"Conversations"</h1>
            </div>
            <div class="grid grid-cols-[50vw] gap-8 justify-center itme-center my-auto">
            </div>
        </div>
    }
}