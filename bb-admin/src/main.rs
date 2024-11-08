#![allow(non_snake_case)]
#![allow(warnings)]

use std::{collections::HashMap, path::Path};

use api_types::{GetLobbiesResponse, GetLobbyResponse, Lobby};
use chrono::Utc;
use common::{
    Card, Connect, Destination, GameAction, GameEventResult, GameMessage, GameState,
    GameVisibility, GameplayState, PlayerDetails, SetupGameOptions, Suit,
};
use components::lobbylist;
use dioxus::prelude::*;
use dioxus_elements::link;
use dotenvy::dotenv;

mod components;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::TryStreamExt;
use futures_util::{SinkExt, StreamExt};
use manganis::{asset, Asset, ImageAsset, ImageAssetBuilder};
use reqwest::Client;
use reqwest_websocket::WebSocket;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, Level};

// All of our routes will be a variant of this Route enum
// #[derive(Routable, PartialEq, Clone)]
// #[rustfmt::skip]
// pub enum AppRoutes {
//     #[layout(StateProvider)]
//     #[route("/")]
//     Home {},

//     #[nest("/games")]
//     #[route("")]
//     Explorer {},
//     #[route("/:room_code")]
//     GameRoom { room_code: String },
//     // #[route("/game/:room_code")]
//     // Game { room_code: String },
// }

#[derive(Clone, Debug)]
struct AppProps {
    username: String,
    lobby_code: String,
    client_secret: String,
    server_url: String,
    server_base_url: String,
    server_ws_url: String,
    environment: Env,
    debug_mode: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Env {
    Production,
    Development,
}

#[derive(Debug, Deserialize, Serialize)]
enum InnerMessage {
    UpdateWsState { new: WsState },
    GameMessage { msg: GameMessage },
    Connect(Connect),
}

#[component]
fn StateProvider() -> Element {
    let mut app_props = use_context_provider(|| {
        let is_prod = option_env!("ENVIRONMENT").unwrap_or("default") == "production";
        let mut server_url =
            String::from("https://blackballgame-blackballgame-server.onrender.com");
        let mut server_base_url = String::from("blackballgame-blackballgame-server.onrender.com");
        let mut server_ws_url =
            String::from("wss://blackballgame-blackballgame-server.onrender.com");

        if !is_prod {
            server_url = String::from("http://localhost:8080");
            server_base_url = String::from("localhost:8080");
            server_ws_url = String::from("ws://localhost:8080");
        }

        Signal::new(AppProps {
            username: if is_prod {
                String::new()
            } else {
                String::from("player1")
            },
            // username: String::new(),
            lobby_code: String::new(),
            client_secret: String::new(),
            // server_url: String::from("http://localhost:8080/"),
            server_url: server_url,
            server_base_url: server_base_url,
            server_ws_url: server_ws_url,
            environment: if is_prod {
                Env::Production
            } else {
                Env::Development
            },
            debug_mode: false,
        })
    });

    let mut current_route = use_context_provider(|| Signal::new("Home".to_string()));

    rsx!(Home {})
}

// const _STYLE: &str = manganis::mg!(file("main.css"));
// const _STYLE: &str = manganis::mg!(file("./assets/tailwind.css"));
// Urls are relative to your Cargo.toml file
// const _TAILWIND_URL: &str = manganis::mg!(file("./public/tailwind.css"));
// const _TAILWIND_URL: &str = manganis::mg!(file("/bb-admin/assets/tailwind.css"));
// const TAILWIND_URL: &str = asset!("./assets/tailwind.css").bundled;
const TAILWIND_URL: Asset = asset!("./assets/tailwind.css");

// const __TAILWIND_URL: &str = manganis::mg!(file("./public/tailwind.css"));

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("Starting app");
    launch(|| {
        rsx! {
            link { rel: "stylesheet", href: asset!("./assets/tailwind.css") }
            // head {
            //     link { rel: "stylesheet", href: "{TAILWIND_URL.bundled}" }
            // }
            // link::Head { rel: "stylesheet", href: asset!("./assets/style.css") }
            // Router::<AppRoutes> {}
            // Home {}

            StateProvider {}
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum WsState {
    Pause,
    Resume,
}

fn get_title_logo() -> Element {
    rsx!(
        div { class: "grid items-center justify-center",
            h1 { class: "col-start-1 row-start-1 text-8xl md:text-6xl font-extrabold text-transparent bg-clip-text bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 drop-shadow-lg animate-gradient-shine",
                "Blackball"
            }
            div { class: "inset-0 w-[300px] h-[300px] bg-black justify-self-center rounded-full z-1 col-start-1 row-start-1" }
        }
    )
}

const TEST: bool = false;

#[component]
fn Home() -> Element {
    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();

    let mut disabled = use_signal(|| true);

    let ws_send: Coroutine<InnerMessage> = use_coroutine(|mut rx| async move {});
    let ws_send_signal = use_signal(|| ws_send);

    let current_component = match current_route.read().as_str() {
        "Home" => rsx!(
            div { class: "flex flex-col items-center justify-center text-center w-dvw h-dvh bg-[--bg-color]",
                {get_title_logo()},
                div { class: "flex flex-col",
                    div { class: "flex flex-row items-center justify-center p-2 gap-2",
                        label { class: "text-2xl", "Username" }
                        input {
                            class: "w-full text-2xl font-semibold text-gray-100 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 placeholder-gray-400 shadow-md transition duration-200 transform hover:scale-105",
                            r#type: "text",
                            value: "{app_props.read().username}",
                            oninput: move |event| {
                                info!(
                                    "Got username len, val: {}, {} - {}", event.value().len(), event.value(),
                                    disabled.read()
                                );
                                if event.value().len() >= 3 {
                                    info!("Username length is good");
                                    disabled.set(false);
                                    app_props.write().username = event.value();
                                } else {
                                    disabled.set(true);
                                    app_props.write().username = event.value();
                                }
                            }
                        }
                    }
                    button {
                        class: "bg-green-400 text-xl rounded-md border border-solid w-full",
                        // to: AppRoutes::Explorer {},
                        onclick: move |_| {
                            current_route.set("Explorer".to_string());
                        },
                        "Play"
                    }
                }
            }
        ),
        "Explorer" => rsx!(Explorer {}),
        "GameRoom" => rsx!(GameRoom {
            room_code: app_props.read().lobby_code.clone()
        }),
        _ => rsx!(Home {}),
    };

    if !TEST {
        rsx!({ current_component })
    } else {
        let mut gamestate = GameState::new(String::from("test"));

        gamestate.add_player(
            "player1".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0".to_string(),
        );
        gamestate.add_player(
            "player2".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0".to_string(),
        );
        gamestate.process_event(GameMessage {
            username: "player1".to_string(),
            lobby: "lobby".to_string(),
            action: GameAction::StartGame(SetupGameOptions {
                rounds: 4,
                deterministic: true,
                start_round: Some(3),
                max_players: 4,
                game_mode: "Standard".to_string(),
                visibility: GameVisibility::Public,
                password: None,
            }),

            timestamp: Utc::now(),
        });
        if let Some(x) = gamestate.players.get_mut(&"player1".to_string()) {
            x.hand = vec![
                // *x.hand = vec![
                Card::new(Suit::Club, 5),
                Card::new(Suit::Club, 14),
                Card::new(Suit::Club, 1),
                Card::new(Suit::Club, 10),
            ];
        }
        gamestate.curr_played_cards = vec![
            Card::new(Suit::Club, 5),
            Card::new(Suit::Heart, 14),
            Card::new(Suit::Diamond, 1),
            Card::new(Suit::Spade, 10),
        ];
        let mut gamestate_signal = use_signal(|| GameState::new(String::from("test")));

        rsx!(GameStateComponent {
            gamestate: gamestate_signal,
            ws_send: ws_send_signal
        })
    }
}

#[component]
fn Explorer() -> Element {
    let mut ws_action = use_signal(|| WsState::Pause);
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();

    let mut lobby_name = use_signal(|| String::new());

    let mut lobbies = use_signal(|| GetLobbiesResponse { lobbies: vec![] });

    use_effect(move || {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    "/rooms"
                ))
                .send()
                .await;

            match resp {
                Ok(data) => {
                    lobbies.set(
                        data.json::<GetLobbiesResponse>()
                            .await
                            .expect("Failed to parse lobbies"),
                    );
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    lobbies.set(GetLobbiesResponse { lobbies: vec![] });
                }
            }
        });
    });

