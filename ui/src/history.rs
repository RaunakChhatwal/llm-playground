use common::Conversation;
use leptos::*;
use wasm_bindgen::prelude::*;
use crate::util::{listen, Button, ErrorMessage, Menu};

async fn load_conversations(conversations: RwSignal<Vec<RwSignal<Conversation>>>, error: RwSignal<String>) {
    match crate::commands::load_conversations().await {
        Ok(_conversations) => {
            conversations.set(_conversations.into_iter().map(RwSignal::new).collect())
        },
        Err(err) => error.set(err.to_string())
    }
}

#[component]
pub fn History(menu: RwSignal<Menu>) -> impl IntoView {
    let error = create_rw_signal("".to_string());
    let conversations = create_rw_signal(Vec::<RwSignal<Conversation>>::new());

    spawn_local(load_conversations(conversations, error));

    spawn_local(async move {
        // listen for when the user/another window/this window changes the conversation history
        let on_update = Closure::new(move |_| spawn_local(load_conversations(conversations, error)));

        if let Err(_) = listen("conversations_updated", &on_update).await {
            error.set("Error listening for conversation history updates".into());
        }

        // keep on_update alive forever
        std::mem::forget(on_update);
    });

    let local_formatted_time = |conversation: Conversation| conversation.last_updated
        .with_timezone(&chrono::Local)
        .format("%m-%d-%Y")
        .to_string();
    let to_hide = create_signal(false).0.into();

    view! {
        <div class="relative flex flex-col items-center mx-auto md:w-[max-content] md:min-w-[60vw]
                    h-full p-4 md:p-[5vh] overflow-y-hidden text-[0.9em]"
                style:display=move || (menu.get() != Menu::History).then(|| "None")>
            <Button class="mr-auto" label="Back" to_hide
                on_click=Box::new(move || menu.set(Menu::Menu)) />
            <h1 class="text-[1.25em]">"History"</h1>
            <div class="w-full mt-2"><ErrorMessage error /></div>
            <div class="grid grid-cols-[repeat(3,max-content)] gap-[5vh] px-[5vw] w-full
                    overflow-y-auto justify-center items-center h-[75%] my-auto">
                <For each=conversations
                    key=|conversation| conversation.get_untracked().uuid
                    children=move |conversation| view! {
                        <p class="text-[0.9em]">{local_formatted_time(conversation())}</p>
                        <a class="truncate max-w-[45vw] text-blue-600" href
                        >{conversation().title}</a>
                        <a class="text-blue-600" href>"delete"</a>
                    } />
            </div>
        </div>
    }
}