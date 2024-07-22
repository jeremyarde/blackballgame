#![allow(non_snake_case)]

use std::collections::HashMap;

use common::GameState;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::Level;

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[component]
fn App() -> Element {
    let mut username = use_signal(|| String::new());
    let mut lobby = use_signal(|| String::new());
    let mut connect_response = use_signal(|| String::from("..."));
    let mut lobbies = use_signal(|| vec![]);

    // Build cool things ✌️
    let mut get_games_future = use_resource(|| async move {
        reqwest::get("http://localhost:8080/v1/rooms")
            .await
            .unwrap()
            .json::<HashMap<String, GameState>>()
            .await
    });

    let create_lobby = move |_| {
        #[derive(Deserialize, Serialize)]
        pub struct CreateGameRequest {
            lobby_code: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .post("http://localhost:8080/rooms")
                .json(&CreateGameRequest {
                    lobby_code: lobby().clone(),
                })
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    connect_response.set(format!("response: {:?}", data).into());
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    connect_response.set(format!("{err}").into());
                }
            }
        });
    };

    let join_lobby = move |_| {
        #[derive(Deserialize, Serialize)]
        struct Connect {
            username: String,
            channel: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .post("http://localhost:8080/rooms")
                .json(&Connect {
                    username: username().clone(),
                    channel: lobby().clone(),
                })
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    connect_response.set(format!("response: {:?}", data).into());
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    connect_response.set(format!("{err}").into());
                }
            }
        });
    };

    let refresh_lobbies = move |_| {
        #[derive(Deserialize, Serialize)]
        struct Connect {
            username: String,
            channel: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .get("http://localhost:8080/rooms")
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    connect_response.set(format!("response: {:?}", data).into());
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    connect_response.set(format!("{err}").into());
                }
            }
        });
    };

    rsx! {
        div { class: "flex flex-col",
            div { "results: {connect_response}" }
            input {
                r#type: "text",
                value: "{username}",
                oninput: move |event| username.set(event.value())
            }
            input {
                r#type: "text",
                value: "{lobby}",
                oninput: move |event| lobby.set(event.value())
            }
        }
        button { onclick: join_lobby, "Join" }
        div { class: "flex flex-col",
            "create lobby"
            input {
                r#type: "text",
                value: "{lobby}",
                oninput: move |event| lobby.set(event.value())
            }
            div { "results: {connect_response}" }
        }
        button { onclick: create_lobby, "Create lobby" }

        button { onclick: refresh_lobbies, "Refresh lobbies" }
        div {
            "lobby list"
            {vec!["lobby code here"].into_iter().map(|lobby| rsx!(div{"{lobby}"}))}
        }
    }
}