    let refresh_lobbies = move |_| {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    "/rooms"
                ))
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    lobbies.set(
                        data.json::<GetLobbiesResponse>()
                            .await
                            .expect("Failed to refresh lobbies"),
                    );
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    lobbies.set(GetLobbiesResponse { lobbies: vec![] });
                }
            }
        });
    };

    let create_lobby_function = move || {
        #[derive(Deserialize, Serialize)]
        pub struct CreateGameRequest {
            lobby_code: String,
        }

        let cloned_room_code = lobby_name.clone();
        spawn(async move {
            let resp = reqwest::Client::new()
                .post(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    "/rooms"
                ))
                .json(&CreateGameRequest {
                    lobby_code: cloned_room_code.read().clone(),
                })
                .send()
                .await;

            match resp {
                Ok(data) => {
                    info!("create_lobby success");
                    // log::info!("Got response: {:?}", resp);
                    create_lobby_response_msg.set(format!("response: {:?}", data).into());
                }
                Err(err) => {
                    info!("create_lobby failed");
                    // log::info!("Request failed with error: {err:?}")
                    create_lobby_response_msg.set(format!("{err}").into());
                }
            }
        });
    };

    rsx! {

        div { class: "flex flex-row text-center w-dvw h-dvh bg-[--bg-color] items-baseline flex-nowrap justify-center gap-2 p-4",
            div { class: "flex flex-col justify-center align-top max-w-[600px] border border-black rounded-md p-4",
                span { class: "text-lg font-bold", "Create a new lobby for others to join" }
                div { class: "flex flex-row justify-center text-center w-full",
                    label { class: "text-xl", "Lobby name" }
                    input {
                        class: "input",
                        r#type: "text",
                        value: "{lobby_name.read()}",
                        oninput: move |event| lobby_name.set(event.value()),
                        "lobby"
                    }
                }
                // Link {
                //     to: AppRoutes::GameRoom {
                //         room_code: lobby_name.read().to_string(),
                //     },
                //     class: "bg-yellow-400 border border-solid border-black text-center rounded-md",
                //     "Create lobby"
                // }
                button {
                    // to: AppRoutes::GameRoom {
                    //     room_code: lobby_name.read().to_string(),
                    // },
                    class: "bg-yellow-400 border border-solid border-black text-center rounded-md",
                    onclick: move |_| {
                        create_lobby_function();
                    },
                    "Create lobby"
                }
                {if create_lobby_response_msg() == String::from("") { rsx!() } else { rsx!(div { "{create_lobby_response_msg.read()}" }) }}
            }
            div { class: "flex flex-col justify-center align-top max-w-[600px] border border-black rounded-md p-4",
                div { class: "border border-solid border-black bg-white rounded-md",
                    LobbyList { lobbies: lobbies.read().lobbies.clone(), refresh_lobbies }
                }
            }
        }
    }
}

#[component]
pub fn LobbyComponent(lobby: Lobby) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();

    rsx!(
        tr { key: "{lobby.name}",
            td { class: "px-6 py-4 whitespace-nowrap", "{lobby.name}" }
            td { class: "px-6 py-4 whitespace-nowrap", "{lobby.players.len()}/{lobby.max_players}" }
            td { class: "px-6 py-4 whitespace-nowrap", "{lobby.game_mode}" }
            td { class: "px-6 py-4 whitespace-nowrap",
                // button {
                //     onclick: move |evt| join_lobby.call(lobby.clone()),
                //     disabled: true,
                //     class: "px-4 py-2 rounded-md text-sm font-medium bg-yellow-300",
                //     "Join lobby"
                // }
                button {
                    // to: GameRoom {
                    //     room_code: lobby.name.clone(),
                    // },

                    onclick: move |evt| {
                        app_props.write().lobby_code = lobby.name.clone();
                        current_route.set("GameRoom".to_string());
                    },
                    class: "px-4 py-2 rounded-md text-sm font-medium bg-yellow-300",
                    "Join lobby"
                }
            }
        }
    )
}

