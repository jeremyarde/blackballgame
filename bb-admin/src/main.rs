#![allow(non_snake_case)]
#![allow(warnings)]

use std::{collections::HashMap, path::Path};

use api_types::{GetLobbiesResponse, GetLobbyResponse};
use chrono::Utc;
use common::{
    Card, Connect, Destination, GameAction, GameClient, GameEvent, GameEventResult, GameMessage,
    GameState, GameplayState, InternalMessage, PlayState, PlayerDetails, PlayerSecret,
    SetupGameOptions,
};
use dioxus::prelude::*;
use dotenvy::dotenv;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::TryStreamExt;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use reqwest_websocket::WebSocket;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, Level};

// All of our routes will be a variant of this Route enum
#[derive(Routable, PartialEq, Clone)]
#[rustfmt::skip]
enum Route {
    // #[route("/")]
    // //  if the current location doesn't match any of the other routes, redirect to "/home"
    // #[redirect("/:..segments", |segments: Vec<String>| Route::Home {})]
    // Home {},
    #[layout(StateProvider)]
    #[route("/")]
    Home {},

    #[nest("/games")]
    #[route("/")]
    Explorer {},
    #[route("/:room_code")]
    GameRoom { room_code: String },
    // #[route("/game/:room_code")]
    // Game { room_code: String },
}

#[derive(Clone, Debug)]
struct AppProps {
    username: String,
    lobbyCode: String,
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
        Signal::new(AppProps {
            username: String::new(),
            lobbyCode: String::new(),
        })
    });

    rsx!(Outlet::<Route> {})
}

// static APP_STATE: OnceLock<FrontendAppState> = OnceLock::new();
// static APP_STATE: GlobalSignal<FrontendAppState> = Signal::global(|| FrontendAppState::new());
// const STYLE: &str = manganis::mg!(file("./assets/main.css")); // this works but does not reload nicely

const STYLE: &str = include_str!("../assets/main.css");

fn main() {
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

// #[derive(Clone, Debug)]
// enum InternalMessage {
//     Game(GameMessage),
//     Server(Connect),
//     WsAction(WsAction),
// }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum WsState {
    Pause,
    Resume,
}

#[component]
fn Home() -> Element {
    // let mut app_props: Signal<AppProps> = use_context_provider(|| {
    //     Signal::new(AppProps {
    //         username: String::new(),
    //         lobbyCode: String::new(),
    //     })
    // });

    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();

    // rsx!(
    //     div { "Blackball" }
    //     Link { class: "bg-orange-300 w-full h-full", to: Route::Explorer {}, "Explore games" }
    //     div {
    //         "Join a game"
    //         label { "Lobby code" }
    //         input {
    //             r#type: "text",
    //             value: "{app_props.read().lobbyCode}",
    //             oninput: move |event| app_props.write().lobbyCode = event.value()
    //         }
    //         Link {
    //             class: "bg-orange-300 w-full h-full",
    //             to: Route::GameRoom {
    //                 room_code: app_props.read().lobbyCode.clone(),
    //             },
    //             "Join game"
    //         }
    //     }
    // )
    rsx!(
        div { class: "w-2/3 h-56 flex flex-col bg-green-100 float-start align-middle ",
            h1 { class: "text-6xl h-full w-full font-bold text-center content-center",
                "Blackball"
            }
            Link {
                class: "text-center bg-orange-300  m-4 h-[200px] rounded-lg shadow-md",
                to: Route::Explorer {},
                div { class: "", "Start or find a game" }
            }
            div { class: "flex flex-row",
                div { class: "flex flex-col",
                    label { "Lobby code" }
                    input {
                        r#type: "text",
                        value: "{app_props.read().lobbyCode}",
                        oninput: move |event| app_props.write().lobbyCode = event.value()
                    }
                }
                Link {
                    class: "text-center bg-orange-300 w-full h-full",
                    to: Route::GameRoom {
                        room_code: app_props.read().lobbyCode.clone(),
                    },
                    div { class: "text-center bg-orange-300 w-full h-full", "Join an existing game" }
                }
            }
        }
    )
}

