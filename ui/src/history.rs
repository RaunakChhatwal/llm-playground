use common::Conversation;
use leptos::*;
use wasm_bindgen::prelude::*;
use crate::commands::delete_conversation;
use crate::util::{button, listen, set_conversation_uuid, ErrorMessage, Menu};

lazy_static::lazy_static! {
    // anyhow! macro doesn't work if there is a static variable named "error" in the namespace
    pub static ref signal_pair: (ReadSignal<String>, WriteSignal<String>) = create_signal("".into());
    pub static ref set_error: WriteSignal<String> = signal_pair.1;
}

async fn load_conversations(conversations: RwSignal<Vec<RwSignal<Conversation>>>) {
    let new_conversations = match crate::commands::load_conversations().await {
        Ok(conversations) => conversations,
        Err(error) => {
            set_error(error.to_string());
            return;
        }
    };

    let uuid_to_conversation = conversations.get_untracked()
        .into_iter()
        .map(|conversation| (conversation.get_untracked().uuid, conversation))
        .collect::<std::collections::HashMap<_, _>>();

    conversations.update(|conversations| {
        conversations.clear();
        for new_conversation in new_conversations {
            if let Some(&conversation) = uuid_to_conversation.get(&new_conversation.uuid) {
                conversation.set(new_conversation);
                conversations.push(conversation);
            } else {
                conversations.push(create_rw_signal(new_conversation));
            }
        }
    });
}

#[component]
pub fn History(menu: RwSignal<Menu>) -> impl IntoView {
    let error = signal_pair.0;
    let conversations = create_rw_signal(Vec::<RwSignal<Conversation>>::new());

    spawn_local(load_conversations(conversations));

    spawn_local(async move {
        // listen for when the user/another window/this window changes the conversation history
        let on_update = Closure::new(move |_| spawn_local(load_conversations(conversations)));

        if let Err(_) = listen("conversations_updated", &on_update).await {
            set_error("Error listening for conversation history updates".into());
        }

        // keep on_update alive forever
        std::mem::forget(on_update);
    });

    let on_load = move |uuid| {
        set_conversation_uuid(uuid);
        menu.set(Menu::Chat);
    };

    let on_delete = move |uuid| spawn_local(async move {
        if let Err(error) = delete_conversation(uuid).await {
            set_error(error.to_string());
        }
    });

    let local_formatted_time = |conversation: Conversation| conversation.last_updated
        .with_timezone(&chrono::Local)
        .format("%m-%d-%Y")
        .to_string();

    view! {
        <div class="relative flex flex-col items-center mx-auto md:w-[max-content] md:min-w-[60vw]
                    h-full px-[5vw] py-[5vh] overflow-y-hidden"
                style:display=move || (menu.get() != Menu::History).then(|| "None")>
            <button class=button() + "mr-auto" on:click=move |_| menu.set(Menu::Menu)>"Back"</button>
            <h1 class="text-[1.25em]">"History"</h1>
            <div class="w-full mt-2"><ErrorMessage error /></div>
            <p class="w-full mt-[10vh] mr-auto"
                style:display=move || (!conversations().is_empty()).then(|| "None")
            >"No conversations saved."</p>
            <div class="grid grid-cols-[repeat(3,max-content)] gap-[5vh] my-[10vh] w-full
                    overflow-y-auto justify-center items-center text-[0.925em]">
                <For each=conversations
                    key=|conversation| conversation.get_untracked().uuid
                    children=move |conversation| view! {
                        <p class="text-[0.9em]">{move || local_formatted_time(conversation())}</p>
                        <a class="truncate w-[45vw] text-blue-600 cursor-pointer"
                            on:click=move |_| on_load(Some(conversation.get_untracked().uuid))
                        >{move || conversation().title}</a>
                        <a class="text-blue-600 cursor-pointer"
                            on:click=move |_| on_delete(conversation.get_untracked().uuid)
                        >"delete"</a>
                    } />
            </div>
        </div>
    }
}