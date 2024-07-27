#![allow(non_snake_case)]

use std::{collections::HashMap, path::Path};

use api_types::GetLobbiesResponse;
use common::{Connect, GameEvent, GameMessage, GameState};
use dioxus::prelude::*;
use dotenvy::dotenv;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, Level};

#[derive(Clone, Debug)]
struct FrontendAppState {
    ws_url: String,
    server_url: String,
}

impl FrontendAppState {
    fn new() -> Self {
        // Self {
        //     ws_url: dotenvy::var("WS_URL").unwrap(),
        //     server_url: dotenvy::var("SERVER_URL").unwrap(),
        // }
        Self {
            ws_url: String::new(),
            server_url: String::new(),
        }
    }
}

// static APP_STATE: OnceLock<FrontendAppState> = OnceLock::new();
static APP_STATE: GlobalSignal<FrontendAppState> = Signal::global(|| FrontendAppState::new());

// pub fn get_app_state() -> &'static FrontendAppState {
//     APP_STATE().unwrap()
// }

fn main() {
    // info!("Starting app...");
    // let feas = FrontendAppState {
    //     ws_url: dotenvy::var("WS_URL").unwrap(),
    //     server_url: dotenvy::var("SERVER_URL").unwrap(),
    // };
    // APP_STATE.set(feas);
    // appstate.ws_url = dotenvy::var("WS_URL").unwrap();
    // appstate.server_url = dotenvy::var("SERVER_URL").unwrap();
    // let mut appstate = FrontendAppState {
    //     ws_url: dotenvy::var("WS_URL").unwrap(),
    //     server_url: dotenvy::var("SERVER_URL").unwrap(),
    // };
    // APP_STATE.set(appstate).unwrap();

    // info!("AppState: {:?}", get_app_state());

    // info!("cwd: {:?}", std::env::current_dir());
    // let va = dotenvy::dotenv().ok(); // load .env file
    // info!("va: {:?}", va);
    // info!("server_url: {:?}", dotenvy::var("SERVER_URL"));

    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[derive(Clone, Debug)]
enum InternalMessage {
    Game(GameMessage),
    Server(Connect),
    WsAction(WsAction),
}

#[derive(Clone, Debug)]
enum WsAction {
    Pause,
    Resume,
}

static GAMESTATE: GlobalSignal<GameState> = Signal::global(|| GameState::new());

#[component]
fn App() -> Element {
    let mut ws_url = use_signal(|| String::from("ws://0.0.0.0:8080/ws"));
    let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));
    let mut test_ws = use_signal(|| String::from("test ws"));
    let mut ws_action = use_signal(|| WsAction::Pause);

    let ws: Coroutine<InternalMessage> = use_coroutine(|mut rx| async move {
        info!("ws coroutine started - action: {:?}", &ws_action);

        'pauseloop: while let Some(internal_msg) = rx.next().await {
            match internal_msg {
                InternalMessage::Game(_) => break,
                InternalMessage::Server(_) => break,
                InternalMessage::WsAction(action) => match action {
                    WsAction::Pause => {
                        continue 'pauseloop;
                    }
                    WsAction::Resume => {
                        break;
                    }
                },
            }
        }

        // Connect to some sort of service
        // Creates a GET request, upgrades and sends it.
        let response = Client::default()
            .get(ws_url())
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
            InternalMessage::WsAction(_) => {}
        }

        // The WebSocket is also a `TryStream` over `Message`s.
        while let Some(message) = websocket.try_next().await.unwrap() {
            if let Message::Text(text) = message {
                info!("received: {text}");
                test_ws.set(text);
            }
        }
    });

    let mut username = use_signal(|| String::new());
    let mut lobby = use_signal(|| String::new());
    // let mut lobbies = use_signal(|| String::new());
    let mut connect_response = use_signal(|| String::from("..."));
    let mut lobbies = use_signal(|| GetLobbiesResponse { lobbies: vec![] });

    // Build cool things ✌️
    let mut get_games_future = use_resource(|| async move {
        reqwest::get("http://0.0.0.0:8080/rooms")
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
            button { onclick: create_lobby, "Create lobby" }
            div { "results: {connect_response}" }
        }
        button { onclick: refresh_lobbies, "Refresh lobbies" }
        div {
            "Ongoing games"
            {lobbies.read().lobbies.iter().map(|lobby| rsx!(LobbyComponent {lobby: lobby}))}
        }
    }
}

#[component]
fn LobbyComponent(lobby: String) -> Element {
    // let mut lobby = use_signal(|| lobby);
    rsx!(
        div { class: "flex flex-row justify-between",
            div { "{lobby}" }
            button {
                class: "shadow-sm p-6 rounded-md bg-slate-200 hover:bg-slate-300 w-1/2",
                onclick: move |_| println!("Join {lobby}"),
                "Join"
            }
        }
    )
}