#[component]
fn Explorer() -> Element {
    let mut ws_action = use_signal(|| WsState::Pause);
    let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();

    let mut lobbies = use_signal(|| GetLobbiesResponse { lobbies: vec![] });

    use_effect(move || {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get("http://localhost:8080/rooms")
                .send()
                .await;

            match resp {
                Ok(data) => {
                    // log::info!("Got response: {:?}", resp);
                    lobbies.set(
                        data.json::<GetLobbiesResponse>()
                            .await
                            .expect("Failed to parse lobbies"),
                    );
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    lobbies.set(GetLobbiesResponse { lobbies: vec![] });
                    // create_lobby_response_msg.set(format!("{err}").into());
                }
            }
        });
    });

    let refresh_lobbies = move |_| {
        spawn(async move {
            let resp = reqwest::Client::new()
                .get("http://localhost:8080/rooms")
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

    rsx! {
        div { class: "flex flex-col outline w-2/3 h-full m-10 content-center text-center",
            div { class: "flex flex-col w-full h-full m-2",
                div { class: "flex flex-row content-center text-center self-center",
                    label { "Enter a username" }
                    input {
                        r#type: "text",
                        value: "{app_props.read().username}",
                        oninput: move |event| {
                            app_props.write().username = event.value();
                        }
                    }
                }
                div { class: "flex flex-row content-center text-center self-center",
                    label { "Lobby" }
                    input {
                        r#type: "text",
                        value: "{app_props.read().lobbyCode}",
                        oninput: move |event| app_props.write().lobbyCode = event.value(),
                        "lobby"
                    }
                }
            }
            // button {
            //     class: "bg-green-300 w-full h-full hover:outline-2 hover:outline hover:outline-green-500",
            //     onclick: move |evt| {
            //         create_lobby(evt.clone());
            //         info!("create_lobby called");
            //         refresh_lobbies(evt);
            //         info!("refresh_lobbies called");
            //     },
            //     "Create lobby"
            // }
            Link {
                to: Route::GameRoom {
                    room_code: app_props.read().lobbyCode.clone(),
                },
                class: "bg-green-300 w-full h-full hover:outline-2 hover:outline hover:outline-green-500",
                "Create lobby"
            }
            {if create_lobby_response_msg() == String::from("") { rsx!() } else { rsx!(div { "{create_lobby_response_msg.read()}" }) }}
        }
        div { class: "flex flex-col bg-green-50 w-2/3",
            button {
                class: "bg-green-300 w-full h-full hover:outline-2 hover:outline hover:outline-green-500",
                onclick: refresh_lobbies,
                "Refresh lobbies"
            }
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
}

#[component]
fn LobbyComponent(lobby: String) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    rsx!(
        div { class: "flex flex-row justify-between",
            div { "{lobby}" }
            button {
                class: "shadow-sm p-6 rounded-md bg-slate-200 hover:bg-slate-300 w-1/2",
                onclick: move |_| {
                    info!("Get details for lobby");
                },
                "Game details"
            }
            Link {
                class: "shadow-sm p-6 rounded-md bg-slate-200 hover:bg-slate-300 w-1/2",
                to: Route::GameRoom {
                    room_code: lobby.clone(),
                },
                onclick: move |_| {
                    info!("Joining {}", & lobby);
                    app_props.write().lobbyCode = lobby.clone();
                },
                "Join game"
            }
        }
    )
}

#[component]
fn GameRoom(room_code: String) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();

    let mut server_message = use_signal(|| Value::Null);
    let mut gamestate = use_signal(|| GameState::new(room_code.clone()));
    let mut ws_url =
        use_signal(|| String::from(format!("ws://0.0.0.0:8080/rooms/{}/ws", room_code.clone())));
    // use_signal(|| String::from(format!("ws://0.0.0.0:8080/games/ws")));

    let mut server_url = use_signal(|| String::from("http://0.0.0.0:8080/"));
    let mut lobby = use_signal(|| GetLobbyResponse {
        lobby_code: room_code.clone(),
        players: vec![],
    });
    // let mut username = use_signal(|| String::new());
    let mut error = use_signal(|| Value::Null);
    let mut player_secret = use_signal(|| String::new());
    let mut num_rounds = use_signal(|| 9);

    let mut server_websocket_listener: Signal<Option<SplitStream<WebSocket>>> = use_signal(|| None);
    let mut server_websocket_sender: Signal<Option<SplitSink<WebSocket, Message>>> =
        use_signal(|| None);
    let mut ws_action = use_signal(|| WsState::Resume);

    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    let room_code_clone = room_code.clone();
    use_effect(move || {
        info!("create_lobby on lobby creation");
        info!(
            "jere/ lobby: {:?}, username: {:?}",
            app_props.read().lobbyCode,
            app_props.read().username
        );
        #[derive(Deserialize, Serialize)]
        pub struct CreateGameRequest {
            lobby_code: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .post("http://localhost:8080/rooms")
                .json(&CreateGameRequest {
                    lobby_code: app_props.read().lobbyCode.clone(),
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

            info!("Attempting to connect to websocket server during startup");
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

    let listen_for_server_messages =
        use_coroutine(|mut rx: UnboundedReceiver<String>| async move {
            info!("[SERVER-LISTENER] listen_for_server_messages coroutine starting...");
            let _ = rx.next().await; // waiting for start message
                                     // while server_websocket_listener.read().is_none() {
                                     //     info!("No websocket listener, waiting...");
                                     //     // sleep(Duration::from_millis(5000));
                                     //     // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                     // }

            // if server_websocket_listener.read().is_none() {
            //     info!("No websocket listener");
            //     return;
            // }

            info!("[SERVER-LISTENER] Unpaused server websocket listener");

            if server_websocket_listener.read().is_some() {
                info!("Server websocket listener already exists");
            }

            let mut ws_server_listener: Write<SplitStream<WebSocket>> = server_websocket_listener
                .as_mut()
                .expect("[SERVER-LISTENER] No websocket listener");
            while let Some(Ok(Message::Text(message))) = ws_server_listener.next().await {
                info!("[SERVER-LISTENER] Got messages: {:?}", message);

                // if let Message::Text(text) = message {
                //     info!("received: {text}");

                //     let mut is_gamestate = false;
                match serde_json::from_str::<GameEventResult>(&message) {
                    Ok(ger) => {
                        // is_gamestate = true;
                        if ger.game_state.is_some() {
                            info!("[SERVER-LISTENING] Got game state: {:?}", ger);
                            let new_gs = ger.game_state.expect("Was not able to parse game state");
                            gamestate.set(new_gs);
                        }
                        // gamestate.set();
                    }
                    Err(err) => {
                        info!(
                            "[SERVER-LISTENING] Failed to parse server message: {:?}",
                            err
                        );
                    }
                };

                //     if is_gamestate {
                //         return;
                //     }

                //     match serde_json::from_str::<PlayerSecret>(&text) {
                //         Ok(x) => {
                //             player_secret.set(x.client_secret);
                //             info!("player_secret: {player_secret}");
                //             // server_message.set(x);
                //         }
                //         Err(err) => info!("Failed to parse server message: {}", err),
                //     };
                // }
            }
            info!("[SERVER-LISTENER] ended server listener.")
        });

    // this is internal messaging, between frontend to connection websocket
    let ws_send: Coroutine<InnerMessage> = use_coroutine(|mut rx| async move {
        info!("ws_send coroutine starting...");

        info!("Ready to listen to player actions");
        'pauseloop: while let Some(internal_msg) = rx.next().await {
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

                    let im: InternalMessage = InternalMessage::ToGame {
                        dest: common::Destination::Lobby(app_props.read().lobbyCode.clone()),
                        msg: msg,
                    };

                    let _ = ws_server_sender
                        .send(Message::Text(json!(im).to_string()))
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

    // let start_ws = move || async move {
    //     info!("Attempting to connect to websocket server");
    //     // Connect to some sort of service
    //     // Creates a GET request, upgrades and sends it.

    //     // if server_websocket_listener.read().is_some() {
    //     //     info!("Server websocket listener already exists");
    //     //     return;
    //     // }

    //     let response = Client::default()
    //         .get(ws_url())
    //         .upgrade() // Prepares the WebSocket upgrade.
    //         .send()
    //         .await
    //         .expect("Failed to connect to websocket");

    //     // Turns the response into a WebSocket stream.
    //     let mut websocket = response
    //         .into_websocket()
    //         .await
    //         .expect("Failed to upgrade to websocket");
    //     let (mut ws_tx, mut ws_rx) = websocket.split();
    //     server_websocket_listener.set(Some(ws_rx));
    //     server_websocket_sender.set(Some(ws_tx));
    //     listen_for_server_messages.send(("ready".to_string()));
    //     info!("Successfully connected to server");
    // };

    rsx!(
        {
            if error().is_null() {
                rsx!()
            } else {
                error.read().as_str().map(|err| rsx!(div { "{err}" }))
            }
        },
        {
            if gamestate().gameplay_state == GameplayState::Pregame {
                rsx!(
                    button {
                        class: "h-full w-full bg-blue-500",
                        onclick: move |evt| get_game_details(app_props.read().lobbyCode.clone()),
                        "Refresh player list"
                    }
                    button {
                        class: "h-full w-full bg-blue-500",
                        onclick: move |evt| {
                            // let room_code_clone = room_code_clone.clone();
                            async move {
                                info!("Clicked join game");
                                listen_for_server_messages.send(("ready".to_string()));
                                ws_send
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props().username.clone(),
                                            message: GameEvent {
                                                action: GameAction::JoinGame(PlayerDetails{ username: app_props.read().username.clone(), ip: String::new() }),
                                                    // app_props().username.clone()),
                                            },
                                            timestamp: Utc::now(),
                                        },
                                    });

                            }
                        },
                        "Join this game",
                    }
                    div { "lobby: {lobby.read().lobby_code}" }
                    div { "Players ({lobby.read().players.len()})" }
                    {lobby.read().players.iter().enumerate().map(|(i, player)| rsx!(div { "{i}: {player}" }))},
                    div { class: "flex flex-col bg-purple-300 h-full w-full text-center",
                        div { class: "text-4xl", "Game options" }
                        div { class: "flex flex-row align-middle w-full",
                            label { "Rounds" }
                            input {
                                r#type: "number",
                                onchange: move |evt| num_rounds.set(evt.value().parse::<usize>().unwrap_or(9)),
                                value: "{num_rounds}"
                            }
                        }
                    }
                    button {
                        class: "h-full w-full bg-blue-500",
                        onclick: move |evt| {
                            info!("Starting game");
                            ws_send
                                .send(
                                    InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props.read().username.clone(),
                                            message: GameEvent {
                                                action: GameAction::StartGame(SetupGameOptions {
                                                    rounds: gamestate().setup_game_options.rounds,
                                                    deterministic: false,
                                                    start_round: None,
                                                }),
                                            },
                                            timestamp: Utc::now(),
                                    }});
                        },
                        "Start game"
                    }
                )
            } else {
                rsx!(div { "Game is not in pregame state" })
            }
        },
        {
            if gamestate()
                .players
                .get(&app_props.read().username)
                .is_some()
            {
                let curr_hand = gamestate
                    .read()
                    .players
                    .get(&app_props.read().username)
                    .expect("Player not found")
                    .encrypted_hand
                    .clone();
                let decrypted_hand =
                    GameState::decrypt_player_hand(curr_hand, &player_secret.read().clone());
                rsx!(
                        div { class: "bg-green-300 w-full h-full",
                    div { "This is the game" }
                    div { "State: {gamestate().gameplay_state:?}" }
                    div { "Trump: {gamestate().trump:?}" }
                    div {
                        ol { {gamestate().player_order.iter().map(|player| rsx!(li { "{player}" }))} }
                    }
                    div { "Round: {gamestate().curr_round}" }
                    div { "Dealer: {gamestate().curr_dealer}" }
                    div { "Player turn: {gamestate().curr_player_turn:?}" }
                }
                div { class: "bg-blue-300 w-full h-full",
                    "Play area"
                    div {
                        "Cards"
                        {gamestate().curr_played_cards.iter().map(|card| rsx!(li { "{card}" }))}
                    }
                }
                div { class: "bg-red-300 w-full h-full",
                    "Action area"
                    div {
                        "My cards"
                        {decrypted_hand.iter().map(|card| rsx!(li { "{card:?}" }))}
                    }
                    if gamestate().gameplay_state == GameplayState::Bid {
                        div { class: "flex justify-center m-4",
                            label { "Bid" }
                            ol { class: "flex flex-row",
                                {(0..gamestate().curr_round).into_iter().map(|i| {
                                    rsx!(
                                        li { key: "{i}",
                                            button {
                                                class: "w-24 h-10 border border-solid rounded-md bg-slate-100",
                                                onclick: move |_| {
                                                    info!("Clicked on bid {i}");
                                                    // send_bid(*i);
                                                    // ws_send.send(InternalMessage::Game(GameMessage {
                                                    //     username: app_props.read().username.clone(),
                                                    //     message: GameEvent {
                                                    //         action: GameAction::Bid(i),
                                                    //     },
                                                    //     timestamp: Utc::now(),
                                                    // }));
                                                    ws_send.send(InnerMessage::GameMessage {
                                                        msg: GameMessage {
                                                            username: app_props.read().username.clone(),
                                                            message: GameEvent {
                                                                action: GameAction::Bid(i),
                                                            },
                                                            timestamp: Utc::now(),
                                                }});
                                            },
                                                "{i}"
                                            }
                                        }
                                    )
                                })}
                            }
                        }
                    }
                }
                    )
            } else {
                rsx!(div { "Press start when all players have joined to begin" })
            }
        }
    )
}

#[component]
fn GameState(player_secret: Signal<String>, gamestate: Signal<GameState>) -> Element {
    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();

    fn create_action(username: String, action: GameAction) -> GameMessage {
        return GameMessage {
            username: username.clone(),
            message: GameEvent { action },
            timestamp: Utc::now(),
        };
    };
    info!("GameState: {gamestate:?}");
    let curr_hand = gamestate
        .read()
        .players
        .get(&app_props.read().username)
        .expect("Player not found")
        .encrypted_hand
        .clone();
    info!("curr_hand: {curr_hand:?}");
    info!("player_secret: {:?}", player_secret.read());
    let decrypted_hand = GameState::decrypt_player_hand(curr_hand, &player_secret.read().clone());
    info!("decrypted_hand: {decrypted_hand:?}");
    rsx!(
        div { class: "bg-green-300 w-full h-full",
            div { "This is the game" }
            div { "State: {gamestate().gameplay_state:?}" }
            div { "Trump: {gamestate().trump:?}" }
            div {
                ol { {gamestate().player_order.iter().map(|player| rsx!(li { "{player}" }))} }
            }
            div { "Round: {gamestate().curr_round}" }
            div { "Dealer: {gamestate().curr_dealer}" }
            div { "Player turn: {gamestate().curr_player_turn:?}" }
        }
        div { class: "bg-blue-300 w-full h-full",
            "Play area"
            div {
                "Cards"
                {gamestate().curr_played_cards.iter().map(|card| rsx!(li { "{card}" }))}
            }
        }
        div { class: "bg-red-300 w-full h-full",
            "Action area"
            div {
                "My cards"
                {decrypted_hand.iter().map(|card| rsx!(li { "{card:?}" }))}
            }
            if gamestate().gameplay_state == GameplayState::Bid {
                div { class: "flex justify-center m-4",
                    label { "Bid" }
                    ol { class: "flex flex-row",
                        {(0..gamestate().curr_round).into_iter().map(|i| {
                            rsx!(
                                li { key: "{i}",
                                    button {
                                        class: "w-24 h-10 border border-solid rounded-md bg-slate-100",
                                        onclick: move |_| {
                                            info!("Clicked on bid {i}");
                                            // send_bid(*i);
                                        },
                                        "{i}"
                                    }
                                }
                            )
                        })}
                    }
                }
            }
        }
    )
}

#[component]
fn PlayerComponent(player: GameClient) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();

    info!("PlayerComponent: {}", player.id);
    rsx!(
        div { class: "bg-blue-300 w-full h-full",
            div { "this is a player" }
            div { "{player.id}" }
        }
    )
}
