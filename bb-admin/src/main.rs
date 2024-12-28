#![allow(non_snake_case)]
#![allow(warnings)]

use std::env;
use std::{collections::HashMap, path::Path};

use api_types::{GetLobbiesResponse, GetLobbyResponse, Lobby};
use chrono::Utc;
use common::{
    Card, Connect, Destination, GameAction, GameActionResponse, GameEventResult, GameMessage,
    GameState, GameVisibility, GameplayState, PlayState, PlayerDetails, SetupGameOptions, Suit,
};
use components::lobbylist;
use components::state_provider::state_provider::StateProvider;
use dioxus::prelude::*;
use dioxus_elements::link;
// use dotenvy::dotenv;

mod components;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use futures_util::{TryFutureExt, TryStreamExt};
use gloo_storage::{LocalStorage, Storage};
use manganis::{asset, Asset};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use reqwest_websocket::{UpgradeResponse, WebSocket};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use server_client::server_client::ServerClient;
use tracing::{info, Level};
use websocket::websocket_connection::WebsocketConnection;

mod server_client;
mod websocket;

mod styles {
    pub const INC_DEC_BUTTONS: &str = "bg-gray-100 disabled:bg-red-100 hover:bg-gray-200 border border-gray-300 p-2 h-9 focus:ring-gray-100 focus:ring-2 focus:outline-none";
    pub const TITLE_CONTAINER: &str = "grid items-center justify-center";
    pub const TITLE_TEXT: &str = "col-start-1 row-start-1 text-8xl md:text-6xl font-extrabold \
        text-transparent bg-clip-text bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 \
        drop- animate-gradient-shine";
    pub const INPUT_FIELD: &str =
        "w-full text-2xl font-semibold text-black bg-white border border-gray-700 rounded-md 
         placeholder-gray-500  focus:outline-none focus:ring-2 focus:ring-indigo-500 
         focus:border-indigo-500 hover:scale-102 transition-transform duration-200 ease-in-out text-center";
    pub const ROUND_DETAILS_TAILWIND: &str = "w-full bg-gradient-to-r from-purple-500 to-indigo-500 text-white rounded-lg  text-center p-2 relative z-20";
    pub const STANDARD_BUTTON: &str = "px-4 py-2 bg-gray-800 font-semibold rounded-lg  hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-400 focus:ring-opacity-75";
    pub const BID_BUTTON: &str = "
        px-2.5 py-2 rounded-md text-lg font-semibold text-gray-800
        bg-gradient-to-b from-gray-200 to-gray-400
        hover:from-gray-300 hover:to-gray-500
        active:from-gray-400 active:to-gray-600
        border border-gray-400
        shadow-[inset_0_1px_0_rgba(255,255,255,0.6),0_1px_3px_rgba(0,0,0,0.2)]
        hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.6),0_2px_5px_rgba(0,0,0,0.3)]
        active:shadow-[inset_0_2px_5px_rgba(0,0,0,0.2)]
        transition-all duration-150 ease-in-out
        focus:outline-none focus:ring-2 focus:ring-blue-300 focus:ring-opacity-50
      ";
}

#[derive(Clone, Debug)]
struct AppProps {
    environment: Env,
    debug_mode: bool,
    // current_route: String,
}

impl AppProps {
    fn is_prod(&self) -> bool {
        self.environment == Env::Production
    }

    fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }
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

mod environment;