#[component]
pub fn LobbyList(lobbies: Vec<Lobby>, refresh_lobbies: EventHandler) -> Element {
    let lobby = String::from("test");
    rsx!(
        div { class: "container mx-auto p-4",
            div { class: "flex flex-row justify-center gap-2 space-between",
                h1 { class: "text-2xl font-bold mb-4", "Game Lobbies" }
                button {
                    class: "bg-gray-300 flex flex-row text-center border p-1 border-solid border-black rounded-md justify-center items-center cursor-pointer",
                    onclick: move |evt| refresh_lobbies.call(()),
                    svg {
                        class: "w-6 h-6",
                        fill: "none",
                        stroke: "currentColor",
                        "stroke-width": "1",
                        "view-box": "0 0 24 24",
                        path {
                            "stroke-linecap": "round",
                            "stroke-linejoin": "round",
                            d: "M4 4v5h.582c.523-1.838 1.856-3.309 3.628-4.062A7.978 7.978 0 0112 4c4.418 0 8 3.582 8 8s-3.582 8-8 8a7.978 7.978 0 01-7.658-5.125c-.149-.348-.54-.497-.878-.365s-.507.537-.355.885A9.956 9.956 0 0012 22c5.523 0 10-4.477 10-10S17.523 2 12 2c-2.045 0-3.94.613-5.514 1.653A6.978 6.978 0 004.582 4H4z"
                        }
                    }
                    label { class: "text-lg", "Refresh" }
                }
            }
            div { class: "flex flex-col mb-4",
                svg {
                    class: "absolute translate-x-[10px] translate-y-[10px] text-gray-400 h-5 w-5",
                    "xmlns": "http://www.w3.org/2000/svg",
                    height: "24",
                    "stroke-linejoin": "round",
                    "viewBox": "0 0 24 24",
                    "stroke-width": "2",
                    "fill": "none",
                    "stroke-linecap": "round",
                    "stroke": "currentColor",
                    width: "24",
                    class: "lucide lucide-search",
                    circle { "r": "8", "cx": "11", "cy": "11" }
                    path { "d": "m21 21-4.3-4.3" }
                }
                input {
                    r#type: "text",
                    placeholder: "Search lobbies...",
                    value: "",
                    // onChange: move || {},
                    class: "w-full pl-10 pr-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                }
                div { class: "overflow-x-auto",
                    table { class: "min-w-full bg-white border border-gray-300",
                        thead {
                            tr { class: "bg-gray-100",
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Lobby Name"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Players"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Game Mode"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Action"
                                }
                            }
                        }
                        tbody { class: "divide-y divide-gray-200",
                            {lobbies.iter().map(|lobby| {
                                rsx!(
                                    LobbyComponent {
                                        lobby: lobby.clone(),
                                    }
                                )
                            })
                            }
                        }
                    }
                }
            }
        }
    )
}

