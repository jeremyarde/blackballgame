#![allow(non_snake_case)]

use std::collections::HashMap;

use api_types::GetLobbiesResponse;
use common::{Connect, GameEvent, GameMessage, GameState};
use dioxus::prelude::*;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::Level;

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

enum InternalMessage {
    Game(GameMessage),
    Server(Connect),
}

static GAMESTATE: GlobalSignal<GameState> = Signal::global(|| GameState::new());

#[component]
fn App() -> Element {
    let mut test_ws = use_signal(|| String::new());

    let ws: Coroutine<InternalMessage> = use_coroutine(|mut rx| async move {
        // Connect to some sort of service
        // Creates a GET request, upgrades and sends it.
        let response = Client::default()
            .get("ws://0.0.0.0:8080/ws")
            .upgrade() // Prepares the WebSocket upgrade.
            .send()
            .await
            .unwrap();

        // Turns the response into a WebSocket stream.
        let mut websocket = response.into_websocket().await.unwrap();

        // wait for connection message
        let connection_details: InternalMessage = rx.next().await.unwrap();

        match connection_details {
            InternalMessage::Game(x) => {
                let msg = Message::Text(json!(x).to_string());
                websocket.send(msg).await.unwrap();
            }
            InternalMessage::Server(x) => {
                let msg = Message::Text(json!(x).to_string());
                websocket.send(msg).await.unwrap();
            }
        }

        // The WebSocket is also a `TryStream` over `Message`s.
        while let Some(message) = websocket.try_next().await.unwrap() {
            if let Message::Text(text) = message {
                println!("received: {text}");
                test_ws.set(text);
            }
        }
    });

    let mut username = use_signal(|| String::new());
    let mut lobby = use_signal(|| String::new());
    // let mut lobbies = use_signal(|| String::new());
    let mut connect_response = use_signal(|| String::from("..."));
    let mut lobbies = use_signal(|| GetLobbiesResponse {
        lobbies: vec![String::from("No lobbies")],
    });

    // Build cool things ✌️
    let mut get_games_future = use_resource(|| async move {
        reqwest::get("http://0.0.0.0:8080/v1/rooms")
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
        ws.send(InternalMessage::Server(Connect {
            username: username(),
            channel: lobby(),
            secret: None,
        }));
    };

    let refresh_lobbies = move |_| {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get("http://localhost:8080/rooms")
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    lobbies.set(data.json::<GetLobbiesResponse>().await.unwrap());
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    lobbies.set(GetLobbiesResponse {
                        lobbies: vec![format!("{err}")],
                    });
                }
            }
        });
    };

    rsx! {
        div { class: "flex flex-col",
            div { "websocket state: {test_ws}" }
            label { "username" }
            input {
                r#type: "text",
                value: "{username}",
                oninput: move |event| username.set(event.value())
            }
            label { "lobby" }
            input {
                r#type: "text",
                value: "{lobby}",
                oninput: move |event| lobby.set(event.value()),
                "lobby"
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
            {lobbies.read().lobbies.iter().map(|lobby| rsx!(div{"{lobby}"}))}
        }
    }
}
