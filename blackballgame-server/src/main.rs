use std::borrow::BorrowMut;
use std::borrow::Cow;
use std::clone;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::future::IntoFuture;
use std::io;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::str::Bytes;
use std::sync::Arc;
use std::task::Waker;
use std::time::Duration;

use axum::extract::ws::CloseFrame;
use axum::extract::ConnectInfo;
use axum::extract::Path;
use axum::extract::State;
use axum::http::Method;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use axum_extra::headers;
use axum_extra::TypedHeader;
use bevy::render::render_resource::encase::internal;
use bevy::utils::info;
use chrono::DateTime;
use chrono::Utc;
use client::GameClient;
use futures_util::stream::SplitSink;
use futures_util::stream::SplitStream;
use futures_util::SinkExt;
use futures_util::Stream;
use futures_util::StreamExt;
use game::GameServer;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

use tokio::time::sleep;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tracing::debug;
use tracing::info;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::client::PlayerRole;
use crate::game::GameEvent;
use crate::game::GameState;

mod client;
mod game;

/// Shorthand for the transmit half of the message channel.
type Tx = SplitSink<WebSocket, Message>;

/// Shorthand for the receive half of the message channel.
// type Rx = SplitSink<WebSocket, Message>;
type Rx = SplitStream<WebSocket>;

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

#[derive(Serialize)]
struct ServerMessage {
    message: String,
    from: String,
}