#[component]
fn GameRoom(room_code: String) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut server_message = use_signal(|| Value::Null);
    let mut gamestate = use_signal(|| GameState::new(room_code.clone()));
    let mut setupgameoptions = use_signal(|| SetupGameOptions {
        rounds: 4,
        deterministic: true,
        start_round: None,
        max_players: 4,
        game_mode: "Standard".to_string(),
        visibility: GameVisibility::Public,
        password: None,
    });

    let mut ws_url = use_signal(|| {
        String::from(format!(
            "{}/rooms/{}/ws",
            app_props.read().server_ws_url.clone(),
            room_code.clone()
        ))
    });

    let mut get_lobby_response = use_signal(|| GetLobbyResponse {
        lobby: Lobby {
            name: room_code.clone(),
            players: vec![],
            max_players: 4,
            game_mode: "Standard".to_string(),
        },
    });

    let mut error = use_signal(|| Value::Null);
    let mut player_secret = use_signal(|| String::new());

    let mut server_websocket_listener: Signal<Option<SplitStream<WebSocket>>> = use_signal(|| None);
    let mut server_websocket_sender: Signal<Option<SplitSink<WebSocket, Message>>> =
        use_signal(|| None);
    let mut ws_action = use_signal(|| WsState::Resume);

    let mut create_lobby_response_msg = use_signal(|| String::from(""));
    let room_code_clone = room_code.clone();

    use_effect(move || {
        spawn(async move {
            info!("Attempting to connect to websocket server during startup: {server_websocket_listener:?}");
            if server_websocket_listener.try_read().is_err() {
                info!("[SERVER-LISTENER] Server websocket listener already exists");
                return;
            }

            let response = Client::default()
                .get(ws_url())
                .upgrade() // Prepares the WebSocket upgrade.
                .send()
                .await
                .expect("Failed to connect to websocket");

            // Turns the response into a WebSocket stream.
            let mut websocket = response
                .into_websocket()
                .await
                .expect("Failed to upgrade to websocket");
            let (mut ws_tx, mut ws_rx) = websocket.split();
            server_websocket_listener.set(Some(ws_rx));
            server_websocket_sender.set(Some(ws_tx));

            // listen_for_server_messages.send(("ready".to_string()));
            info!("Successfully connected to websocket server");
        });
    });

    let get_details_room_code = room_code_clone.clone();
    let get_game_details = move |get_details_room_code: String| {
        info!("Getting details of game: {get_details_room_code}");
        spawn(async move {
            let resp = reqwest::Client::new()
                .get(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    format!("/rooms/{}", get_details_room_code)
                ))
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    match data.json::<GetLobbyResponse>().await {
                        Ok(resp) => get_lobby_response.set(resp),
                        Err(err) => error.set(json!(format!("Failed to parse lobby: {:?}", err))),
                    }
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    get_lobby_response.set(GetLobbyResponse {
                        lobby: Lobby {
                            name: get_details_room_code.clone(),
                            players: vec![],
                            max_players: 4,
                            game_mode: "Standard".to_string(),
                        },
                    });
                }
            }
        });
    };

    let listen_for_server_messages =
        use_coroutine(move |mut rx: UnboundedReceiver<String>| async move {
            info!("[SERVER-LISTENER] listen_for_server_messages coroutine starting...");
            let _ = rx.next().await;

            info!("[SERVER-LISTENER] Unpaused server websocket listener");

            if server_websocket_listener.read().is_none() {
                info!("[SERVER-LISTENER] Server websocket listener is not available...");
            }

            if server_websocket_listener.read().is_some() {
                info!("[SERVER-LISTENER] Server websocket listener already exists");
            }
            let mut ws_server_listener: Write<SplitStream<WebSocket>> = server_websocket_listener
                .as_mut()
                .expect("[SERVER-LISTENER] No websocket listener");
            let mut error_count = 0;
            while error_count < 10 {
                while let Some(Ok(Message::Text(message))) = ws_server_listener.next().await {
                    info!("[SERVER-LISTENER] Got messages: {:?}", message);

                    // if let Message::Text(text) = message {
                    //     info!("received: {text}");

                    //     let mut is_gamestate = false;
                    match serde_json::from_str::<GameEventResult>(&message) {
                        Ok(ger) => {
                            // is_gamestate = true;
                            match ger.msg {
                                common::GameActionResponse::Connect(con) => {
                                    info!("Got connect message: {con:?}");
                                    app_props.write().client_secret =
                                        con.secret.unwrap_or(String::new());
                                }
                                common::GameActionResponse::GameState(gs) => {
                                    info!("Got game state: {gs:?}");
                                    gamestate.set(gs);
                                }
                                common::GameActionResponse::Message(text) => {
                                    info!("Got message: {text}");
                                }
                            }
                        }
                        Err(err) => {
                            info!(
                                "[SERVER-LISTENING] Failed to parse server message: {:?}",
                                err
                            );
                        }
                    };
                }
                info!("[SERVER-LISTENER] Got error {}, retrying", error_count);
                error_count += 1;
            }
            info!("[SERVER-LISTENER] ended server listener.")
        });

    // this is internal messaging, between frontend to connection websocket
    let ws_send: Coroutine<InnerMessage> = use_coroutine(move |mut rx| async move {
        info!("ws_send coroutine starting...");

        info!("Ready to listen to player actions");
        'pauseloop: while let Some(internal_msg) = rx.next().await {
            info!("Received internal message: {:?}", internal_msg);
            if server_websocket_sender.read().is_none() {
                info!("No websocket sender");
                return;
            }
            let mut ws_server_sender = server_websocket_sender
                .as_mut()
                .expect("No websocket sender");
            info!("Received internal message: {:?}", internal_msg);
            match internal_msg {
                InnerMessage::UpdateWsState { new } => ws_action.set(new),
                InnerMessage::GameMessage { msg } => {
                    if ws_action() == WsState::Pause {
                        continue 'pauseloop;
                    }

                    let _ = ws_server_sender
                        .send(Message::Text(json!(msg).to_string()))
                        .await;
                }
                InnerMessage::Connect(con) => {
                    let _ = ws_server_sender
                        .send(Message::Text(json!(con).to_string()))
                        .await;
                }
            }
            info!("Finished processing action, waiting for next...");
        }
        info!("Finished listening to player actions");
    });

    let ws_send_signal = use_signal(|| ws_send);
    rsx!(
        div { class: "items-center flex flex-col",
            {
                if error().is_null() {
                    rsx!()
                } else {
                    error
                        .read()
                        .as_str()
                        .map(|err| rsx!(div { "{err}" }))
                        .expect("Failed to parse error")
                }
            },
            div {
                "Debug details:"
                div { class: " bg-gray-300", "Secret: {app_props.read().client_secret}" }
                div { class: " bg-gray-300", "Game: {gamestate():#?}" }
            }
            {
                if gamestate().gameplay_state == GameplayState::Pregame {
                    rsx!(
                        div {
                            class: "flex flex-row",
                            div {
                                class: "flex flex-col max-w-[600px] border border-black rounded-md p-4",
                                button {
                                    class: "button",
                                    onclick: move |evt| get_game_details(room_code_clone.clone()),
                                    "Refresh player list"
                                }
                                button {
                                    class: "button",
                                    onclick: move |evt| {
                                        // let button_room_code_clone = room_code_clone.clone();

                                        async move {
                                            info!("Clicked join game");
                                            listen_for_server_messages.send(("ready".to_string()));
                                            ws_send
                                                .send( InnerMessage::GameMessage {
                                                    msg: GameMessage {
                                                        username: app_props().username.clone(),
                                                        timestamp: Utc::now(),
                                                        action: GameAction::JoinGame(
                                                            PlayerDetails{
                                                                username: app_props.read().username.clone(),
                                                                ip: String::new(),
                                                                client_secret: app_props.read().client_secret.clone(),
                                                            }),
                                                            lobby: app_props.read().lobby_code.clone(),
                                                        }
                                                });
                                        }
                                    },
                                    "Join this game"
                                }
                                div {
                                    class: "flex flex-row justify-center align-top text-center items-center max-w-[600px] border border-black rounded-md p-4",
                                    h1 {class: "lg", "{get_lobby_response.read().lobby.name}" }
                                    div { class: "container", "Players ({get_lobby_response.read().lobby.players.len()})"
                                        {get_lobby_response.read().lobby.players.iter().enumerate().map(|(i, player)| rsx!(div { "{i}: {player}" }))}
                                    }
                                }
                            }
                            div { class: "flex flex-col max-w-[600px] border border-black rounded-md p-4",
                                h2 {
                                    class: "lg",
                                    "Game options"
                                }
                                // settings
                                div { class: "flex flex-col align-middle justify-center text-center w-full",
                                    div { class: "flex flex-row items-center",
                                        label { "Rounds" }
                                        input {
                                            r#type: "text",
                                            "data-input-counter": "false",
                                            placeholder: "9",
                                            required: "false",
                                            value: "{setupgameoptions.read().rounds}",
                                            class: "",
                                        }
                                        button {
                                            "data-input-counter-decrement": "quantity-input",
                                            r#type: "button",
                                            class: "bg-gray-100 dark:bg-gray-700 dark:hover:bg-gray-600 dark:border-gray-600 hover:bg-gray-200 border border-gray-300 rounded-s-lg p-3 h-11 focus:ring-gray-100 dark:focus:ring-gray-700 focus:ring-2 focus:outline-none",
                                            id: "decrement-button",
                                            onclick: move |evt| setupgameoptions.write().rounds -= 1,
                                            svg {
                                                "viewBox": "0 0 18 2",
                                                "fill": "none",
                                                "xmlns": "http://www.w3.org/2000/svg",
                                                "aria-hidden": "true",
                                                class: "w-3 h-3 text-gray-900 dark:text-white",
                                                path {
                                                    "stroke-width": "2",
                                                    "d": "M1 1h16",
                                                    "stroke": "currentColor",
                                                    "stroke-linecap": "round",
                                                    "stroke-linejoin": "round"
                                                }
                                            }
                                        }
                                        button {
                                            "data-input-counter-increment": "quantity-input",
                                            r#type: "button",
                                            class: "bg-gray-100 dark:bg-gray-700 dark:hover:bg-gray-600 dark:border-gray-600 hover:bg-gray-200 border border-gray-300 rounded-e-lg p-3 h-11 focus:ring-gray-100 dark:focus:ring-gray-700 focus:ring-2 focus:outline-none",
                                            id: "increment-button",
                                            onclick: move |_| setupgameoptions.write().rounds += 1,
                                            svg {
                                                "xmlns": "http://www.w3.org/2000/svg",
                                                "aria-hidden": "true",
                                                "fill": "none",
                                                "viewBox": "0 0 18 18",
                                                class: "w-3 h-3 text-gray-900 dark:text-white",
                                                path {
                                                    "stroke-linejoin": "round",
                                                    "stroke": "currentColor",
                                                    "stroke-width": "2",
                                                    "stroke-linecap": "round",
                                                    "d": "M9 1v16M1 9h16"
                                                }
                                            }
                                        }
                                    }
                                    div {
                                        class: "flex flex-row align-middle justify-center text-center",
                                        span { "Public" }
                                        label {
                                            class: "relative items-center cursor-pointer",
                                            div {
                                                class: "relative",
                                                input {
                                                    checked: "{setupgameoptions.read().visibility == GameVisibility::Private}",
                                                    class: "sr-only",
                                                    r#type: "checkbox",
                                                    onchange: move |evt| {
                                                        setupgameoptions.write().visibility = if setupgameoptions.read().visibility == GameVisibility::Private { GameVisibility::Public } else { GameVisibility::Private };
                                                    },
                                                }
                                                div {
                                                    class: format!("block w-14 h-8 rounded-full {}", if setupgameoptions.read().visibility == GameVisibility::Private { "bg-red-300" } else { "bg-green-200" }) }
                                                div {
                                                    class: format!("absolute left-1 top-1 bg-white w-6 h-6 rounded-full transition-transform duration-300 ease-in-out {}", if setupgameoptions.read().visibility == GameVisibility::Private { "transform translate-x-full" } else { "" })
                                                }
                                            }
                                        }
                                        span { "Private" }

                                    }
                                    {if setupgameoptions.read().visibility == GameVisibility::Private {
                                        rsx!(
                                            div {
                                                class: "flex flex-row align-middle justify-center text-center",
                                                "Password"
                                                input {
                                                    r#type: "text",
                                                    placeholder: "",
                                                    required: "false",
                                                    value: if setupgameoptions.read().password.is_some() {"{setupgameoptions.read().password:?}"} else {""},
                                                    class: "bg-gray-50 border-x-0 border-gray-300 h-11 text-center text-gray-900 text-sm focus:ring-blue-500 focus:border-blue-500 block w-full py-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500",
                                                    // id: "quantity-input"
                                                    onchange: move |evt| {
                                                        setupgameoptions.write().password = evt.value().parse::<String>().ok();
                                                    }
                                                }
                                            }
                                        )
                                    } else {
                                        rsx!()
                                    }}
                                }

                            button {
                                class: "bg-yellow-300 border border-solid border-black text-center rounded-md",
                                onclick: move |evt| {
                                    info!("Starting game");
                                    listen_for_server_messages.send(("ready".to_string()));
                                    ws_send
                                        .send(InnerMessage::GameMessage { msg: GameMessage {
                                                username: app_props().username.clone(),
                                                timestamp: Utc::now(),
                                                action: GameAction::JoinGame(
                                                    PlayerDetails{
                                                        username: app_props.read().username.clone(),
                                                        ip: String::new(),
                                                        client_secret: app_props.read().client_secret.clone(),
                                                    }),
                                                lobby: app_props.read().lobby_code.clone(),
                                            }
                                        });
                                    ws_send
                                        .send(
                                            InnerMessage::GameMessage {
                                                msg: GameMessage {
                                                    username: app_props.read().username.clone(),
                                                    action: GameAction::StartGame(setupgameoptions()),
                                                    lobby: app_props.read().lobby_code.clone(),
                                                    timestamp: Utc::now(),
                                            }});
                                },
                                "Start game"
                            }
                            div {
                                class: "flex flex-col",
                                {if gamestate().system_status.len() > 0 {
                                    rsx!(
                                        ul {
                                            {gamestate().system_status.iter().map(|issue| rsx!(li { "{issue}" }))}
                                        }
                                    )
                                    } else {
                                        rsx!(div { "Please join the game" })
                                    }
                                }
                            }
                        }
                        }
                    )
                } else {
                    rsx!{GameStateComponent { gamestate, ws_send: ws_send_signal }}
                }
            }
        }
    )
}

