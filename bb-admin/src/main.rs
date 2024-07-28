#![allow(non_snake_case)]

use std::{collections::HashMap, path::Path};

use api_types::{GetLobbiesResponse, GetLobbyResponse};
use chrono::Utc;
use common::{
    Card, Connect, GameAction, GameClient, GameEvent, GameMessage, GameState, SetupGameOptions,
};
use dioxus::prelude::*;
use dotenvy::dotenv;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
// static APP_STATE: GlobalSignal<FrontendAppState> = Signal::global(|| FrontendAppState::new());
// const STYLE: &str = manganis::mg!(file("./assets/main.css")); // this works but does not reload nicely

const STYLE: &str = include_str!("../assets/main.css");

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
    // launch(App);
    launch(|| {
        rsx! {
            head {
                link { rel: "stylesheet", href: STYLE }
            }
            Router::<Route> {}
        }
    });
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

// static GAMESTATE: GlobalSignal<GameState> = Signal::global(|| GameState::new());

#[component]
fn Admin() -> Element {
    let mut ws_url = use_signal(|| String::from("ws://0.0.0.0:8080/ws"));
    let mut ws_action = use_signal(|| WsAction::Pause);
    let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));
    let mut test_ws = use_signal(|| String::from("test ws"));

    let mut username = use_signal(|| String::new());
    let mut lobby = use_signal(|| String::new());
    // let mut lobbies = use_signal(|| String::new());
    let mut connect_response = use_signal(|| String::from("..."));
    let mut lobbies = use_signal(|| GetLobbiesResponse { lobbies: vec![] });

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
            {if lobbies.read().lobbies.len() == 0 {
                rsx!(div { "No games" })
            } else {
                rsx!({
                    lobbies.read().lobbies.iter().map(|lobby| rsx!(LobbyComponent {lobby: lobby}))
                })
            }}
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
                onclick: move |_| {
                    info!("Join {lobby}");
                },
                "Game details"
            }
            Link {
                class: "shadow-sm p-6 rounded-md bg-slate-200 hover:bg-slate-300 w-1/2",
                to: Route::GameRoom {
                    room_code: lobby.clone(),
                },
                "Join game"
            }
        }
    )
}

// All of our routes will be a variant of this Route enum
#[derive(Routable, PartialEq, Clone)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    //  if the current location doesn't match any of the other routes, redirect to "/home"
    #[redirect("/:..segments", |segments: Vec<String>| Route::Home {})]
    Home {},
    #[route("/rooms/:room_code")]
    GameRoom { room_code: String },
    #[route("/admin")]
    Admin {},
    // #[route("/game/:room_code")]
    // Game { room_code: String },
}

