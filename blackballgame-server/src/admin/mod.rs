use std::sync::Arc;

use axum::{extract::State, response::Html};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[axum::debug_handler]
pub async fn app_endpoint(State(state): State<Arc<AppState>>) -> Html<String> {
    // let mut user_name = use_signal(|| "?".to_string());

    let test = state
        .rooms
        .lock()
        .await
        .iter()
        .map(|(roomid, gamestate)| {
            return format!("{} has {} players", roomid, gamestate.players.len());
        })
        .collect::<Vec<String>>();

    let rooms = if test.len() > 0 {
        test.join("\n")
    } else {
        "No rooms".to_string()
    };
    let text = format!("Rooms:\n{}", rooms);

    // render the rsx! macro to HTML
    Html(dioxus_ssr::render_element(rsx! {
        div { "{text}" }
        div {
            input {
                // onchange: move |evt| user_name.set(evt.value()),
                // value: "{user_name}"
            }
            button {
                onclick: move |_| {
                    async move {
                        post_server_data(String::from("Hi from client")).await.unwrap();
                    }
                },
                "Login Test User"
            }
        }
    }))
}

#[server]
async fn post_server_data(data: String) -> Result<(), ServerFnError> {
    println!("Server received: {}", data);

    Ok(())
}