pub const CARD_ASSET: manganis::ImageAsset = asset!("./assets/outline.png").image();
// pub const CARD_BG_SVG: manganis::ImageAsset =
//     manganis::mg!(image("./assets/outline.svg").format(ImageType::Svg));
pub const SUIT_CLUB: ImageAsset = asset!("./assets/suits/club.png").image();
pub const SUIT_HEART: manganis::ImageAsset = asset!("./assets/suits/heart.png").image();
pub const SUIT_DIAMOND: manganis::ImageAsset = asset!("./assets/suits/diamond.png").image();
pub const SUIT_SPADE: manganis::ImageAsset = asset!("./assets/suits/spade.png").image();
pub const SUIT_NOTRUMP: ImageAsset = asset!("./assets/suits/notrump.png").image();

#[component]
fn CardComponent(card: Card, onclick: EventHandler<Card>) -> Element {
    let suit = match card.suit {
        Suit::Heart => SUIT_HEART,
        Suit::Diamond => SUIT_DIAMOND,
        Suit::Club => SUIT_CLUB,
        Suit::Spade => SUIT_SPADE,
        Suit::NoTrump => SUIT_NOTRUMP,
    };

    let textvalue = match card.value {
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        14 => "A".to_string(),
        val => val.to_string().clone(),
    };

    let suit_svg = get_trump_svg(&card.suit);

    rsx!(
        button {
            class: "h-[120px] gap-2 grid justify-center text-center",
            onclick: move |evt| {
                onclick(card.clone());
            },
            svg {
                class: "col-start-1 row-start-1 w-full h-full",
                "shape-rendering": "crispEdges",
                "viewBox": "0 -0.5 48 64",
                "xmlns": "http://www.w3.org/2000/svg",
                meta { data: "false" }
                "Made with Pixels to Svg https://codepen.io/shshaw/pen/XbxvNj"
                path {
                    "d": "M0 0h48M0 1h48M0 2h48M0 3h4M44 3h4M0 4h3M4 4h39M45 4h3M0 5h3M4 5h39M46 5h2M0 6h3M4 6h39M46 6h2M0 7h3M4 7h39M46 7h2M0 8h3M4 8h39M46 8h2M0 9h3M4 9h39M46 9h2M0 10h3M4 10h39M46 10h2M0 11h3M4 11h39M46 11h2M0 12h3M4 12h39M46 12h2M0 13h3M4 13h39M46 13h2M0 14h3M4 14h39M46 14h2M0 15h3M4 15h39M46 15h2M0 16h3M4 16h39M46 16h2M0 17h3M4 17h39M46 17h2M0 18h3M4 18h39M46 18h2M0 19h3M4 19h39M46 19h2M0 20h3M4 20h39M46 20h2M0 21h3M4 21h39M46 21h2M0 22h3M4 22h39M46 22h2M0 23h3M4 23h39M46 23h2M0 24h3M4 24h39M46 24h2M0 25h3M4 25h39M46 25h2M0 26h3M4 26h39M46 26h2M0 27h3M4 27h39M46 27h2M0 28h3M4 28h39M46 28h2M0 29h3M4 29h39M46 29h2M0 30h3M4 30h39M46 30h2M0 31h3M4 31h39M46 31h2M0 32h3M4 32h39M46 32h2M0 33h3M4 33h39M46 33h2M0 34h3M4 34h39M46 34h2M0 35h3M4 35h39M46 35h2M0 36h3M4 36h39M46 36h2M0 37h3M4 37h39M46 37h2M0 38h3M4 38h39M46 38h2M0 39h3M4 39h39M46 39h2M0 40h3M4 40h39M46 40h2M0 41h3M4 41h39M46 41h2M0 42h3M4 42h39M46 42h2M0 43h3M4 43h39M46 43h2M0 44h3M4 44h39M46 44h2M0 45h3M4 45h39M46 45h2M0 46h3M4 46h39M46 46h2M0 47h3M4 47h39M46 47h2M0 48h3M4 48h39M46 48h2M0 49h3M4 49h39M46 49h2M0 50h3M4 50h39M46 50h2M0 51h3M4 51h39M46 51h2M0 52h3M4 52h39M46 52h2M0 53h3M4 53h39M46 53h2M0 54h3M4 54h39M46 54h2M0 55h3M4 55h39M46 55h2M0 56h3M4 56h39M46 56h2M0 57h3M4 57h39M46 57h2M0 58h3M4 58h39M46 58h2M0 59h3M46 59h2M0 60h4M46 60h2M0 61h5M45 61h3M0 62h48M0 63h48",
                    "stroke": "#ffffff"
                }
                path {
                    "d": "M4 3h1M28 3h7M39 3h3M43 3h1M43 4h1M43 11h1M3 13h1M43 21h1M3 24h1M3 25h1M3 27h1M3 28h1M3 29h1M43 32h1M3 36h1M3 37h1M3 40h1M3 41h1M3 42h1M43 42h1M3 43h1M3 44h1M3 45h1M3 46h1M3 47h1M3 48h1M43 50h1M43 51h1M3 52h1M3 53h1M3 54h1M3 55h1M3 59h1M6 59h2M14 59h2M29 59h4M34 59h9",
                    "stroke": "#000000"
                }
                path {
                    "d": "M5 3h23M35 3h4M42 3h1M3 4h1M3 5h1M43 5h1M3 6h1M43 6h1M3 7h1M43 7h1M3 8h1M43 8h1M3 9h1M43 9h1M3 10h1M43 10h1M3 11h1M3 12h1M43 12h1M43 13h1M3 14h1M43 14h1M3 15h1M43 15h1M3 16h1M43 16h1M3 17h1M43 17h1M3 18h1M43 18h1M3 19h1M43 19h1M3 20h1M43 20h1M3 21h1M3 22h1M43 22h1M3 23h1M43 23h1M43 24h1M43 25h1M3 26h1M43 26h1M43 27h1M43 28h1M43 29h1M3 30h1M43 30h1M3 31h1M43 31h1M3 32h1M3 33h1M43 33h1M3 34h1M43 34h1M3 35h1M43 35h1M43 36h1M43 37h1M3 38h1M43 38h1M3 39h1M43 39h1M43 40h1M43 41h1M43 43h1M43 44h1M43 45h1M43 46h1M43 47h1M43 48h1M3 49h1M43 49h1M3 50h1M3 51h1M43 52h1M43 53h1M43 54h1M43 55h1M3 56h1M43 56h1M3 57h1M43 57h1M3 58h1M43 58h1M4 59h2M8 59h6M16 59h13M33 59h1",
                    "stroke": "#010101"
                }
                path {
                    "stroke": "#807f7f",
                    "d": "M44 4h1M44 5h1M44 6h1M44 7h1M44 8h1M44 9h1M44 10h1M44 11h1M44 12h1M44 13h1M44 14h1M44 15h1M44 16h1M44 17h1M44 18h1M44 19h1M44 20h1M44 21h1M44 22h1M44 23h1M44 24h1M44 25h1M44 26h1M44 27h1M44 28h1M44 29h1M44 30h1M44 31h1M44 32h1M44 33h1M44 34h1M44 35h1M44 36h1M44 37h1M44 38h1M44 39h1M44 40h1M44 41h1M44 42h1M44 43h1M44 44h1M44 45h1M44 46h1M44 47h1M44 48h1M44 49h1M44 50h1M44 51h1M44 52h1M44 53h1M44 54h1M44 55h1M44 56h1M44 57h1M44 58h1M44 59h1M4 60h40"
                }
                path {
                    "d": "M45 5h1M45 6h1M45 7h1M45 8h1M45 9h1M45 10h1M45 11h1M45 12h1M45 13h1M45 14h1M45 15h1M45 16h1M45 17h1M45 18h1M45 19h1M45 20h1M45 21h1M45 22h1M45 23h1M45 24h1M45 25h1M45 26h1M45 27h1M45 28h1M45 29h1M45 30h1M45 31h1M45 32h1M45 33h1M45 34h1M45 35h1M45 36h1M45 37h1M45 38h1M45 39h1M45 40h1M45 41h1M45 42h1M45 43h1M45 44h1M45 45h1M45 46h1M45 47h1M45 48h1M45 49h1M45 50h1M45 51h1M45 52h1M45 53h1M45 54h1M45 55h1M45 56h1M45 57h1M45 58h1M45 59h1M44 60h2M5 61h12M18 61h2M21 61h20M42 61h3",
                    "stroke": "#d7d2d2"
                }
                path { "stroke": "#7f7e7e", "d": "M43 59h1" }
                path { "stroke": "#d8d2d2", "d": "M17 61h1M20 61h1M41 61h1" }
                {suit_svg}
            }
            span { class: "text-white content-center text-center text-5xl self-center h-full col-start-1 justify-center row-start-1 drop-shadow-[0_2.2px_2.2px_rgba(0,0,0,0.8)]",
                "{textvalue}"
            }
        }
    )
}

