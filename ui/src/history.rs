use leptos::*;
use crate::util::{Button,ErrorMessage, Menu};

#[component]
pub fn History(menu: ReadSignal<Menu>, set_menu: WriteSignal<Menu>) -> impl IntoView {
    let (error, set_error) = create_signal("".to_string());

    let to_hide = create_signal(false).0.into();

    view! {
        <div class="relative flex flex-col items-center mx-auto md:w-[max-content] md:min-w-[60vw]
                    h-full p-4 md:p-[5vh] overflow-y-hidden text-[0.95em]"
                style:display=move || (menu.get() != Menu::History).then(|| "None")>
            <Button class="mr-auto" label="Back" to_hide
                on_click=Box::new(move || set_menu(Menu::Menu)) />
            <h1 class="text-[1.25em]">"History"</h1>
            <div class="w-full mt-2"><ErrorMessage error /></div>
            <div class="grid grid-cols-[repeat(2,max-content)] gap-[6.5vh]
                    items-center my-auto overflow-y-auto">
            </div>
        </div>
    }
}