impl ServerMessage {
    fn from(message: String, from: &str) -> Self {
        return ServerMessage {
            message,
            from: from.to_string(),
        };
    }
}
async fn handle_socket(mut socket: WebSocket, who: SocketAddr, mut state: Arc<AppState>) {
    let mut username = String::new();
    let mut lobby_code = String::new();
    let mut created_new_game = false;
    // let mut tx = None::<Sender<String>>;
    let mut tx_from_game_to_client = None::<tokio::sync::broadcast::Sender<GameServer>>;
    let mut recv_channel: Option<tokio::sync::mpsc::Receiver<GameMessage>> = None;

    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        info!("Pinged {who}...");
    } else {
        info!("Could not send ping {who}!");
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    let (mut sender, mut receiver) = socket.split();

    // initial connection and game setup
    while let Some(Ok(msg)) = receiver.next().await {
        info!("Recieved: {:?}", msg);
        // try to connect at first
        if let Message::Text(name) = msg {
            info!("message from js: {}", name);

            #[derive(Deserialize, Debug)]
            struct Connect {
                username: String,
                channel: String,
                secret: Option<String>,
            }

            let connect: Connect = match serde_json::from_str(&name) {
                Ok(connect) => {
                    info!("connect value: {:?}", connect);
                    connect
                }
                Err(err) => {
                    info!("{} had error: {}", &name, err);
                    let _ = sender
                        .send(Message::Text(
                            json!(ServerMessage {
                                message: "Failed to connect".into(),
                                from: "Server".into()
                            })
                            .to_string(),
                        ))
                        .await;
                    continue;
                }
            };

            info!(
                "{:?} is trying to connect to {:?}",
                connect.username, connect.channel
            );

            {
                let mut rooms = state.rooms.lock().await;
                info!("All rooms: {:?}", rooms.keys());
                lobby_code = connect.channel.clone(); // set lobby code as what we connected with

                let mut player_role: PlayerRole;

                // Check if there is a game already ongoing, and connect to the channel if yes
                let game = match rooms.get_mut(&connect.channel) {
                    Some(gameserver) => {
                        info!("Game already ongoing, joining");
                        created_new_game = false;
                        player_role = PlayerRole::Player;
                        tx_from_game_to_client = Some(
                            state
                                .room_broadcast_channel
                                .lock()
                                .await
                                .get(&connect.channel)
                                .unwrap()
                                .clone(),
                        );

                        // if player name is chosen and secret
                        let saved_secret = gameserver.players_secrets.get(&connect.username);
                        match (
                            gameserver.players.contains_key(&connect.username),
                            (saved_secret.is_some() && connect.secret.is_some()),
                        ) {
                            (true, true) => {
                                // check if secrets match
                                info!("Room already had username, checking is secrets match");
                                if saved_secret.unwrap() == &connect.secret.unwrap() {
                                    info!("Secrets match, attempting to reconnect");

                                    let _ = sender
                                        .send(Message::Text(
                                            json!(ServerMessage {
                                                message: format!(
                                                    "{} joined the game.",
                                                    connect.username
                                                ),
                                                from: "System".to_string(),
                                            })
                                            .to_string(),
                                        ))
                                        .await;

                                    // don't need to replace the player
                                    // gameserver.players.insert(
                                    //     connect.username.to_owned(),
                                    //     GameClient::new(connect.username.clone(), player_role),
                                    // );
                                    username = connect.username.clone();
                                } else {
                                    info!("Could not reconnect because secrets don't match");
                                    let _ = sender
                                        .send(Message::Text(
                                            json!(ServerMessage {
                                                message: format!(
                                                    "{} joined the game.",
                                                    connect.username
                                                ),
                                                from: "System".to_string(),
                                            })
                                            .to_string(),
                                        ))
                                        .await;

                                    continue;
                                }
                            }
                            (false, _) => {
                                println!("Username available");
                                let _ = sender
                                    .send(Message::Text(
                                        json!(ServerMessage {
                                            message: format!(
                                                "{} joined the game.",
                                                connect.username
                                            ),
                                            from: "System".to_string(),
                                        })
                                        .to_string(),
                                    ))
                                    .await;

                                gameserver.players.insert(
                                    connect.username.to_owned(),
                                    GameClient::new(connect.username.clone(), player_role),
                                );

                                let client_secret = format!("sky_{}", connect.username.clone());

                                gameserver
                                    .players_secrets
                                    .insert(connect.username.clone(), client_secret.clone());

                                let _ = sender
                                    .send(Message::Text(
                                        json!({"client_secret": client_secret}).to_string(),
                                    ))
                                    .await;

                                username = connect.username.clone();
                            }
                            (_, _) => {
                                let _ = sender
                                    .send(Message::Text(
                                        json!(ServerMessage {
                                            message: "Username already taken.".to_string(),
                                            from: username.clone(),
                                        })
                                        .to_string(),
                                    ))
                                    .await;

                                continue;
                            }
                        }

                        gameserver
                    }
                    None => {
                        // this is not pretty
                        info!("Created a new game");
                        let _ = sender
                            .send(Message::Text(
                                json!(ServerMessage {
                                    message: "User created a new game".to_string(),
                                    from: "System".into()
                                })
                                .to_string(),
                            ))
                            .await;
                        created_new_game = true;
                        player_role = PlayerRole::Leader;
                        let server = GameServer::new();
                        rooms.insert(connect.channel.clone(), server);

                        let broadcast_channel = tokio::sync::broadcast::channel(10).0;
                        {
                            let mut channels = state.room_broadcast_channel.lock().await;
                            channels.insert(connect.channel.clone(), broadcast_channel.clone());
                        }
                        // let (lobby_sender, lobby_reciever) = tokio::sync::mpsc::channel(10);
                        let (lobby_sender, lobby_reciever) = tokio::sync::mpsc::channel(10);
                        {
                            let mut clientchannels = state.lobby_to_game_channel_send.lock().await;
                            clientchannels.insert(connect.channel.clone(), lobby_sender);

                            // let mut clientchannels = state.lobby_to_game_channel_recv.lock().await;
                            // clientchannels.insert(connect.channel.clone(), lobby_reciever);
                            recv_channel = Some(lobby_reciever);
                        }

                        tx_from_game_to_client = Some(broadcast_channel);

                        let gameserver = rooms.get_mut(&connect.channel).unwrap();

                        gameserver.players.insert(
                            connect.username.to_owned(),
                            GameClient::new(connect.username.clone(), player_role),
                        );
                        // insert secret for reconnects
                        let client_secret = connect.username.clone();
                        gameserver
                            .players_secrets
                            .insert(connect.username.clone(), client_secret.clone());
                        username = connect.username.clone();

                        let _ = sender
                            .send(Message::Text(
                                json!({"client_secret": client_secret}).to_string(),
                            ))
                            .await;

                        gameserver
                    }
                };

                info!("room users: {:?}", game.players);
                info!("Connection completed.");
                break;
            }
        } else {
            let _ = sender
                .send(Message::Text(
                    json!(ServerMessage {
                        message: "Wrong message format, try to connect again".to_string(),
                        from: username.clone()
                    })
                    .to_string(),
                ))
                .await;
        }
    }

    let mut rx = tx_from_game_to_client.as_ref().unwrap().subscribe();

    // Recieve messages from client
    let username_for_recv = username.clone();
    let mut cloned_sender = None::<Sender<GameMessage>>;
    // let (tx_game_messages, mut rx_game_messages) = tokio::sync::mpsc::channel::<GameMessage>(100);
    {
        let channels = state.lobby_to_game_channel_send.lock().await;

        let mysender = match channels.get(&lobby_code) {
            Some(x) => x,
            None => panic!("Can't join a game that doesn't exist"),
        };

        cloned_sender = Some(mysender.clone());
    }

    let mut recv_messages_from_clients = tokio::spawn(async move {
        info!(
            "Reciever for user={} is now ready to accept messages.",
            username_for_recv
        );
        while let Some(Ok(Message::Text(msg))) = receiver.next().await {
            info!("Attempt to deserialize GameMessage: {}", msg);
            let gamemessage: GameMessage = match serde_json::from_str(&msg) {
                Ok(x) => x,
                Err(err) => {
                    info!("Error deserializing GameMessage: {}", err);
                    continue;
                }
            };

            let _ = cloned_sender.clone().unwrap().send(gamemessage).await;
        }
    });

    let username_for_send = username.clone();

    let mut send_messages_to_client = {
        tokio::spawn(async move {
            info!(
                "Sender for user={} is now ready to accept messages.",
                username_for_send
            );
            // recieve message from a channel subscribed to events from any client
            while let Ok(text) = rx.recv().await {
                // send message back to original client
                let _ = sender.send(Message::Text(json!(text).to_string())).await;
            }
        })
    };

    // only create a new thread when the first person has created a game
    if created_new_game && recv_channel.is_some() {
        let mut recv_channel_inner = recv_channel.unwrap();
        let internal_broadcast_clone = tx_from_game_to_client.unwrap().clone();
        let mut game_loop = {
            tokio::spawn(async move {
                let event_cap = 5;
                let mymessagechannel = info!("Starting up game");
                loop {
                    let mut game_messages = Vec::with_capacity(event_cap);

                    info!("Waiting for messages");
                    recv_channel_inner
                        .recv_many(&mut game_messages, event_cap)
                        .await;

                    if game_messages.len() == 0 {
                        sleep(Duration::from_millis(2000)).await;
                        continue;
                    }

                    info!("Got messages");
                    println!("Messages: {:?}", game_messages);
                    {
                        let mut rooms = state.rooms.lock().await;
                        let game = rooms.get_mut(&lobby_code).unwrap();

                        game.process_event(game_messages, &internal_broadcast_clone);
                        // if let Some(state) = state {
                        //     let _ = internal_broadcast_clone.send(state);
                        // }
                    }

                    sleep(Duration::from_millis(500)).await;
                }
            })
        };

        tokio::select! {
            _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
            _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
            _ = (&mut game_loop) => game_loop.abort(),
        };
    } else {
        tokio::select! {
            _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
            _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
        };
    }
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct GameMessage {
    username: String,
    message: GameEvent,
    timestamp: DateTime<Utc>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    // info!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn root() -> &'static str {
    "Hello, World"
}

#[derive(Debug)]
struct AppState {
    rooms: Mutex<HashMap<String, GameServer>>,
    // players: Mutex<HashMap<(String, String), SplitSink<WebSocket, axum::extract::ws::Message>>>, // gameid, playerid
    room_broadcast_channel: Mutex<HashMap<String, tokio::sync::broadcast::Sender<GameServer>>>,
    lobby_to_game_channel_send: Mutex<HashMap<String, tokio::sync::mpsc::Sender<GameMessage>>>,
    // lobby_to_game_channel_recv: Mutex<HashMap<String, tokio::sync::mpsc::Receiver<GameMessage>>>,
    // lobby_message_queue
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("blackballgame=debug".parse().unwrap()),
        )
        .with_span_events(FmtSpan::FULL)
        // .with_thread_names(true) // only says "tokio-runtime-worker"
        .with_thread_ids(true)
        .finish()
        .init();

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let assets_dir =
        PathBuf::from("/Users/jarde/Documents/code/blackballgame/blackballgame-client/dist");

    // let serverstate: Arc<AppState> = Arc::new(allgames);
    let serverstate = Arc::new(AppState {
        rooms: Mutex::new(HashMap::new()),
        // players: Mutex::new(HashMap::new()),
        room_broadcast_channel: Mutex::new(HashMap::new()),
        lobby_to_game_channel_send: Mutex::new(HashMap::new()),
        // lobby_to_game_channel_recv: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        // `GET /` goes to `root`
        // .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        // .route("/", get(root))
        .route("/ws", get(ws_handler))
        .nest_service(
            "/",
            ServeDir::new(assets_dir)
                .fallback(ServeFile::new("blackballgame-server/assets/index.html")),
        )
        .layer(cors)
        // .route("/ui".get(ServeDir::new(assets_dir).append_index_html_on_directories(true)))
        // .route("/game", get(Game))
        .with_state(serverstate);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