#[component]
fn GameStateComponent(
    gamestate: Signal<GameState>,
    ws_send: Signal<Coroutine<InnerMessage>>,
) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();

    let trump_svg = get_trump_svg(&gamestate.read().trump);
    let curr_player = gamestate
        .read()
        .curr_player_turn
        .clone()
        .unwrap_or("".to_string());
    let curr_hand = if gamestate.read().players.contains_key(&curr_player) {
        Some(
            gamestate
                .read()
                .players
                .get(&curr_player)
                .expect("Failed to get player in gamestate")
                .encrypted_hand
                .clone(),
        )
    } else {
        None
    };

    rsx!(
        div { class: "flex flex-col w-dvw h-dvh bg-[--bg-color] items-center gap-4",
            div { class: " bg-gray-300",
                div { class: "flex content-between",
                    div {

                        h2 { "Phase: {gamestate().gameplay_state:?}" }
                        div {
                            "Trump: {gamestate().trump:?}"
                            {trump_svg}
                        }
                        ol {
                            {gamestate().player_order.iter().map(|player| rsx!(li { class: "player-turn", "{player}" }))}
                        }
                        div { "Round: {gamestate().curr_round}/{gamestate().setup_game_options.rounds}" }
                        div { "Dealer: {gamestate().curr_dealer}" }
                        if gamestate().curr_player_turn.is_some() {
                            // div { "{gamestate().curr_player_turn.unwrap()}" }
                            div { "Player turn: TODO" }
                        } else {
                            div { "Player turn: None" }
                        }
                    }
                    div { class: "flex flex-row",
                        h2 { "Players" }
                        {gamestate().players.iter().map(|(playername, client)| {
                            let wins = gamestate().wins.get(playername).unwrap_or(&0).clone();
                            let bid = gamestate().bids.get(playername).unwrap_or(&0).clone();
                            rsx!(
                                div {
                                    class: "container-row",
                                    div { "{playername}" }
                                    div { "{wins}/{bid}" }
                                    div { "Score: {bid}" }
                                }
                            )
                        })}
                    }
                }
            }

            div { class: "relative w-full md:w-1/2 bg-[var(--bg-color)] rounded-lg p-4 shadow-lg text-gray-100 border border-black",
                div { class: "absolute top-2 left-2 px-3 py-1 text-sm font-bold text-white bg-indigo-600 rounded-md shadow",
                    "Played cards"
                }
                div { class: "grid grid-rows-1 gap-4 mt-8",
                    {gamestate().curr_played_cards.iter().map(|card| rsx!(
                        CardComponent {
                            onclick: move |_| { info!("Clicked a card: {:?}", "fake card") },
                            card: card.clone()
                        }
                    ))}
                }
            }
            if gamestate().curr_player_turn.unwrap_or("".to_string()) == app_props.read().username {
                {rsx!(div {
                    class: "container-row turn-indicator",
                    "Your turn"
                })}
            }
            div { class: "relative w-full md:w-1/2 bg-[var(--bg-color)] rounded-lg p-4 shadow-lg text-gray-100 border border-black",
                div { class: "absolute top-2 left-2 px-3 py-1 text-sm font-bold text-white bg-yellow-700 rounded-md shadow",
                    "Your hand"
                }
                div { class: "grid grid-rows-1 gap-4 mt-8",
                    {if curr_hand.is_none() {
                        rsx!()
                    } else {
                        rsx!({GameState::decrypt_player_hand(curr_hand.unwrap(), &app_props.read().client_secret.clone())
                            .iter()
                            .map(|card| {
                                return rsx!(CardComponent {
                                    onclick: move |clicked_card: Card| {
                                        ws_send().send(InnerMessage::GameMessage {
                                            msg: GameMessage {
                                                username: app_props.read().username.clone(),
                                                action: GameAction::PlayCard(clicked_card),
                                                timestamp: Utc::now(),
                                                lobby: app_props.read().lobby_code.clone(),
                                            },
                                        });
                                    },
                                    card: card.clone()
                                });
                            })})
                        }
                    }
                }
            }
            if gamestate().gameplay_state == GameplayState::Bid {
                div { class: "flex flex-col items-center",
                    label { class: "text-2xl p-2", "How many hands do you want to win" }
                    ul { class: "flex flex-row gap-2 items-center p-2",
                        {(0..=gamestate().curr_round).map(|i| {
                            rsx!(
                                button {
                                    class: "bg-yellow-300 p-4 rounded-lg",
                                    onclick: move |_| {
                                        info!("Clicked on bid {i}");
                                        ws_send().send(InnerMessage::GameMessage {
                                            msg: GameMessage {
                                                username: app_props.read().username.clone(),
                                                action: GameAction::Bid(i),
                                                lobby: app_props.read().lobby_code.clone(),
                                                timestamp: Utc::now(),
                                    }});
                                },
                                    "{i}"
                                },
                            )
                            })
                        }
                    }
                }
            }
            {if let GameplayState::PostHand(ps) = gamestate().gameplay_state {
                rsx!(
                    div {
                        class: "container",
                        button {
                            class: "button",
                            onclick: move |_| {
                                ws_send()
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props.read().username.clone(),
                                            action: GameAction::Ack,
                                            lobby: app_props.read().lobby_code.clone(),
                                            timestamp: Utc::now(),
                                        },
                                    });
                            },
                            "Acknowledge"
                        }
                    }
                )
            } else {
                rsx!()
            }},
            {if let GameplayState::PostRound = gamestate().gameplay_state {
                rsx!(
                    div {
                        class: "container",
                        button {
                            class: "button",
                            onclick: move |_| {
                                ws_send()
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props.read().username.clone(),
                                            action: GameAction::Ack,
                                            lobby: app_props.read().lobby_code.clone(),
                                            timestamp: Utc::now(),
                                        },
                                    });
                            },
                            "Acknowledge"
                        }
                    }
                )
            } else {
                rsx!()
            }},
            {if let GameplayState::End = gamestate().gameplay_state {
                rsx!(
                    div {
                        class: "container",
                        div {"GAME OVER"}
                        {gamestate().score.iter().map(|(player, score)| {rsx!(li { "{player}: {score}" })})}
                    }
                    div {
                        class: "container",
                        button {
                            class: "button",
                            onclick: move |_| {
                                ws_send()
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props.read().username.clone(),
                                            action: GameAction::Ack,
                                            lobby: app_props.read().lobby_code.clone(),
                                            timestamp: Utc::now(),
                                        },
                                    });
                            },
                            "Acknowledge"
                        }
                    }
                )
            } else {
                rsx!()
            }}
        }
    )
}

