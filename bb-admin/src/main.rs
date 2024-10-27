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

    rsx!(Outlet::<Route> {})
}

// const _STYLE: &str = manganis::mg!(file("main.css"));
// const _STYLE: &str = manganis::mg!(file("./assets/tailwind.css"));
// Urls are relative to your Cargo.toml file
const _TAILWIND_URL: &str = manganis::mg!(file("./public/tailwind.css"));
// const __TAILWIND_URL: &str = manganis::mg!(file("./public/tailwind.css"));

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

    let ws_send: Coroutine<InnerMessage> = use_coroutine(|mut rx| async move {});
    let ws_send_signal = use_signal(|| ws_send);

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
        message: GameEvent {
            action: GameAction::StartGame(SetupGameOptions {
                rounds: 4,
                deterministic: true,
                start_round: Some(3),
            }),
        },
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
    let mut gamestate = use_signal(|| gamestate);

    // rsx!(
    //     div { class: "title",
    //         h1 { class: "header", "Blackball" }
    //         label { "Enter a username" }
    //         input {
    //             class: "input",
    //             r#type: "text",
    //             value: "{app_props.read().username}",
    //             oninput: move |event| {
    //                 info!(
    //                     "Got username len, val: {}, {} - {}", event.value().len(), event.value(),
    //                     disabled.read()
    //                 );
    //                 if event.value().len() >= 3 {
    //                     info!("Username length is good");
    //                     disabled.set(false);
    //                     app_props.write().username = event.value();
    //                 } else {
    //                     disabled.set(true);
    //                     app_props.write().username = event.value();
    //                 }
    //             }
    //         }
    //         Link { class: "link disabled_{disabled}", to: Route::Explorer {}, "Start or find a game" }
    //     }
    // )

    rsx!(GameStateComponent {
        gamestate,
        ws_send: ws_send_signal
    })
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
        div { class: "container",
            label { class: "lg", "Ongoing games" }
            {if lobbies.read().lobbies.len() == 0 {
                rsx!(div { "No games" })
            } else {
                rsx!{
                    ul {
                            {lobbies.read().lobbies.iter().map(|lobby|
                                {
                                    if lobby.eq("") {
                                        rsx!()
                                    } else {
                                        rsx!(LobbyComponent {lobby: lobby})
                                    }
                                }
                            )}
                    }
                }
            }}
        }
    }
}

#[component]
fn LobbyComponent(lobby: String) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut create_lobby_response_msg = use_signal(|| String::from(""));

    let lobbyclone = lobby.clone();
    rsx!(

        Link {
            class: "lobby-link",
            to: Route::GameRoom {
                room_code: lobby.clone(),
            },
            onclick: move |_| {
                app_props.write().lobby_code = lobbyclone.clone();
            },
            "Join game: {lobby}"
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
                            client_secret: app_props.read().client_secret.clone(),
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

    let ws_send_signal = use_signal(|| ws_send);

    rsx!(
        {
            if error().is_null() {
                rsx!()
            } else {
                error
                    .read()
                    .as_str()
                    .map(|err| rsx!(div { "{err}" }))
                    .unwrap()
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
                                                        client_secret: app_props.read().client_secret.clone(),
                                                    })
                                            },
                                        },
                                    });
                            }
                        },
                        "Join this game"
                    }
                    div {
                        class: "container border",
                        h1 {class: "lg", "{lobby.read().lobby_code}" }
                        div { class: "container", "Players ({lobby.read().players.len()})"
                            {lobby.read().players.iter().enumerate().map(|(i, player)| rsx!(div { "{i}: {player}" }))}
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
                                                        client_secret: app_props.read().client_secret.clone(),
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
                                                        rounds: num_rounds(),
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
        GameStateComponent {
            gamestate,
            ws_send: ws_send_signal
        }
    )
}

pub const CARD_ASSET: manganis::ImageAsset =
    manganis::mg!(image("./assets/outline.png").size(96, 128));
// pub const CARD_BG_SVG: manganis::ImageAsset =
//     manganis::mg!(image("./assets/outline.svg").format(ImageType::Svg));
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

    let textvalue = match card.value {
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        14 => "A".to_string(),
        val => val.to_string().clone(),
    };

    let suit_svg = get_trump_svg(&card.suit);

    rsx!(
        div {
            class: "h-[120px] w-[120px] bg-white border border-black grid justify-center text-center",
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
        gamestate
            .read()
            .players
            .get(&curr_player)
            .unwrap()
            .encrypted_hand
            .clone()
    } else {
        "".to_string()
    };

    rsx!(
        div { class: "flex flex-col w-dvw h-dvh bg-green-100",

            div { class: " bg-gray-300",
                div { class: "flex",
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
                        div { "Player turn: {gamestate().curr_player_turn.unwrap()}" }
                    } else {
                        div { "Player turn: None" }
                    }
                }
                div { class: "gameinfo right",
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

            div { class: "flex flex-col items-center h-[100px] bg-blue-200 w-full h-full",
                div { class: "w-full h-[100px] flex flex-row relative justify-center",
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
            div { class: "container-row",
                div { class: "card-area",
                    {GameState::decrypt_player_hand(
                        curr_hand,
                        &app_props.read().client_secret.clone(),
                    ).iter().map(|card| {
                        return rsx!(
                            CardComponent {
                                onclick: move |clicked_card: Card| {
                                    ws_send().send(InnerMessage::GameMessage {
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
            }
            if gamestate().gameplay_state == GameplayState::Bid {
                div { class: "container",
                    label { class: "lg center", "How many hands do you want to win" }
                    ul { class: "bid-list",
                        {(0..=gamestate().curr_round).map(|i| {
                            rsx!(
                                button {
                                    class: "bid-item is-selected",
                                    onclick: move |_| {
                                        info!("Clicked on bid {i}");
                                        ws_send().send(InnerMessage::GameMessage {
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
                        class: "container",
                        button {
                            class: "button",
                            onclick: move |_| {
                                ws_send()
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