fn main() {
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("Starting app");
    launch(|| {
        rsx! {
            link { rel: "stylesheet", href: "./assets/tailwind.css" }
            // document::Link { rel: "stylesheet", href: asset!("/public/tailwind.css") }
            StateProvider {}
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum WsState {
    Pause,
    Resume,
}
// STEP 1: Extract environment configuration

#[derive(Clone)]
struct ServerConfig {
    server_url: String,
    server_base_url: String,
    server_ws_url: String,
}

impl ServerConfig {
    fn new(is_prod: bool) -> Self {
        if is_prod {
            Self {
                server_url: String::from("https://bball.fly.dev"),
                server_base_url: String::from("bball.fly.dev"),
                server_ws_url: String::from("wss://bball.fly.dev/ws"),
                // server_url: String::from("https://blackballgame-blackballgame-server.onrender.com"),
                // server_base_url: String::from("blackballgame-blackballgame-server.onrender.com"),
                // server_ws_url: String::from(
                //     "wss://blackballgame-blackballgame-server.onrender.com/ws",
                // ),
            }
        } else {
            Self {
                server_url: String::from("http://localhost:8080"),
                server_base_url: String::from("localhost:8080"),
                server_ws_url: String::from("ws://localhost:8080/ws"),
            }
        }
    }
}

// STEP 4: Split AppProps
#[derive(Clone)]
struct UserConfig {
    username: String,
    lobby_code: String,
    client_secret: String,
}

impl UserConfig {
    fn update_username(&mut self, new_username: String, disabled: &mut Signal<bool>) -> bool {
        self.username = new_username;
        validate_username(&self.username, disabled)
    }
}

// STEP 5: Add WebSocket error handling
#[derive(Debug)]
enum WsError {
    ConnectionFailed,
    MessageSendFailed,
    InvalidMessage,
}

// STEP 6: Extract UI strings
mod constants {
    pub const TITLE: &str = "Blackball";
    pub const USERNAME_LABEL: &str = "Username";
    pub const MIN_USERNAME_LENGTH: usize = 3;
}

use crate::styles::INPUT_FIELD;

const USERNAME_KEY: &str = "bb_username";
const LOBBY_CODE_KEY: &str = "bb_lobby_code";
const CLIENT_SECRET_KEY: &str = "bb_client_secret";

fn validate_username(username: &str, disabled: &mut Signal<bool>) -> bool {
    let is_valid = username.len() >= constants::MIN_USERNAME_LENGTH;
    disabled.set(!is_valid);
    is_valid
}

fn get_title_logo() -> Element {
    rsx!(
        div { class: "grid items-center justify-center",
            h1 { class: "col-start-1 row-start-1 z-10 text-6xl font-extrabold text-transparent bg-clip-text bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 drop-shadow animate-gradient-shine",
                "Blackball"
            }
            div { class: "inset-0 w-[300px] h-[300px] bg-black justify-self-center rounded-full col-start-1 row-start-1" }
        }
    )
}

#[component]
fn Home() -> Element {
    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();
    let mut server_config: Signal<ServerConfig> = use_context::<Signal<ServerConfig>>();

    let mut disabled = use_signal(|| true);
    let mut open_modal = use_signal(|| false);
    let mut show_username_error = use_signal(|| false);
    let ws_send: Coroutine<InnerMessage> = use_coroutine(|mut rx| async move {});
    let ws_send_signal = use_signal(|| ws_send);

    let current_component = match current_route.read().as_str() {
        "Home" => rsx!(
            div { class: "overflow-hidden bg-bg-color flex flex-col items-center justify-center text-center min-h-screen w-full p-10",
                {get_title_logo()}
                div { class: "flex flex-col gap-2 w-full max-w-md",
                    div { class: "flex flex-col sm:flex-row items-center justify-center gap-2",
                        label { class: "text-xl whitespace-nowrap", "Username" }
                        input {
                            class: "{styles::INPUT_FIELD} w-full",
                            r#type: "text",
                            value: "{user_config.read().username}",
                            oninput: move |event| {
                                info!(
                                    "Got username len, val: {}, {} - {}", event.value().len(), event.value(),
                                    disabled.read()
                                );
                                if event.value().len() >= 3 {
                                    info!("Username length is good");
                                    disabled.set(false);
                                    user_config.write().username = event.value();
                                    LocalStorage::set(USERNAME_KEY, event.value());
                                } else {
                                    disabled.set(true);
                                    user_config.write().username = event.value();
                                }
                                if event.value().len() == 0 {
                                    show_username_error.set(true);
                                }
                            },
                        }
                    }
                    button {
                        class: "{styles::STANDARD_BUTTON} w-full bg-green-200 text-black hover:bg-green-400",
                        disabled: if user_config.read().username.is_empty() { true } else { false },
                        onclick: move |_| {
                            current_route.set("Explorer".to_string());
                        },
                        "Play"
                    }
                    if !user_config.read().client_secret.is_empty() {
                        button {
                            class: "{styles::STANDARD_BUTTON} w-full bg-green-200 text-black hover:bg-green-400",
                            disabled: if user_config.read().username.is_empty() { true } else { false },
                            onclick: move |_| {
                                current_route.set("Explorer".to_string());
                            },
                            "Rejoin"
                        }
                    }
                    if user_config.read().username.is_empty() && *show_username_error.read() {
                        span { class: "text-red-500 text-sm ",
                            p { "Please enter a username to play" }
                        }
                    }
                    div { class: "w-full",
                        button {
                            onclick: move |evt| {},
                            class: "{styles::STANDARD_BUTTON} w-full bg-yellow-200 hover:bg-yellow-400",
                            "How to Play"
                        }
                        if open_modal() == true {
                            div { class: "fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50",
                                div { class: "bg-white rounded-lg  max-w-xl w-full max-h-[90vh] overflow-y-auto",
                                    div { class: "p-4 sm:p-6",
                                        h2 { class: "text-xl sm:text-2xl font-bold",
                                            "How to Play"
                                        }
                                        div { class: "space-y-4",
                                            ul { class: "list-disc list-inside space-y-2 text-left text-sm ",
                                                li {
                                                    "Blackball is played with a standard deck of 52 cards."
                                                }
                                                li {
                                                    "The dealer deals each player the same amount of cards as the round number."
                                                }
                                                li {
                                                    "Starting after the dealer, each player bids on the number of hands they want to win based on the cards they were default."
                                                }
                                                li { "The player with the highest bid goes first." }
                                                li {
                                                    "Each round has a "trump
                                                    " suit. Cards with the trump suit are considered higher than any cards when determining the winner. The trump suit rotates every round starting with Hearts, then Diamonds, Clubs, Spades then No Trump (highest card of first played cards suit wins.)"
                                                }
                                                li {
                                                    "Players each play a single card from their hand. The card the player plays must be the same suit as the first card played. If the player does not have a card of that suit, they can play any other card from their hand."
                                                }
                                                li {
                                                    "The player who played the highest card of the first card played each hand's suit, or the highest card of the trump suit wins the hand."
                                                }
                                                li {
                                                    "The winner of the hand plays the first card in the next hand."
                                                }
                                                li {
                                                    "Once all the cards in players hands have been played, players win 10 points plus the number of hands bid if they won the same number of hands, and receive a blackball if they won more or less hands than they bid."
                                                }
                                            }
                                        }
                                    }
                                    button {
                                        onclick: move |evt| open_modal.set(false),
                                        class: "bg-bg-color border border-black text-black w-full p-3 rounded hover:bg-gray-400 transition-colors focus:outline-none focus:ring-2 focus:ring-gray-400 focus:ring-opacity-50",
                                        "Close"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ),
        "Explorer" => rsx!(Explorer {}),
        "GameRoom" => rsx!(GameRoom {
            room_code: user_config.read().lobby_code.clone()
        }),
        _ => rsx!(Home {}),
    };

    if !app_props.read().is_debug_mode() {
        rsx!({ current_component })
    } else {
        let mut gamestate = GameState::new(String::from("test"));

        gamestate.add_player(
            "player1".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0:1111".to_string(),
        );
        gamestate.add_player(
            "player2".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0:1234".to_string(),
        );
        gamestate.add_player(
            "player3".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0:1234".to_string(),
        );
        gamestate.add_player(
            "player4".to_string(),
            common::PlayerRole::Player,
            "0.0.0.0:1234".to_string(),
        );
        let client_secret = gamestate
            .players
            .get("player1")
            .unwrap()
            .details
            .client_secret
            .clone();

        use_effect(move || {
            user_config.write().username = "player1".to_string();
            user_config.write().lobby_code = "test".to_string();
            user_config.write().client_secret = client_secret.clone().unwrap();
        });

        gamestate.process_event(GameMessage {
            username: "player1".to_string(),
            lobby: "lobby".to_string(),
            action: GameAction::StartGame(SetupGameOptions {
                rounds: 9,
                deterministic: false,
                start_round: Some(7),
                max_players: 4,
                game_mode: "Standard".to_string(),
                visibility: GameVisibility::Public,
                password: None,
                computer_players: 0,
            }),
            timestamp: Utc::now(),
        });
        // if let Some(x) = gamestate.players.get_mut(&"player1".to_string()) {
        //     x.hand = vec![
        //         Card::new(Suit::Club, 5),
        //         Card::new(Suit::Club, 14),
        //         Card::new(Suit::Club, 1),
        //       Card::new(Suit::Club, 10),
        //     ];
        // }

        // gamestate.curr_player_turn = Some("player1".to_string());
        gamestate.curr_played_cards = vec![
            Card::with_played_by(Suit::Club, 5, "player_2".to_string()),
            Card::with_played_by(Suit::Heart, 14, "player_1".to_string()),
            Card::with_played_by(Suit::Diamond, 1, "cpu_5".to_string()),
            Card::with_played_by(Suit::Spade, 10, "this is a long name".to_string()),
            Card::with_played_by(Suit::Club, 1, "cpu_1".to_string()),
            Card::with_played_by(Suit::Heart, 12, "cpu_0".to_string()),
            Card::with_played_by(Suit::Diamond, 11, "player1".to_string()),
            Card::with_played_by(Suit::Spade, 7, "player1".to_string()),
        ];
        gamestate.curr_winning_card = Some(Card::new(Suit::Club, 5));
        gamestate.gameplay_state = GameplayState::Bid;
        gamestate.player_bids = vec![("player1".to_string(), 0), ("player2".to_string(), 0)];
        gamestate.event_log = vec![
            GameMessage {
                username: String::from("player1"),
                action: GameAction::Bid(0),
                timestamp: Utc::now(),
                lobby: "testlobby".to_string(),
            },
            GameMessage {
                username: String::from("player1"),
                action: GameAction::Ack,
                timestamp: Utc::now(),
                lobby: "testlobby".to_string(),
            },
        ];
        gamestate.curr_player_turn = Some("player1".to_string());

        let mut gamestate_signal = use_signal(|| gamestate);

        // rsx!(GameStateComponent {
        //     gamestate: gamestate_signal,
        //     ws_send: ws_send_signal
        // })

        rsx!(Explorer {})
        // rsx!(GameRoom {
        //     room_code: user_config.read().lobby_code.clone()
        // })
    }
}

#[component]
fn Explorer() -> Element {
    let mut app_props: Signal<AppProps> = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();
    let mut server_config: Signal<ServerConfig> = use_context::<Signal<ServerConfig>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();
    let mut server_client: Signal<ServerClient> = use_context::<Signal<ServerClient>>();

    let mut lobby_name = use_signal(|| String::new());

    let mut lobbies = use_signal(|| GetLobbiesResponse { lobbies: vec![] });

    // use_effect(move || {
    //     spawn(async move {
    //         let resp = server_client.read().get_rooms().await;
    //         lobbies.set(resp.expect("Failed to get rooms"));
    //     });
    // });

    rsx! {
        div { class: "flex flex-col text-center bg-bg-color flex-nowrap gap-2 p-2 w-screen h-screen overflow-hidden items-center justify-center",
            // div { class: "flex flex-col justify-center align-top w-full h-full border border-black rounded-md p-2  items-start",
            div { class: "border border-solid border-black bg-white rounded-md p-2 md:max-w-[600px] h-full md:max-h-[600px]",
                LobbyList { test_lobbies: lobbies.read().lobbies.clone() }
            }
        }
    }
}

#[component]
pub fn LobbyComponent(lobby: Lobby) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();
    let mut server_config: Signal<ServerConfig> = use_context::<Signal<ServerConfig>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();

    rsx!(
        div { class: "break-words text-center", "{lobby.name}" }
        div { class: "", "{lobby.players.len()}/{lobby.max_players}" }
        div { class: "",
            button {

                onclick: move |evt| {
                    user_config.write().lobby_code = lobby.name.clone();
                    current_route.set("GameRoom".to_string());
                },
                class: "py-2 rounded-md text-sm font-medium w-full bg-yellow-300",
                "Join"
            }
        }
    )
}

#[component]
pub fn LobbyList(test_lobbies: Vec<Lobby>) -> Element {
    let mut server_config: Signal<ServerConfig> = use_context::<Signal<ServerConfig>>();
    let mut server_client: Signal<ServerClient> = use_context::<Signal<ServerClient>>();
    let mut current_route: Signal<String> = use_context::<Signal<String>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();

    let mut create_lobby_response_msg = use_signal(|| String::from(""));
    let mut lobby_name = use_signal(|| String::new());
    let lobby = String::from("test");
    let mut searchterm = use_signal(|| String::new());

    let mut all_lobbies = use_resource(move || async move {
        let resp = server_client.read().get_rooms().await;
        let lobbies = match resp {
            Ok(x) => x.lobbies,
            Err(err) => vec![],
        };
        lobbies
    });
    // let mut search_lobbies: Signal<Vec<Lobby>> = use_signal(|| vec![]);

    // let test_lobbies: Vec<Lobby> = match &*all_lobbies.read_unchecked() {
    //     Some(Ok(vals)) => {
    //         let searchmatches = vals
    //             .lobbies
    //             .iter()
    //             .filter(|lobby| lobby.name.contains(searchterm.read().as_str()))
    //             .cloned()
    //             .collect::<Vec<Lobby>>();
    //         // search_lobbies.set(searchmatches);
    //         searchmatches
    //     }
    //     Some(Err(err)) => vec![],
    //     // Some(Err(err)) => search_lobbies.set(vec![]),
    //     None => vec![],
    //     // None => search_lobbies.set(vec![]),
    // };

    let create_lobby_function = move |lobby: String| {
        #[derive(Deserialize, Serialize)]
        pub struct CreateGameRequest {
            lobby_code: String,
        }

        spawn(async move {
            let resp = reqwest::Client::new()
                .post(format!(
                    "{}{}",
                    server_config.read().server_url.clone(),
                    "/rooms"
                ))
                .json(&CreateGameRequest {
                    lobby_code: lobby.clone(),
                })
                .send()
                .await;

            match resp {
                Ok(data) => {
                    info!("create_lobby success");
                    create_lobby_response_msg
                        .set(format!("Success! Created new game lobby").into());
                    all_lobbies.restart();
                }
                Err(err) => {
                    create_lobby_response_msg.set(format!("{err}").into());
                }
            }
        });
    };

    // let join_lobby = |lobby_code| {
    //     current_route.set("GameRoom".to_string());
    //     user_config.write().lobby_code = lobby.name.clone();
    // };

    // let test_results = search_results.read().clone();
    // let mut update_current_lobby = |evt, lobby_name| {
    //     current_route.set("GameRoom".to_string());
    //     user_config.write().lobby_code = lobby_name;
    // };
    // search results rsx

    let mut update_lobby_details = move |lobby_code: String| {
        current_route.set("GameRoom".to_string());
        user_config.write().lobby_code = lobby_code.clone();
    };

    rsx!(
        // div { class: "max-w-[300px]",
        div { class: "justify-center space-between p-2 w-full",
            h1 { class: "text-2xl font-bold text-center", "Game Lobbies" }
            div { class: "justify-center items-center w-full gap-2",
                div { class: "flex flex-row",
                    input {
                        class: "{styles::INPUT_FIELD} w-full sm:w-auto",
                        r#type: "text",
                        value: "{lobby_name.read()}",
                        oninput: move |event| lobby_name.set(event.value()),
                        "lobby"
                    }
                    button {
                        class: "bg-yellow-400 border border-solid border-black text-center rounded-md w-1/4 p-2",
                        onclick: move |_| {
                            info!("Clicked create lobby");
                            create_lobby_function(lobby_name.read().clone());
                        },
                        "create"
                    }
                    button {
                        class: "bg-gray-300 flex flex-row text-center border border-solid border-black rounded-md justify-center items-center cursor-pointer w-1/6 p-2 hover:bg-gray-400",
                        onclick: move |evt| {
                            all_lobbies.restart();
                        },
                        svg {
                            class: "w-6 h-6 transition-transform duration-300 hover:rotate-180",
                            fill: "black",
                            stroke: "currentColor",
                            "stroke-width": "1",
                            "view-box": "0 0 24 24",
                            path {
                                "stroke-linecap": "round",
                                "stroke-linejoin": "round",
                                d: "M4 4v5h.582c.523-1.838 1.856-3.309 3.628-4.062A7.978 7.978 0 0112 4c4.418 0 8 3.582 8 8s-3.582 8-8 8a7.978 7.978 0 01-7.658-5.125c-.149-.348-.54-.497-.878-.365s-.507.537-.355.885A9.956 9.956 0 0012 22c5.523 0 10-4.477 10-10S17.523 2 12 2c-2.045 0-3.94.613-5.514 1.653A6.978 6.978 0 004.582 4H4z",
                            }
                        }
                    }
                }
            }
            {
                if create_lobby_response_msg() == String::from("") {
                    rsx! {
                        div {}
                    }
                } else {
                    rsx! {
                        div { class: "text-center", "{create_lobby_response_msg.read()}" }
                    }
                }
            }
        }
        div { class: "flex flex-col",
            div { class: "relative",
                svg {
                    class: "absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 h-5 w-5",
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
                    class: "w-full pl-10 pr-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500",
                    r#type: "text",
                    placeholder: "Search lobbies...",
                    value: "",
                    oninput: move |evt| {
                        info!("Got search query: {evt:?}");
                        searchterm.set(evt.value().clone());
                    },
                }
            }
        }
        // div { class: "bg-white border border-gray-300 w-full",
        div { class: "grid grid-cols-3 items-center",
            div { class: "bg-gray-200",
                // class: "px-4 py-3 text-left text-xs text-gray-500 uppercase tracking-wider",
                "Lobby Name"
            }
            div { class: "bg-gray-200",
                // class: "px-1 py-3 text-left text-xs text-gray-500 uppercase tracking-wider",
                "Players"
            }
            div { class: "bg-gray-200",
                // class: "text-left text-xs text-gray-500 uppercase tracking-wider",
                "Action"
            }
        }
        div { class: "border border-black rounded-md h-[400px]",
            div { class: "grid grid-cols-3 overflow-scroll items-baseline",
                match &*all_lobbies.read_unchecked() {
                    Some(vals) => {
                        let searchmatches = vals
                            .iter()
                            .filter(|lobby| lobby.name.contains(searchterm.read().as_str()))
                            .cloned()
                            .collect::<Vec<Lobby>>();
                        rsx! {
                            {
                                searchmatches
                                    .into_iter()
                                    .map(|lobby| {
                                        rsx! {
                                            div { class: "break-words text-center", "{lobby.name}" }
                                            div { class: "", "{lobby.players.len()}/{lobby.max_players}" }
                                            div { class: "",
                                                button {
                                                    onclick: move |evt| update_lobby_details(lobby.name.clone()),
                                                    class: "py-2 rounded-md text-sm font-medium w-full bg-yellow-300",
                                                    "Join"
                                                }
                                            }
                                        }
                                    })
                            }
                        }
                    }
                    None => rsx! {
                        div { "No matches" }
                    },
                }
            }
        }
    )
}

#[component]
fn GameRoom(room_code: String) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();
    let mut server_config: Signal<ServerConfig> = use_context::<Signal<ServerConfig>>();
    let mut gamestate = use_signal(|| GameState::new(room_code.clone()));
    let mut setupgameoptions = use_signal(|| SetupGameOptions {
        rounds: 4,
        deterministic: if app_props.read().is_prod() {
            false
        } else {
            false
        },
        start_round: None,
        max_players: 4,
        game_mode: "Standard".to_string(),
        visibility: GameVisibility::Public,
        password: None,
        computer_players: 0,
    });

    let mut ws_url = use_signal(|| {
        String::from(format!(
            "{}/rooms/{}/ws",
            server_config.read().server_ws_url.clone(),
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
    let mut websocket_connected = use_signal(|| false);
    // let ws_connection = use_signal(|| WebsocketConnection::new(server_config.read().clone()));

    let mut server_websocket_listener: Signal<Option<SplitStream<WebSocket>>> = use_signal(|| None);
    let mut server_websocket_sender: Signal<Option<SplitSink<WebSocket, Message>>> =
        use_signal(|| None);
    let mut ws_action = use_signal(|| WsState::Resume);
    let mut ws_connection = use_signal(|| WebsocketConnection::new(server_config.read().clone()));

    let mut create_lobby_response_msg = use_signal(|| String::from(""));
    let room_code_clone = room_code.clone();

    use_effect(move || {
        spawn(async move {
            info!(
                "Attempting to connect to websocket server: {}",
                ws_connection.read().config.server_ws_url
            );
            let ws = ws_connection.write().connect_websocket().await.unwrap();
            let (mut ws_tx, mut ws_rx) = ws.split();
            server_websocket_sender.set(Some(ws_tx));
            server_websocket_listener.set(Some(ws_rx));
            info!("Websocket connection task finished");
        });
    });

    let get_details_room_code = room_code_clone.clone();
    let get_game_details = move |get_details_room_code: String| {
        info!("Getting details of game: {get_details_room_code}");
        spawn(async move {
            let resp = reqwest::Client::new()
                .get(format!(
                    "{}{}",
                    server_config.read().server_url.clone(),
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

            let mut listen = server_websocket_listener.write();
            let listener = listen.as_mut().expect("No websocket listener");
            let mut error_count = 0;
            while error_count < 10 {
                while let Some(Ok(Message::Text(message))) = listener.next().await {
                    info!("[SERVER-LISTENER] Got messages:");

                    match serde_json::from_str::<GameActionResponse>(&message) {
                        Ok(gar) => {
                            // is_gamestate = true;
                            match gar {
                                common::GameActionResponse::Connect(con) => {
                                    info!("Got connect message: {con:?}");
                                    user_config.write().client_secret =
                                        con.secret.unwrap_or(String::new());
                                }
                                common::GameActionResponse::GameState(gs) => {
                                    info!("Got game state");
                                    gamestate.set(gs);
                                }
                                common::GameActionResponse::Message(text) => {
                                    info!("Got message");
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
            // info!("Received internal message: {:?}", internal_msg);

            if server_websocket_sender.read().is_none() {
                info!("No websocket sender");
                return;
            }

            let mut send = server_websocket_sender.write();
            let sender = send.as_mut().unwrap();

            // info!("Received internal message: {:?}", internal_msg);
            match internal_msg {
                InnerMessage::UpdateWsState { new } => ws_action.set(new),
                InnerMessage::GameMessage { msg } => {
                    if ws_action() == WsState::Pause {
                        continue 'pauseloop;
                    }

                    let _ = sender.send(Message::Text(json!(msg).to_string())).await;
                }
                InnerMessage::Connect(con) => {
                    let _ = sender.send(Message::Text(json!(con).to_string())).await;
                }
            }
            info!("Finished processing action, waiting for next...");
        }
        info!("Finished listening to player actions");
    });

    let ws_send_signal = use_signal(|| ws_send);
    rsx!(
        div { class: "grid flex-col md:flex-row text-center bg-bg-color min-h-screen w-full flex-wrap md:flex-nowrap justify-center gap-2 p-2 md:p-4 items-center align-middle",
            {
                if error().is_null() {
                    rsx! {}
                } else {
                    error
                        .read()
                        .as_str()
                        .map(|err| rsx! {
                            div { class: "w-full", "{err}" }
                        })
                        .expect("Failed to parse error")
                }
            }
            {
                if !app_props.read().is_debug_mode() {
                    rsx! {}
                } else {
                    rsx! {}
                }
            }
            {
                if gamestate().gameplay_state == GameplayState::Pregame {
                    rsx! {
                        div { class: "flex flex-col gap-2 w-full self-center max-w-[600px]",
                            div { class: "flex flex-row w-full border border-black rounded-md p-2 md:p-4 self-center",
                                div { class: "flex flex-col md:flex-row justify-center align-top text-center items-center w-full border border-black rounded-md p-2",
                                    h1 { class: "text-xl md:text-2xl", "{get_lobby_response.read().lobby.name}" }
                                    div { class: "container",
                                        "Players ({get_lobby_response.read().lobby.players.len()})"
                                        {
                                            get_lobby_response
                                                .read()
                                                .lobby
                                                .players
                                                .iter()
                                                .enumerate()
                                                .map(|(i, player)| rsx! {
                                                    div { "{i}: {player}" }
                                                })
                                        }
                                    }
                                }
                                button {
                                    class: "{styles::STANDARD_BUTTON} text-white",
                                    onclick: move |evt| get_game_details(room_code_clone.clone()),
                                    "Refresh"
                                }
                            }
                            div { class: "flex flex-col w-full md:max-w-[600px] self-center border border-black rounded-md p-2",
                                h2 { class: "text-xl md:text-2xl", "Game options" }
                                // settings
                                div { class: "flex flex-col align-middle justify-center text-center w-full container mx-auto p-4 max-w-lg gap-2",
                                    div { class: "flex flex-row items-center justify-center",
                                        label { class: "text-sm md:text-base", "Rounds" }
                                        input {
                                            class: "{styles::INPUT_FIELD} w-16 md:w-20",
                                            r#type: "text",
                                            "data-input-counter": "false",
                                            placeholder: "9",
                                            required: "false",
                                            value: "{setupgameoptions.read().rounds}",
                                        }
                                        button {
                                            "data-input-counter-decrement": "quantity-input",
                                            r#type: "button",
                                            class: "{styles::INC_DEC_BUTTONS} rounded-s-lg",
                                            id: "decrement-button",
                                            disabled: if setupgameoptions.read().rounds <= 1 { true } else { false },
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
                                                    "stroke-linejoin": "round",
                                                }
                                            }
                                        }
                                        button {
                                            "data-input-counter-increment": "quantity-input",
                                            r#type: "button",
                                            class: "{styles::INC_DEC_BUTTONS} rounded-e-lg",
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
                                                    "d": "M9 1v16M1 9h16",
                                                }
                                            }
                                        }
                                    }
                                    div { class: "flex flex-row items-center justify-center",
                                        label { class: "text-sm md:text-base", "computer players" }
                                        input {
                                            class: "{styles::INPUT_FIELD} w-16 md:w-20",
                                            r#type: "number",
                                            required: "false",
                                            value: "{setupgameoptions.read().computer_players}",
                                                        // onchange: move |evt| {
                                        //     setupgameoptions.write().computer_players = evt.value().parse::<usize>().unwrap_or(0);
                                        // }
                                        }
                                        button {
                                            "data-input-counter-decrement": "quantity-input",
                                            r#type: "button",
                                            class: "{styles::INC_DEC_BUTTONS} rounded-s-lg",
                                            id: "decrement-button",
                                            disabled: if setupgameoptions.read().computer_players == 0 { true } else { false },
                                            onclick: move |evt| {
                                                if setupgameoptions.read().computer_players > 0 {
                                                    setupgameoptions.write().computer_players -= 1;
                                                }
                                            },
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
                                                    "stroke-linejoin": "round",
                                                }
                                            }
                                        }
                                        button {
                                            "data-input-counter-increment": "quantity-input",
                                            r#type: "button",
                                            class: "{styles::INC_DEC_BUTTONS} rounded-e-lg",
                                            id: "increment-button",
                                            onclick: move |_| setupgameoptions.write().computer_players += 1,
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
                                                    "d": "M9 1v16M1 9h16",
                                                }
                                            }
                                        }
                                    }
                                    if app_props.read().is_debug_mode() {
                                        div { class: "flex flex-row items-center justify-center space-x-4",
                                            label { class: "text-sm md:text-base", "start round" }
                                            input {
                                                class: "{styles::INPUT_FIELD} w-16 md:w-20",
                                                r#type: "text",
                                                // "data-input-counter": "false",
                                                // placeholder: "",
                                                required: "false",
                                                value: "{setupgameoptions.read().start_round.unwrap_or(1)}",
                                                onchange: move |evt| {
                                                    setupgameoptions.write().start_round = evt.value().parse::<usize>().ok();
                                                },
                                            }
                                        }
                                    }
                                    div { class: "flex flex-row items-center justify-center space-x-4",
                                        span { class: "text-sm md:text-base", "Public" }
                                        label { class: "relative flex items-center cursor-pointer",
                                            div { class: "relative",
                                                input {
                                                    checked: "{setupgameoptions.read().visibility == GameVisibility::Private}",
                                                    class: "peer hidden",
                                                    r#type: "checkbox",
                                                    onchange: move |evt| {
                                                        setupgameoptions.write().visibility = if setupgameoptions.read().visibility
                                                            == GameVisibility::Private
                                                        {
                                                            GameVisibility::Public
                                                        } else {
                                                            GameVisibility::Private
                                                        };
                                                    },
                                                }
                                                div {
                                                    class: "block w-12 md:w-14 h-6 md:h-8 rounded-full transition-colors duration-300 ease-in-out",
                                                    class: if setupgameoptions.read().visibility == GameVisibility::Private { "bg-red-300" } else { "bg-green-200" },
                                                }
                                                div { class: "absolute left-1 top-1 bg-white w-4 md:w-6 h-4 md:h-6 rounded-full transition-transform duration-300 ease-in-out peer-checked:translate-x-6" }
                                            }
                                        }
                                        span { class: "text-sm md:text-base", "Private" }
                                    }
                                    {
                                        if setupgameoptions.read().visibility == GameVisibility::Private {
                                            rsx! {
                                                div { class: "flex flex-row items-center justify-center space-x-2",
                                                    span { class: "text-sm md:text-base", "Password" }
                                                    input {
                                                        class: "{styles::INPUT_FIELD} w-32 md:w-40",
                                                        r#type: "text",
                                                        placeholder: "",
                                                        required: "false",
                                                        value: if setupgameoptions.read().password.is_some() { "{setupgameoptions.read().password:?}" } else { "" },
                                                        onchange: move |evt| {
                                                            setupgameoptions.write().password = evt.value().parse::<String>().ok();
                                                        },
                                                    }
                                                }
                                            }
                                        } else {
                                            rsx! {}
                                        }
                                    }
                                }
                                button {
                                    class: "{styles::STANDARD_BUTTON} text-white",
                                    onclick: move |evt| {
                                        async move {
                                            info!("Clicked join game");
                                            listen_for_server_messages.send(("ready".to_string()));
                                            ws_send
                                                .send(InnerMessage::GameMessage {
                                                    msg: GameMessage {
                                                        username: user_config().username.clone(),
                                                        timestamp: Utc::now(),
                                                        action: GameAction::JoinGame(PlayerDetails {
                                                            lobby: user_config.read().lobby_code.clone(),
                                                            username: user_config.read().username.clone(),
                                                            ip: None,
                                                            client_secret: Some(user_config.read().client_secret.clone()),
                                                        }),
                                                        lobby: user_config.read().lobby_code.clone(),
                                                    },
                                                });
                                        }
                                    },
                                    "Join"
                                }
                                button {
                                    class: "bg-yellow-300 border border-solid border-black text-center rounded-md p-2  hover:bg-yellow-400 transition-colors",
                                    onclick: move |evt| {
                                        info!("Starting game");
                                        listen_for_server_messages.send(("ready".to_string()));
                                        if (user_config.read().client_secret.is_empty()) {
                                            ws_send
                                                .send(InnerMessage::GameMessage {
                                                    msg: GameMessage {
                                                        username: user_config().username.clone(),
                                                        timestamp: Utc::now(),
                                                        action: GameAction::JoinGame(PlayerDetails {
                                                            lobby: user_config.read().lobby_code.clone(),
                                                            username: user_config.read().username.clone(),
                                                            ip: None,
                                                            client_secret: Some(user_config.read().client_secret.clone()),
                                                        }),
                                                        lobby: user_config.read().lobby_code.clone(),
                                                    },
                                                });
                                        }
                                        ws_send
                                            .send(InnerMessage::GameMessage {
                                                msg: GameMessage {
                                                    username: user_config.read().username.clone(),
                                                    action: GameAction::StartGame(setupgameoptions()),
                                                    lobby: user_config.read().lobby_code.clone(),
                                                    timestamp: Utc::now(),
                                                },
                                            });
                                    },
                                    "Start game"
                                }
                                div { class: "flex flex-col w-full",
                                    {
                                        if gamestate().system_status.len() > 0 {
                                            rsx! {
                                                ul { class: "w-full mx-auto my-4 p-4 border border-blue-400 rounded-lg bg-yellow-100 text-blue-800",
                                                    {gamestate().system_status.iter().map(|issue| rsx! {
                                                        li { class: "text-xs md:text-sm", "{issue}" }
                                                    })}
                                                }
                                            }
                                        } else {
                                            rsx! {
                                                div { class: "w-full mx-auto my-4 p-4 border border-blue-400 rounded-lg bg-yellow-100 text-blue-800 text-xs md:text-sm",
                                                    "Please join the game"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! {
                        GameStateComponent { gamestate, ws_send: ws_send_signal }
                    }
                }
            }
        }
    )
}

// pub const SUIT_CLUB: ImageAsset = asset!("./assets/suits/club.png").image();
// pub const SUIT_HEART: manganis::ImageAsset = asset!("./assets/suits/heart.png").image();
// pub const SUIT_DIAMOND: manganis::ImageAsset = asset!("./assets/suits/diamond.png").image();
// pub const SUIT_SPADE: manganis::ImageAsset = asset!("./assets/suits/spade.png").image();
// pub const SUIT_NOTRUMP: ImageAsset = asset!("./assets/suits/notrump.png").image();
pub const SUIT_CLUB: Asset = asset!("./public/suits/club.png");
pub const SUIT_HEART: Asset = asset!("./public/suits/heart.png");
pub const SUIT_DIAMOND: Asset = asset!("./public/suits/diamond.png");
pub const SUIT_SPADE: Asset = asset!("./public/suits/spade.png");
pub const SUIT_NOTRUMP: Asset = asset!("./public/suits/notrump.png");

#[component]
fn CardComponent(
    card: Card,
    onclick: EventHandler<Card>,
    is_winning: bool,
    show_player: bool,
    show_order: bool,
    order: usize,
) -> Element {
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
            class: "grid place-items-center relative",
            onclick: move |evt| {
                onclick(card.clone());
            },
            div { class: "col-start-1 row-start-1 h-full w-full",
                div { class: "grid absolute w-full h-full z-20 place-items-center",
                    div { class: "col-start-1 row-start-1", {suit_svg} }
                    span { class: "col-start-1 row-start-1 text-white content-center text-center text-2xl self-center justify-center drop-shadow-[0_2.2px_2.2px_rgba(0,0,0,0.8)]",
                        "{textvalue}"
                    }
                    if show_player && card.played_by.is_some() {
                        span { class: "absolute col-start-1 row-start-1 bottom-1 text-black text-[9px] bg-green-200 border px-0.5 border-black rounded-md",
                            "{card.played_by.as_ref().unwrap()}"
                        }
                    }
                    if show_order {
                        span { class: "absolute top-1 left-1 text-white text-xs font-bold px-1.5 py-0.5 rounded-full bg-black",
                            "{order}"
                        }
                    }
                    if is_winning {
                        span { class: "absolute top-1 right-1 text-black text-xs font-bold px-1 py-0.5 rounded-full border border-black bg-yellow-200 animate-subtle-pulse-winning",
                            "W"
                        }
                    }
                }
                svg {
                    class: "w-[64px]",
                    "shape-rendering": "crispEdges",
                    "viewBox": "0 -0.5 48 64",
                    "xmlns": "http://www.w3.org/2000/svg",
                    fill: "none",
                    meta { data: "false" }
                    "Made with Pixels to Svg https://codepen.io/shshaw/pen/XbxvNj"
                    path {
                        "d": "M4 3h1M28 3h7M39 3h3M43 3h1M43 4h1M43 11h1M3 13h1M43 21h1M3 24h1M3 25h1M3 27h1M3 28h1M3 29h1M43 32h1M3 36h1M3 37h1M3 40h1M3 41h1M3 42h1M43 42h1M3 43h1M3 44h1M3 45h1M3 46h1M3 47h1M3 48h1M43 50h1M43 51h1M3 52h1M3 53h1M3 54h1M3 55h1M3 59h1M6 59h2M14 59h2M29 59h4M34 59h9",
                        "stroke": "#000000",
                    }
                    path {
                        "d": "M5 3h23M35 3h4M42 3h1M3 4h1M3 5h1M43 5h1M3 6h1M43 6h1M3 7h1M43 7h1M3 8h1M43 8h1M3 9h1M43 9h1M3 10h1M43 10h1M3 11h1M3 12h1M43 12h1M43 13h1M3 14h1M43 14h1M3 15h1M43 15h1M3 16h1M43 16h1M3 17h1M43 17h1M3 18h1M43 18h1M3 19h1M43 19h1M3 20h1M43 20h1M3 21h1M3 22h1M43 22h1M3 23h1M43 23h1M43 24h1M43 25h1M3 26h1M43 26h1M43 27h1M43 28h1M43 29h1M3 30h1M43 30h1M3 31h1M43 31h1M3 32h1M3 33h1M43 33h1M3 34h1M43 34h1M3 35h1M43 35h1M43 36h1M43 37h1M3 38h1M43 38h1M3 39h1M43 39h1M43 40h1M43 41h1M43 43h1M43 44h1M43 45h1M43 46h1M43 47h1M43 48h1M3 49h1M43 49h1M3 50h1M3 51h1M43 52h1M43 53h1M43 54h1M43 55h1M3 56h1M43 56h1M3 57h1M43 57h1M3 58h1M43 58h1M4 59h2M8 59h6M16 59h13M33 59h1",
                        "stroke": "#010101",
                    }
                    path {
                        "stroke": "#807f7f",
                        "d": "M44 4h1M44 5h1M44 6h1M44 7h1M44 8h1M44 9h1M44 10h1M44 11h1M44 12h1M44 13h1M44 14h1M44 15h1M44 16h1M44 17h1M44 18h1M44 19h1M44 20h1M44 21h1M44 22h1M44 23h1M44 24h1M44 25h1M44 26h1M44 27h1M44 28h1M44 29h1M44 30h1M44 31h1M44 32h1M44 33h1M44 34h1M44 35h1M44 36h1M44 37h1M44 38h1M44 39h1M44 40h1M44 41h1M44 42h1M44 43h1M44 44h1M44 45h1M44 46h1M44 47h1M44 48h1M44 49h1M44 50h1M44 51h1M44 52h1M44 53h1M44 54h1M44 55h1M44 56h1M44 57h1M44 58h1M44 59h1M4 60h40",
                    }
                    path {
                        "d": "M45 5h1M45 6h1M45 7h1M45 8h1M45 9h1M45 10h1M45 11h1M45 12h1M45 13h1M45 14h1M45 15h1M45 16h1M45 17h1M45 18h1M45 19h1M45 20h1M45 21h1M45 22h1M45 23h1M45 24h1M45 25h1M45 26h1M45 27h1M45 28h1M45 29h1M45 30h1M45 31h1M45 32h1M45 33h1M45 34h1M45 35h1M45 36h1M45 37h1M45 38h1M45 39h1M45 40h1M45 41h1M45 42h1M45 43h1M45 44h1M45 45h1M45 46h1M45 47h1M45 48h1M45 49h1M45 50h1M45 51h1M45 52h1M45 53h1M45 54h1M45 55h1M45 56h1M45 57h1M45 58h1M45 59h1M44 60h2M5 61h12M18 61h2M21 61h20M42 61h3",
                        "stroke": "#d7d2d2",
                    }
                    path { "stroke": "#7f7e7e", "d": "M43 59h1" }
                    path { "stroke": "#d8d2d2", "d": "M17 61h1M20 61h1M41 61h1" }
                }
            }
        }
    )
}

#[component]
fn GameStatusInfoComponent(gamestate: Signal<GameState>, visible: bool) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();

    rsx!(match gamestate().gameplay_state {
        GameplayState::PostHand(ps) => {
            let round_winner = gamestate
                .read()
                .curr_winning_card
                .clone()
                .unwrap()
                .played_by
                .unwrap_or("Nobody".to_string());
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold",
                        "Hand over, winner is "
                        span { class: "text-yellow-300", "{round_winner}" }
                        "!"
                    }
                }
            }
        }
        GameplayState::Bid => {
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold",
                        "{gamestate().curr_player_turn.clone().unwrap()}'s turn to bid"
                    }
                }
            }
        }
        GameplayState::Play(ps) => {
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold",
                        "{gamestate().curr_player_turn.clone().unwrap()}'s turn to play a card"
                    }
                }
            }
        }
        GameplayState::Pregame => {
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold", "Waiting to start the game" }
                }
            }
        }
        GameplayState::PostRound => {
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold", "Round over" }
                    ul { class: "text-left text-sm w-full justify-center",
                        {
                            gamestate()
                                .players
                                .iter()
                                .map(|(player, client)| {
                                    let wins = gamestate().wins.get(player).unwrap_or(&0).clone();
                                    let bid = gamestate()
                                        .bids
                                        .get(player)
                                        .unwrap_or(&Some(0))
                                        .clone()
                                        .unwrap();
                                    let win_message = format!(
                                        "got {wins}/{bid}{}",
                                        if wins == bid { "" } else { " got BLACKBALL" },
                                    );
                                    rsx! {
                                        li { class: "flex flex-col items-center justify-center",
                                            div { class: "flex justify-between gap-2",
                                                span { "{player}" }
                                                span { "{win_message}" }
                                            }
                                        }
                                    }
                                })
                        }
                    }
                }
            }
        }
        GameplayState::End => {
            let gamewinner = gamestate()
                .score
                .iter()
                .max_by_key(|(_, v)| *v)
                .unwrap()
                .0
                .clone();
            rsx! {
                div { class: "{styles::ROUND_DETAILS_TAILWIND}",
                    p { class: "text-base sm:text-lg font-semibold",
                        "Game over!"
                        span { class: "text-yellow-300", "{gamewinner}" }
                        " won the game!"
                    }
                    ul { class: "text-sm ",
                        {
                            gamestate()
                                .players
                                .iter()
                                .map(|(player, client)| {
                                    let score = gamestate().score.get(player).unwrap_or(&0).clone();
                                    let text = format!("{player}: {score}");
                                    rsx! {
                                        li { "{text}" }
                                    }
                                })
                        }
                    }
                }
            }
        }
        _ => rsx! {
            div {}
        },
    })
}
#[component]
fn TransitionComponent(gamestate: Signal<GameState>, visible: bool) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();

    if !visible {
        return rsx!();
    }

    rsx!(match gamestate().gameplay_state {
        GameplayState::Bid => {
            let curr_player = gamestate().curr_player_turn.clone().unwrap();
            rsx! {
                div { class: "absolute items-center justify-center z-20 w-[200%] h-[20%] animate-gamestate-transition text-4xl font-bold bg-black text-white opacity-0",
                    h2 { class: "text-4xl font-bold text-white", "Bid phase" }
                    h3 { class: "text-4xl font-bold text-white", "{curr_player}'s turn first" }
                }
            }
        }
        GameplayState::Play(ps) => {
            let round = gamestate.read().curr_round;
            let curr_player = gamestate().curr_player_turn.clone().unwrap();
            rsx! {
                div { class: "absolute items-center justify-center z-20 w-[200%] h-[20%] animate-gamestate-transition text-4xl font-bold bg-black text-white opacity-0",
                    h2 { class: "text-4xl font-bold text-white", "Hand {ps.hand_num}/{round}" }
                    h3 { class: "text-4xl font-bold text-white", "{curr_player} plays first" }
                }
            }
        }
        _ => rsx! {},
    })
}

#[component]
fn GameStateComponent(
    gamestate: Signal<GameState>,
    ws_send: Signal<Coroutine<InnerMessage>>,
) -> Element {
    let mut app_props = use_context::<Signal<AppProps>>();
    let mut user_config: Signal<UserConfig> = use_context::<Signal<UserConfig>>();
    let transition_visible = false;

    info!("Rendering gamestate...");

    let trump_svg = get_trump_svg(&gamestate.read().trump);
    let curr_player = gamestate
        .read()
        .curr_player_turn
        .clone()
        .unwrap_or("".to_string());
    let cards_in_hand = if gamestate
        .read()
        .players
        .contains_key(&user_config.read().username)
    {
        Some(
            gamestate
                .read()
                .players
                .get(&user_config.read().username)
                .expect("Failed to get player in gamestate")
                .encrypted_hand
                .clone(),
        )
    } else {
        None
    };

    // let testvec: Vec<Card> = vec![];
    // testvec.sort_by(|a, b| a.id.cmp(&b.id));

    rsx!(
        // div { class: "grid grid-cols-1 grid-rows-[150px_44px_160px_160px_90px] gap-2 w-screen h-screen text-center bg-bg-color flex-nowrap justify-center p-2 overflow-hidden items-start align-middle self-center",
        div { class: "grid grid-cols-1 grid-rows-[150px_44px_auto_auto_90px] gap-2 w-screen h-screen max-w-[600px] min-w-[360px] text-center bg-bg-color flex-nowrap justify-center p-2 overflow-hidden items-start align-middle self-center",
            // TransitionComponent { gamestate, visible: transition_visible }
            div { class: "col-start-1 row-start-1 bg-bg-color rounded-lg p-2 border border-black gap-2 w-full justify-self-center",
                div { class: "flex flex-col justify-between gap-2",
                    div { class: "bg-bg-color rounded-lg flex flex-col w-full items-center justify-between",
                        div { class: "flex flex-row w-full gap-2 items-center",
                            h2 { class: "text-lg font-bold rounded-md bg-black text-white flex-1",
                                "BLACKBALL"
                            }
                            div { class: "flex items-center justify-between",
                                match gamestate().gameplay_state {
                                    GameplayState::PostHand(ps) => rsx! {
                                        span { class: "text-sm  font-bold", "End of hand {ps.hand_num}" }
                                    },
                                    GameplayState::Play(ps) => rsx! {
                                        span { class: "text-sm  font-bold", "Playing hand {ps.hand_num}/{ps.hands}" }
                                    },
                                    _ => rsx! {
                                        span { class: "text-sm  font-bold", "{gamestate().gameplay_state:?}" }
                                    },
                                }
                            }
                            div { class: "flex flex-col items-center justify-between",
                                // span { class: "font-semibold text-sm ", "Trump:" }
                                div { class: "flex items-center", {trump_svg} }
                            }
                            div { class: "flex flex-col items-center justify-between",
                                span { class: "font-semibold text-sm ", "Round" }
                                span { class: "text-sm ",
                                    "{gamestate().curr_round}/{gamestate().max_rounds}"
                                }
                            }
                        }
                    }
                    div { class: "bg-bg-color rounded-lg  overflow-auto w-full",
                        div { class: "gap-2 flex overflow-auto",
                            {
                                gamestate()
                                    .player_order
                                    .iter()
                                    .enumerate()
                                    .map(|(i, playername)| {
                                        let wins = gamestate.read().wins.get(playername).unwrap_or(&0).clone();
                                        let bid = gamestate.read().bids.get(playername).unwrap_or(&None).clone();
                                        let bid_val = if let Some(x) = bid {
                                            x.to_string()
                                        } else {
                                            "N/A".to_string()
                                        };
                                        rsx! {
                                            div {
                                                class: format!(
                                                    "flex flex-col items-center w-full border border-black rounded-lg text-left {} {}",
                                                    if gamestate().curr_player_turn.clone().unwrap_or("".to_string()) == *playername
                                                    {
                                                        "border-4 rounded-lg animate-subtle-pulse"
                                                    } else {
                                                        ""
                                                    },
                                                    if bid.is_some() && wins == bid.unwrap() {
                                                        "bg-green-200"
                                                    } else if bid.is_some() && wins > bid.unwrap() {
                                                        "bg-red-200"
                                                    } else {
                                                        "bg-green-100"
                                                    },
                                                ),
                                                div { class: "flex flex-col items-baseline w-full text-center",
                                                    div {
                                                        class: format!(
                                                            "w-full rounded-lg {}",
                                                            if *playername == user_config.read().username { "bg-name-tag" } else { "" },
                                                        ),
                                                        span { class: "font-semibold text-sm", "{playername}" }
                                                        if *playername == gamestate.read().get_dealer() {
                                                            // if *playername == gamestate.read().curr_dealer {
                                                            span { class: "top-0 right-0 bg-black text-white text-xs font-bold px-1 py-0.5 rounded-full",
                                                                "D"
                                                            }
                                                        }
                                                    }
                                                }
                                                div { class: "text-right",
                                                    div { class: "text-xs flex justify-between",
                                                        span { "Score:" }
                                                        span { "{gamestate().score.get(playername).unwrap_or(&0)}" }
                                                    }
                                                    div { class: "text-xs flex justify-between",
                                                        span { "Wins:" }
                                                        span { "{wins}" }
                                                    }
                                                    div { class: "text-xs flex justify-between",
                                                        span { "Bid:" }
                                                        span { "{bid_val}" }
                                                    }
                                                }
                                            }
                                        }
                                    })
                            }
                        }
                    }
                }
            }
            div { class: "col-start-1 row-start-2 h-[40px] w-full",
                GameStatusInfoComponent { gamestate, visible: true }
            }
            div { class: "col-start-1 row-start-3 relative w-full h-full bg-card-area rounded-lg text-gray-100 border border-black",
                div { class: "absolute top-1 left-1 px-1 py-1 text-[9px] bg-indigo-600 rounded-md z-10",
                    "Played cards"
                }
                div { class: "flex flex-row flex-wrap mt-2 justify-center gap-1",
                    {
                        gamestate()
                            .curr_played_cards
                            .iter()
                            .enumerate()
                            .map(|(i, card)| rsx! {
                                CardComponent {
                                    onclick: move |_| { info!("Clicked a card: {:?}", "fake card") },
                                    card: card.clone(),
                                    is_winning: gamestate.read().curr_winning_card.is_some()
                                        && gamestate.read().curr_winning_card.clone().unwrap() == card.clone(),
                                    show_player: true,
                                    show_order: true,
                                    order: i,
                                }
                            })
                    }
                }
            }
            div { class: "col-start-1 row-start-4 justify-between gap-2 w-full h-full",
                div {
                    class: format!(
                        "relative w-full bg-card-area h-full rounded-lg border border-black {}",
                        if gamestate().curr_player_turn.clone().unwrap_or("".to_string())
                            == user_config.read().username
                        {
                            "border-8 border-red-400 rounded-lg animate-subtle-pulse"
                        } else {
                            ""
                        },
                    ),
                    div { class: "absolute top-1 left-1 px-1 py-1 text-[9px] bg-green-200 rounded-md z-10",
                        "Your hand"
                    }
                    div { class: "flex flex-row flex-wrap mt-2 justify-center gap-1",
                        {
                            if cards_in_hand.is_none() {
                                rsx! {}
                            } else {
                                info!(
                                    "[FE] calling to decrypt player hand: ${:?}, secret: ${:?}",
                                    cards_in_hand, user_config.read().client_secret.clone()
                                );
                                let mut sortedcards = GameState::decrypt_player_hand(
                                        cards_in_hand.unwrap(),
                                        &user_config.read().client_secret.clone(),
                                    )
                                    .clone();
                                sortedcards.sort_by(|a, b| a.id.cmp(&b.id));
                                rsx! {
                                    {
                                        sortedcards
                                            .iter()
                                            .enumerate()
                                            .map(|(i, card)| {
                                                return rsx! {
                                                    CardComponent {
                                                        onclick: move |clicked_card: Card| {
                                                            ws_send()
                                                                .send(InnerMessage::GameMessage {
                                                                    msg: GameMessage {
                                                                        username: user_config.read().username.clone(),
                                                                        action: GameAction::PlayCard(clicked_card),
                                                                        timestamp: Utc::now(),
                                                                        lobby: user_config.read().lobby_code.clone(),
                                                                    },
                                                                });
                                                        },
                                                        card: card.clone(),
                                                        is_winning: gamestate.read().curr_winning_card.is_some()
                                                            && gamestate.read().curr_winning_card.clone().unwrap() == card.clone(),
                                                        show_player: false,
                                                        show_order: false,
                                                        order: i,
                                                    }
                                                };
                                            })
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "col-start-1 row-start-5",
                // span { "this is a test" }
                if gamestate().gameplay_state == GameplayState::Bid
                    && gamestate().curr_player_turn.is_some()
                    && gamestate().curr_player_turn.clone().unwrap() == user_config.read().username
                {
                    div { class: "flex flex-col items-center h-[100px]",
                        label { class: "text-base", "How many hands do you want to win?" }
                        ul { class: "flex flex-row gap-2 items-center p-2 justify-center",
                            {
                                (0..=gamestate().cards_to_deal)
                                    .map(|i| {
                                        if user_config.read().username == gamestate().get_dealer()
                                            && (i + gamestate().bids.values().map(|x| x.unwrap()).sum::<i32>())
                                                == gamestate().cards_to_deal
                                        {
                                            rsx! {
                                                button {
                                                    class: "styles::BID_BUTTON bg-bg-color",
                                                    // disabled: true,
                                                    onclick: move |_| {
                                                        info!("Clicked on bid {i}");
                                                        ws_send()
                                                            .send(InnerMessage::GameMessage {
                                                                msg: GameMessage {
                                                                    username: user_config.read().username.clone(),
                                                                    action: GameAction::Bid(i),
                                                                    lobby: user_config.read().lobby_code.clone(),
                                                                    timestamp: Utc::now(),
                                                                },
                                                            });
                                                    },
                                                    "{i}"
                                                }
                                            }
                                        } else {
                                            rsx! {
                                                button {
                                                    class: "{styles::BID_BUTTON}",
                                                    onclick: move |_| {
                                                        info!("Clicked on bid {i}");
                                                        ws_send()
                                                            .send(InnerMessage::GameMessage {
                                                                msg: GameMessage {
                                                                    username: user_config.read().username.clone(),
                                                                    action: GameAction::Bid(i),
                                                                    lobby: user_config.read().lobby_code.clone(),
                                                                    timestamp: Utc::now(),
                                                                },
                                                            });
                                                    },
                                                    "{i}"
                                                }
                                            }
                                        }
                                    })
                            }
                        }
                    }
                }
                {
                    if let GameplayState::PostHand(ps) = gamestate().gameplay_state {
                        rsx! {
                            button {
                                class: "{styles::STANDARD_BUTTON} text-white",
                                onclick: move |_| {
                                    ws_send()
                                        .send(InnerMessage::GameMessage {
                                            msg: GameMessage {
                                                username: user_config.read().username.clone(),
                                                action: GameAction::Ack,
                                                lobby: user_config.read().lobby_code.clone(),
                                                timestamp: Utc::now(),
                                            },
                                        });
                                },
                                "Acknowledge"
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
                if let GameplayState::PostRound = gamestate().gameplay_state {
                    {
                        rsx! {
                            div { class: "container",
                                button {
                                    class: "{styles::STANDARD_BUTTON} text-white",
                                    onclick: move |_| {
                                        ws_send()
                                            .send(InnerMessage::GameMessage {
                                                msg: GameMessage {
                                                    username: user_config.read().username.clone(),
                                                    action: GameAction::Ack,
                                                    lobby: user_config.read().lobby_code.clone(),
                                                    timestamp: Utc::now(),
                                                },
                                            });
                                    },
                                    "Acknowledge"
                                }
                            }
                        }
                    }
                }
                if let GameplayState::End = gamestate().gameplay_state {
                    {
                        rsx! {
                            div { class: "container text-sm ",
                                div { "GAME OVER" }
                                {
                                    gamestate()
                                        .score
                                        .iter()
                                        .map(|(player, score)| {
                                            rsx! {
                                                li { "{player}: {score}" }
                                            }
                                        })
                                }
                            }
                            div { class: "container",
                                button {
                                    class: "{styles::STANDARD_BUTTON} text-white",
                                    onclick: move |_| {
                                        ws_send()
                                            .send(InnerMessage::GameMessage {
                                                msg: GameMessage {
                                                    username: user_config.read().username.clone(),
                                                    action: GameAction::Ack,
                                                    lobby: user_config.read().lobby_code.clone(),
                                                    timestamp: Utc::now(),
                                                },
                                            });
                                    },
                                    "Acknowledge"
                                }
                            }
                        }
                    }
                }
                // must be your turn and there are errors
                if gamestate.read().system_status.len() > 0
                    && gamestate.read().curr_player_turn.clone().unwrap_or("".to_string())
                        == user_config.read().username
                {
                    span { class: "text-red-500 text-sm",
                        "{gamestate.read().system_status.last().unwrap()}"
                    }
                }
            }
        }
    )
}

fn get_trump_svg(trump: &Suit) -> Element {
    let trump_svg = match trump {
        Suit::Spade => rsx!(
            svg {
                class: "ml-5",
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
                    "fill": "black",
                }
                rect {
                    "y": "40",
                    "x": "19",
                    width: "12",
                    "fill": "black",
                    height: "68",
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
                    "fill": "#FF0000",
                }
                path {
                    "fill": "#FF0000",
                    "d": "M0 25C0 11.1929 11.1929 -3.8147e-06 25 -3.8147e-06C38.8071 -3.8147e-06 50 11.1929 50 25C50 38.8071 38.8071 50 25 50C11.1929 50 0 38.8071 0 25Z",
                }
                path {
                    "d": "M50.5 99.5L97 37.9291L53.5 14L50.5 18.5L47.5 14L4 37.9291L50.5 99.5Z",
                    "fill": "#FF0000",
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
                    "transform": "rotate(-45 0 56.5685)",
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
                    "cy": "62",
                }
                circle {
                    "cx": "75",
                    "cy": "62",
                    "r": "25",
                    "fill": "black",
                }
                circle {
                    "fill": "black",
                    "cy": "25",
                    "cx": "50",
                    "r": "25",
                }
                rect {
                    "y": "40",
                    "x": "44",
                    width: "12",
                    height: "68",
                    "fill": "black",
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
                    width: "112.137",
                }
                rect {
                    "x": "0.500031",
                    "y": "0.499969",
                    width: "112.137",
                    height: "112.137",
                    "stroke": "black",
                }
                g { "filter": "url(#filter0_d_13_7)",
                    rect {
                        "x": "3.05176e-05",
                        height: "80",
                        "rx": "25",
                        "transform": "rotate(-45 3.05176e-05 56.5685)",
                        width: "80",
                        "fill": "white",
                        "y": "56.5685",
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
                            "result": "BackgroundImageFix",
                        }
                        feColorMatrix {
                            "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0",
                            "result": "hardAlpha",
                            "in": "SourceAlpha",
                            r#type: "matrix",
                        }
                        feOffset { "dy": "4" }
                        feGaussianBlur { "stdDeviation": "2" }
                        feComposite { "operator": "out", "in2": "hardAlpha" }
                        feColorMatrix {
                            "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.25 0",
                            r#type: "matrix",
                        }
                        feBlend {
                            "mode": "normal",
                            "in2": "BackgroundImageFix",
                            "result": "effect1_drop_13_7",
                        }
                        feBlend {
                            "mode": "normal",
                            "in2": "effect1_drop_13_7",
                            "in": "SourceGraphic",
                            "result": "shape",
                        }
                    }
                }
            }
        ),
    };

    return trump_svg;
}

// STEP 9: Add unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config() {
        let prod_config = ServerConfig::new(true);
        assert!(prod_config.server_url.contains("fly.dev"));

        let dev_config = ServerConfig::new(false);
        assert!(dev_config.server_url.contains("localhost"));
    }
}