fn get_trump_svg(trump: &Suit) -> Element {
    let trump_svg = match trump {
        Suit::Spade => rsx!(
            svg {
                "fill": "none",
                "xmlns": "http://www.w3.org/2000/svg",
                height: "40",
                width: "40",
                "viewBox": "0 0 100 100",
                x: "15",
                y: "10",
                ellipse {
                    "cy": "43.5",
                    "rx": "25",
                    "cx": "25",
                    "ry": "43.5",
                    "fill": "black"
                }
                rect {
                    "y": "40",
                    "x": "19",
                    width: "12",
                    "fill": "black",
                    height: "68"
                }
            }
        ),
        Suit::Heart => rsx!(
            svg {
                "fill": "none",
                "xmlns": "http://www.w3.org/2000/svg",
                height: "40",
                "viewBox": "0 0 101 103",
                width: "40",
                x: "5",
                y: "10",
                ellipse {
                    "rx": "25",
                    "cx": "76",
                    "cy": "25",
                    "ry": "25",
                    "transform": "rotate(180 76 25)",
                    "fill": "#FF0000"
                }
                path {
                    "fill": "#FF0000",
                    "d": "M0 25C0 11.1929 11.1929 -3.8147e-06 25 -3.8147e-06C38.8071 -3.8147e-06 50 11.1929 50 25C50 38.8071 38.8071 50 25 50C11.1929 50 0 38.8071 0 25Z"
                }
                path {
                    "d": "M50.5 99.5L97 37.9291L53.5 14L50.5 18.5L47.5 14L4 37.9291L50.5 99.5Z",
                    "fill": "#FF0000"
                }
            }
        ),
        Suit::Diamond => rsx!(
            svg {
                width: "40",
                height: "40",
                "xmlns": "http://www.w3.org/2000/svg",
                "fill": "none",
                "viewBox": "0 0 114 114",
                x: "4",
                y: "10",
                rect {
                    width: "80",
                    height: "80",
                    "y": "56.5685",
                    "fill": "#FF0000",
                    "transform": "rotate(-45 0 56.5685)"
                }
            }
        ),
        Suit::Club => rsx!(
            svg {
                "xmlns": "http://www.w3.org/2000/svg",
                "viewBox": "0 0 100 108",
                height: "40",
                width: "40",
                "fill": "none",
                x: "5",
                y: "10",
                circle {
                    "fill": "black",
                    "r": "25",
                    "cx": "25",
                    "cy": "62"
                }
                circle {
                    "cx": "75",
                    "cy": "62",
                    "r": "25",
                    "fill": "black"
                }
                circle {
                    "fill": "black",
                    "cy": "25",
                    "cx": "50",
                    "r": "25"
                }
                rect {
                    "y": "40",
                    "x": "44",
                    width: "12",
                    height: "68",
                    "fill": "black"
                }
            }
        ),
        Suit::NoTrump => rsx!(
            svg {
                "fill": "none",
                height: "40",
                width: "40",
                "xmlns": "http://www.w3.org/2000/svg",
                "viewBox": "0 0 114 114",
                rect {
                    "fill": "#F3ADCF",
                    height: "112.137",
                    "y": "0.499969",
                    "x": "0.500031",
                    width: "112.137"
                }
                rect {
                    "x": "0.500031",
                    "y": "0.499969",
                    width: "112.137",
                    height: "112.137",
                    "stroke": "black"
                }
                g { "filter": "url(#filter0_d_13_7)",
                    rect {
                        "x": "3.05176e-05",
                        height: "80",
                        "rx": "25",
                        "transform": "rotate(-45 3.05176e-05 56.5685)",
                        width: "80",
                        "fill": "white",
                        "y": "56.5685"
                    }
                }
                defs {
                    filter {
                        "filterUnits": "userSpaceOnUse",
                        "y": "10.3553",
                        "color-interpolation-filters": "sRGB",
                        width: "100.426",
                        "x": "6.35538",
                        height: "100.426",
                        id: "filter0_d_13_7",
                        feFlood {
                            "flood-opacity": "0",
                            "result": "BackgroundImageFix"
                        }
                        feColorMatrix {
                            "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0",
                            "result": "hardAlpha",
                            "in": "SourceAlpha",
                            r#type: "matrix"
                        }
                        feOffset { "dy": "4" }
                        feGaussianBlur { "stdDeviation": "2" }
                        feComposite { "operator": "out", "in2": "hardAlpha" }
                        feColorMatrix {
                            "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.25 0",
                            r#type: "matrix"
                        }
                        feBlend {
                            "mode": "normal",
                            "in2": "BackgroundImageFix",
                            "result": "effect1_dropShadow_13_7"
                        }
                        feBlend {
                            "mode": "normal",
                            "in2": "effect1_dropShadow_13_7",
                            "in": "SourceGraphic",
                            "result": "shape"
                        }
                    }
                }
            }
        ),
    };

    return trump_svg;
}