#[component]
fn GameRoom(room_code: String) -> Element {
    info!("GameRoom: {room_code}");

    let mut server_message = use_signal(|| Value::Null);
    let mut gamestate = use_signal(|| GameState::new());
    let mut ws_url =
        use_signal(|| String::from(format!("ws://0.0.0.0:8080/rooms/{}/ws", room_code.clone())));
    let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));
    let mut lobby = use_signal(|| GetLobbyResponse {
        lobby_code: room_code.clone(),
        players: vec![],
    });
    let mut username = use_signal(|| String::new());
    let mut error = use_signal(|| Value::Null);
    let mut player_secret = use_signal(|| String::new());

    let get_game_details = move |room_code: String| {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get(format!("http://localhost:8080/rooms/{}", room_code))
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    match data.json::<GetLobbyResponse>().await {
                        Ok(resp) => lobby.set(resp),
                        Err(err) => error.set(json!(format!("Failed to parse lobby: {:?}", err))),
                    }
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    lobby.set(GetLobbyResponse {
                        lobby_code: room_code.clone(),
                        players: vec![format!("{err}")],
                    });
                }
            }
        });
    };

    let mut ws_url =
        use_signal(|| String::from(format!("ws://0.0.0.0:8080/rooms/{}/ws", room_code.clone())));
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

                let mut is_gamestate = false;
                match serde_json::from_str::<GameState>(&text) {
                    Ok(x) => {
                        is_gamestate = true;
                        gamestate.set(x);
                    }
                    Err(_) => {}
                };

                if is_gamestate {
                    return;
                }
                match serde_json::from_str::<Value>(&text) {
                    Ok(x) => {
                        if x.get("client_secret").is_some() {
                            player_secret.set(x.get("client_secret").unwrap().to_string());
                        }
                        server_message.set(x);
                    }
                    Err(err) => info!("Failed to parse server message: {}", err),
                };
            }
        }
    });

    let room_code_clone = room_code.clone();
    let join_lobby = move |room_code_clone: String| {
        ws.send(InternalMessage::WsAction(WsAction::Resume));
        ws.send(InternalMessage::Server(Connect {
            username: username(),
            channel: room_code_clone,
            secret: None,
        }));
    };

    rsx!(
        {if error().is_null() {rsx!()} else {error.read().as_str().map(|err| rsx!(div { "{err}" }))}},
        button {
            class: "h-full w-full bg-blue-500",
            onclick: move |evt| get_game_details(room_code_clone.clone()),
            "Refresh"
        }
        div { class: "flex flex-row",
            label { "Username" }
            input {
                r#type: "text",
                value: "{username}",
                required: true,
                minlength: 3,
                oninput: move |event| username.set(event.value())
            }
        }
        button {
            class: "h-full w-full bg-blue-500",
            onclick: move |evt| join_lobby(room_code.clone()),
            "Join this game"
        }
        div { "lobby: {lobby.read().lobby_code}" }
        div { "Players ({lobby.read().players.len()})" }
        {lobby.read().players.iter().enumerate().map(|(i, player)| rsx!(div { "{i}: {player}" }))},
        // div { "{gamestate.read():?}" }
        div { "Server message: {server_message.read()}" }
        {if username() != "" {
            rsx!(GameState { username, player_secret, gamestate })
        } else {
            rsx!(div { "Please choose a username" })
        }}
    )
}

#[component]
fn Home() -> Element {
    // let mut ws_url =
    //     use_signal(|| String::from(format!("ws://0.0.0.0:8080/rooms/{}/ws", room_code)));
    // let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));

    rsx!(
        div { class: "bg-slate-200 w-full h-full", "Home" }

        Link { class: "bg-orange-300 w-full h-full", to: Route::Admin {}, "Admin" }
        Link {
            class: "bg-orange-300 w-full h-full",
            to: Route::GameRoom {
                room_code: String::new(),
            },
            "GameRoom"
        }
    )
}

#[component]
fn GameState(
    username: Signal<String>,
    player_secret: Signal<String>,
    gamestate: Signal<GameState>,
) -> Element {
    let myusername = username.read().clone();
    fn create_action(username: String, action: GameAction) -> GameMessage {
        return GameMessage {
            username: username,
            message: GameEvent { action },
            timestamp: Utc::now(),
        };
    };

    let curr_hand = gamestate
        .read()
        .players
        .get(&myusername)
        .unwrap()
        .encrypted_hand
        .clone();
    let decrypted_hand = GameState::decrypt_player_hand(curr_hand, &player_secret.read().clone());
    rsx!(
        div { class: "bg-green-300 w-full h-full",
            div { "This is the game" }
            div { "State: {gamestate.read().gameplay_state:?}" }
            div { "Trump: {gamestate.read().trump:?}" }
            div {
                ol { {gamestate.read().player_order.iter().map(|player| rsx!(li { "{player}" }))} }
            }
            div { "Round: {gamestate.read().curr_round}" }
            div { "Dealer: {gamestate.read().curr_dealer}" }
            div { "Player turn: {gamestate.read().curr_player_turn:?}" }
        }
        div { class: "bg-blue-300 w-full h-full",
            "Play area"
            div {
                "Cards"
                {gamestate.read().curr_played_cards.iter().map(|card| rsx!(li { "{card}" }))}
            }
        }
        div { class: "bg-red-300 w-full h-full",
            "Action area"
            div {
                "My cards"
                {decrypted_hand.iter().map(|card| rsx!(li { "{card:?}" }))}
            }
        }
    )
}

#[component]
fn PlayerComponent(player: GameClient) -> Element {
    info!("PlayerComponent: {}", player.id);
    rsx!(
        div { class: "bg-blue-300 w-full h-full",
            div { "this is a player" }
            div { "{player.id}" }
        }
    )
}
