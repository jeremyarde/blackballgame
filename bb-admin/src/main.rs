#![allow(non_snake_case)]
#![allow(warnings)]

use std::{collections::HashMap, path::Path};

use api_types::{GetLobbiesResponse, GetLobbyResponse};
use chrono::Utc;
use common::{
    Card, Connect, Destination, GameAction, GameClient, GameEvent, GameEventResult, GameMessage,
    GameState, GameplayState, InternalMessage, PlayState, PlayerDetails, PlayerSecret,
    SetupGameOptions, Suit,
};
use dioxus::prelude::*;
use dioxus_elements::link;
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

// static ASSET_PATH: &str = "bb-admin/assets/spade.svg";

// All of our routes will be a variant of this Route enum
#[derive(Routable, PartialEq, Clone)]
#[rustfmt::skip]
enum Route {
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
    lobby_code: String,
    client_secret: String,
    server_url: String,
    server_base_url: String,
    server_ws_url: String,
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
            username: String::new(),
            lobby_code: String::new(),
            client_secret: String::new(),
            // server_url: String::from("http://localhost:8080/"),
            server_url: server_url,
            server_base_url: server_base_url,
            server_ws_url: server_ws_url,
        })
    });

    rsx!(
        Outlet::<Route> {}
    )
}

const _STYLE: &str = manganis::mg!(file("./main.css"));

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    // launch(App);
    launch(|| {
        rsx! {
            // head {
            //     // link { rel: "stylesheet", href: "./bb-admin/assets/main.css" }
            // }
            // link::Head { rel: "stylesheet", href: asset!("./assets/style.css") }
            Router::<Route> {}
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum WsState {
    Pause,
    Resume,
}

#[component]
fn Home() -> Element {
    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();
    let mut disabled = use_signal(|| true);

    rsx!(
        div { class: "container",
            h1 { class: "header", "Blackball" }
            label { "Enter a username" }
            input {
                class: "input",
                r#type: "text",
                value: "{app_props.read().username}",
                oninput: move |event| {
                    info!(
                        "Got username len, val: {}, {} - {}", event.value().len(), event.value(),
                        disabled.read()
                    );
                    if event.value().len() > 3 {
                        info!("Username length is good");
                        disabled.set(false);
                        app_props.write().username = event.value();
                    } else {
                        disabled.set(true);
                        app_props.write().username = event.value();
                    }
                }
            }
            Link { class: "link disabled_{disabled}", to: Route::Explorer {}, "Start or find a game" }
        }
    )
}

#[component]
fn Explorer() -> Element {
    let mut ws_action = use_signal(|| WsState::Pause);
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();

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

    rsx! {
        div { class: "container",
            label { class: "lg", "Create a lobby" }
            input {
                class: "input",
                r#type: "text",
                value: "{app_props.read().lobby_code}",
                oninput: move |event| app_props.write().lobby_code = event.value(),
                "lobby"
            }
            Link {
                to: Route::GameRoom {
                    room_code: app_props.read().lobby_code.clone(),
                },
                class: "button link",
                "Create lobby"
            }
            {if create_lobby_response_msg() == String::from("") { rsx!() } else { rsx!(div { "{create_lobby_response_msg.read()}" }) }},
            button { class: "", onclick: refresh_lobbies, "Refresh lobbies" }
        }
        div {
            label { class: "lg", "Ongoing games" }
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
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    rsx!(
        div { class: "container-row",
            div { "{lobby}" }
            Link {
                class: "link",
                to: Route::GameRoom {
                    room_code: lobby.clone(),
                },
                onclick: move |_| {
                    info!("Joining {}", & lobby);
                    app_props.write().lobby_code = lobby.clone();
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
    let mut ws_url = use_signal(|| {
        String::from(format!(
            "{}/rooms/{}/ws",
            app_props.read().server_ws_url.clone(),
            room_code.clone()
        ))
    });

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
            app_props.read().lobby_code,
            app_props.read().username
        );
        #[derive(Deserialize, Serialize)]
        pub struct CreateGameRequest {
            lobby_code: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .post(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    "/rooms"
                ))
                .json(&CreateGameRequest {
                    lobby_code: app_props.read().lobby_code.clone(),
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

            if server_websocket_listener.try_read().is_err() {
                info!("[SERVER-LISTENER] Server websocket listener already exists");
                return;
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
                .get(format!(
                    "{}{}",
                    app_props.read().server_url.clone(),
                    format!("/rooms/{}", room_code)
                ))
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
                        msg: msg,
                        lobby_code: app_props.read().lobby_code.clone(),
                        from: Destination::User(PlayerDetails {
                            username: app_props.read().username.clone(),
                            ip: String::new(),
                            client_secret: Some(app_props.read().username.clone()),
                        }),
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
                        class: "button",
                        onclick: move |evt| get_game_details(app_props.read().lobby_code.clone()),
                        "Refresh player list"
                    }
                    button {
                        class: "button",
                        onclick: move |evt| {
                            // let room_code_clone = room_code_clone.clone();
                            async move {
                                info!("Clicked join game");
                                listen_for_server_messages.send(("ready".to_string()));
                                ws_send
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props().username.clone(),
                                            timestamp: Utc::now(),
                                            message: GameEvent {
                                                action: GameAction::JoinGame(
                                                    PlayerDetails{
                                                        username: app_props.read().username.clone(),
                                                        ip: String::new(),
                                                        client_secret: Some(app_props.read().client_secret.clone()),
                                                    })
                                            },
                                        },
                                    });
                            }
                        },
                        "Join this game",
                    }
                    div {
                        class: "container",
                        h1 {class: "lg", "{lobby.read().lobby_code}" }
                        div { class: "container", "Players ({lobby.read().players.len()})"
                            {lobby.read().players.iter().enumerate().map(|(i, player)| rsx!(div { "{i}: {player}" }))},
                        }
                        div { class: "container",
                            h2 {
                                class: "lg",
                                "Game options"
                            }
                            div { class: "flex flex-row align-middle w-full",
                                label { "Rounds" }
                                input {
                                    class: "input",
                                    r#type: "number",
                                    onchange: move |evt| num_rounds.set(evt.value().parse::<usize>().unwrap_or(9)),
                                    value: "{num_rounds}"
                                }
                            }
        
                        button {
                            class: "button lg",
                            onclick: move |evt| {
                                info!("Starting game");
                                listen_for_server_messages.send(("ready".to_string()));
                                ws_send
                                    .send(InnerMessage::GameMessage {
                                        msg: GameMessage {
                                            username: app_props().username.clone(),
                                            timestamp: Utc::now(),
                                            message: GameEvent {
                                                action: GameAction::JoinGame(
                                                    PlayerDetails{
                                                        username: app_props.read().username.clone(),
                                                        ip: String::new(),
                                                        client_secret: Some(app_props.read().client_secret.clone()),
                                                    })
                                            },
                                        },
                                    });
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
                    }
                    div {
                        class: "container",
                        h2 {"System messages"}
                        {if gamestate().system_status.len() > 0 {
                            rsx!(
                                ul {
                                {gamestate().system_status.iter().map(|issue| rsx!(li { "{issue}" }))}
                            }
                        )
                        } else {
                            rsx!(
                            div { "Join the game first" }
                        )}
                    }
                    }
                }
                )
            } else {
                rsx!(div {})
            }
        },
        {
            if gamestate().gameplay_state != GameplayState::Pregame {
                let curr_hand = gamestate
                    .read()
                    .players
                    .get(&app_props.read().username)
                    .expect("Player not found")
                    .encrypted_hand
                    .clone();
        
                let is_turn_css = if gamestate()
                    .curr_player_turn
                    .unwrap_or("".to_string())
                    .eq(&app_props.read().username)
                {
                    "bg-green-300"
                } else {
                    "bg-slate-100"
                };
        
                let is_turn_outline_css = if gamestate()
                    .curr_player_turn
                    .unwrap_or("".to_string())
                    .eq(&app_props.read().username)
                {
                    "outline outline-4 outline-yellow-300"
                } else {
                    ""
                };
        
                rsx!(
                    div { class: "bg-red-500 w-full h-full", "My app props: {app_props.read():?}" }
                    div { class: "gameinfo",
                        div { "State: {gamestate().gameplay_state:?}" }
                        div { "Trump: {gamestate().trump:?}" }
                        ol { {gamestate().player_order.iter().map(|player| rsx!(li { class: "player-turn", "{player}" }))} }
                        div { "Round: {gamestate().curr_round}" }
                        div { "Dealer: {gamestate().curr_dealer}" }
                        div { "Player turn: {gamestate().curr_player_turn:?}" }
                    }
                    div { class: "scores",
                        h2 { "Scores " }
                        {gamestate().score.iter().map(|(player, bid)| {
                            rsx!(
                                div {
                                    class: "container-row",
                                    div { "Player: {player}" }
                                    div { "Bid: {bid}" }
                                }
                            )
                        })}
                    }
                    div { class: "bids",
                        h2 { "Bids" }
                        {gamestate().bids.iter().map(|(player, bid)| {
                                rsx!(
                                    div {
                                        class: "container-row",
                                        div { "Player: {player}" }
                                        div { "Bid: {bid}" }
                                    }
                                )
                            })}
                    }
                    div { class: "bg-blue-300 w-full h-full",
                        "Play area"
                        div { class: "w-full h-[100px] flex flex-row relative justify-center",
                            {gamestate().curr_played_cards.iter().map(|card| rsx!(
                                CardComponent {
                                    onclick: move |_| { info!("Clicked a card: {:?}", "fake card") },
                                    card: card.clone()
                                }
                            ))}
                        }
                    }
                    div { class: "",
                        "Action area"
                        div { class: "card-area",
                            {GameState::decrypt_player_hand(
                                curr_hand,
                                &app_props.read().client_secret.clone(),
                            ).iter().map(|card| {
                                return rsx!(
                                    CardComponent {
                                        onclick: move |clicked_card: Card| {
                                            ws_send.send(InnerMessage::GameMessage {
                                                msg: GameMessage {
                                                    username: app_props.read().username.clone(),
                                                    message: GameEvent { action: GameAction::PlayCard(clicked_card) },
                                                    timestamp: Utc::now()}
                                            });
                                        },
                                        card: card.clone()
                                    }
                                );
                            })}
                        }
                        if gamestate().gameplay_state == GameplayState::Bid {
                            div { class: "container",
                                label { class: "lg center", "How many hands do you want to win" }
                                ul { class: "bid-list",
                                    {(0..=6).map(|i| {
                                        rsx!(
                                            button {
                                                class: "bid-item is-selected",
                                                onclick: move |_| {
                                                    info!("Clicked on bid {i}");
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
                                    class: "content-center text-center",
                                    button {
                                        class: "bg-green-300 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded shadow-lg",
                                        onclick: move |_| {
                                            ws_send
                                                .send(InnerMessage::GameMessage {
                                                    msg: GameMessage {
                                                        username: app_props.read().username.clone(),
                                                        message: GameEvent {
                                                            action: GameAction::Ack,
                                                        },
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
                        {if let GameplayState::PostRound = gamestate().gameplay_state {
                            rsx!(
                                div {
                                    class: "content-center text-center",
                                    button {
                                        class: "bg-green-300 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded shadow-lg ",
                                        onclick: move |_| {
                                            ws_send
                                                .send(InnerMessage::GameMessage {
                                                    msg: GameMessage {
                                                        username: app_props.read().username.clone(),
                                                        message: GameEvent {
                                                            action: GameAction::Ack,
                                                        },
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
                })
            } else {
                rsx! {div {"Something went wrong"}}
            }
        }
    )
}

pub const CARD_ASSET: manganis::ImageAsset =
    manganis::mg!(image("./assets/outline.png").size(96, 128));
pub const SUIT_HEART: manganis::ImageAsset = manganis::mg!(image("./assets/suits/heart.png"));
pub const SUIT_DIAMOND: manganis::ImageAsset = manganis::mg!(image("./assets/suits/diamond.png"));
pub const SUIT_CLUB: manganis::ImageAsset = manganis::mg!(image("./assets/suits/club.png"));
pub const SUIT_SPADE: manganis::ImageAsset = manganis::mg!(image("./assets/suits/spade.png"));
pub const SUIT_NOTRUMP: manganis::ImageAsset = manganis::mg!(image("./assets/suits/notrump.png"));

#[component]
fn CardComponent(card: Card, onclick: EventHandler<Card>) -> Element {
    let suit = match card.suit {
        Suit::Heart => SUIT_HEART,
        Suit::Diamond => SUIT_DIAMOND,
        Suit::Club => SUIT_CLUB,
        Suit::Spade => SUIT_SPADE,
        Suit::NoTrump => SUIT_NOTRUMP,
    };
    rsx!(
        div {
            class: "card",
            onclick: move |evt| {
                onclick(card.clone());
            },
            img { class: "card-bg", src: "{CARD_ASSET}" }
            img { class: "card-suit", src: "{suit}" }
            div { class: "card-value", "{card.value}" }
        }
    )
}
