use std::borrow::BorrowMut;
use std::borrow::Cow;
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
use std::task::Context;
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
use crate::game::FullGameState;
use crate::game::GameEvent;
use crate::game::GameState;

mod client;
mod game;

#[derive(Debug, Clone, Copy)]
enum ServerError {}

// struct AppState {
//     games: Mutex<HashMap<String, GameServer>>,
// }

async fn server_process(
    state: Arc<Mutex<GameServer>>,
    mut stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), ServerError> {
    tracing::info!("Starting up server...");

    let mut server = state.lock().await;

    stream.write_all(b"hello, world").await.unwrap();
    info!("success writing some bytes");
    // wait for people to connect
    // start game, ask for input from people, progress game
    let max_rounds = Some(3);

    server.setup_game(max_rounds);
    Ok(())
}

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
    // let mut tx = None::<Sender<String>>;
    let mut tx_from_game_to_client = None::<tokio::sync::broadcast::Sender<FullGameState>>;

    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        info!("Pinged {who}...");
    } else {
        info!("Could not send ping {who}!");
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        info!("Recieved: {:?}", msg);
        // try to connect at first
        if let Message::Text(name) = msg {
            info!("message from js: {}", name);

            #[derive(Deserialize, Debug)]
            struct Connect {
                username: String,
                channel: String,
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
                    break;
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
                    Some(x) => {
                        info!("Game already ongoing, joining");
                        player_role = PlayerRole::Player;
                        tx_from_game_to_client = Some(tokio::sync::broadcast::channel(10).0);
                        x
                    }
                    None => {
                        // this is not pretty
                        info!("Created a new game");
                        player_role = PlayerRole::Leader;
                        let server = GameServer::new();
                        rooms.insert(connect.channel.clone(), server);
                        tx_from_game_to_client = Some(
                            state
                                .room_broadcast_channel
                                .lock()
                                .await
                                .get(&connect.channel)
                                .unwrap()
                                .clone(),
                        );
                        rooms.get_mut(&connect.channel).unwrap()
                    }
                };

                // internal_send = Some(game.tx.clone());

                info!("room users: {:?}", game.players);

                if !game.players.contains_key(&connect.username) {
                    info!("room did not contain user, adding them...");
                    let _ = sender
                        .send(Message::Text(
                            json!(ServerMessage {
                                message: format!("{} joined the game.", username),
                                from: "System".to_string(),
                            })
                            .to_string(),
                        ))
                        .await;

                    game.players.insert(
                        connect.username.to_owned(),
                        GameClient::new(connect.username.clone(), player_role),
                    );
                    username = connect.username.clone();
                } else {
                    let _ = sender
                        .send(Message::Text(
                            json!(ServerMessage {
                                message: "Username already taken.".to_string(),
                                from: username.clone(),
                            })
                            .to_string(),
                        ))
                        .await;
                }
            }

            break;
        } else {
            let _ = sender
                .send(Message::Text(
                    json!(ServerMessage {
                        message: "Wrong message format".to_string(),
                        from: username.clone()
                    })
                    .to_string(),
                ))
                .await;
        }
    }
    // let room = state.room_broadcast_channel.lock().await;
    let rx = tx_from_game_to_client.unwrap().subscribe();
    // let tx = room.get(&lobby_code).unwrap();

    // let tx = room.get.unwrap();

    // let mut rx = tx.subscribe();

    // Recieve from client
    // let channel_for_recv = lobby_code.clone();
    let username_for_recv = username.clone();

    let (tx_game_messages, mut rx_game_messages) = tokio::sync::mpsc::channel::<GameMessage>(100);
    // let queue_for_recv = queue.clone();
    // let shared_game_message_queue = Arc::new(vec![]);
    // let shared_game_message_queue_2 = shared_game_message_queue.clone();

    let mut recv_messages_from_clients = tokio::spawn(async move {
        info!(
            "Reciever for user={} is now ready to accept messages.",
            username_for_recv
        );
        while let Some(Ok(Message::Text(msg))) = receiver.next().await {
            let gamemessage: GameMessage = match serde_json::from_str(&msg) {
                Ok(x) => x,
                Err(err) => {
                    info!("Error deserializing GameMessage: {}", err);
                    continue;
                }
            };

            // let _ = tx_game_messages.send(gamemessage);
            let _ = tx_game_messages.send(gamemessage).await;

            // let gamestate;
            // let internal_sender;
            // {
            //     let mut gameguard = state.rooms.lock().await;
            //     let game = gameguard.get_mut(&channel_for_recv).unwrap();
            //     // game.process_event(gamemessage);
            //     gamestate = game.get_state();
            //     internal_sender = game.tx.clone();
            // }

            // if internal_sender.send(gamestate).is_err() {
            //     info!("Could not send a message, breaking");
            //     break;
            // }
        }
    });

    // let channel_for_send = lobby_code.clone();
    let username_for_send = username.clone();

    let mut send_messages_to_client = {
        tokio::spawn(async move {
            info!(
                "Sender for user={} is now ready to accept messages.",
                username_for_send
            );
            // recieve message from a channel subscribed to events from any client
            while let Ok(text) = rx.recv().await {
                info!("messaging client -> {:?}", text);

                // let game_message: GameMessage = match serde_json::from_str(&text) {
                //     Ok(x) => x,
                //     Err(err) => {
                //         info!("Error deserializing game message: {}", err);
                //         continue;
                //     }
                // };

                // send message back to original client
                let _ = sender.send(Message::Text(json!(text).to_string())).await;
            }
        })
    };

    let internal_broadcast_clone = tx_from_game_to_client.unwrap().clone();
    let mut game_loop = {
        tokio::spawn(async move {
            // state.rooms.lock().await.get_mut(lobby_code)
            let mut newgame = GameServer::new();
            let event_cap = 5;

            info!("Starting up game");
            loop {
                let mut game_messages = Vec::with_capacity(event_cap);

                info!("Waiting for messages");
                rx_game_messages
                    .recv_many(&mut game_messages, event_cap)
                    .await;

                if game_messages.len() == 0 {
                    sleep(Duration::from_millis(2000)).await;
                    continue;
                }

                info!("Got messages");
                println!("Messages: {:?}", game_messages);
                let state: Option<FullGameState> = newgame.process_event(game_messages);

                if let Some(state) = state {
                    let _ = internal_broadcast_clone.send(state);
                } else {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        })
    };

    tokio::select! {
        _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
        _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
        _ = (&mut game_loop) => game_loop.abort(),
    };
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
    players: Mutex<HashMap<(String, String), SplitSink<WebSocket, axum::extract::ws::Message>>>, // gameid, playerid
    room_broadcast_channel: Mutex<HashMap<String, tokio::sync::broadcast::Sender<FullGameState>>>,
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
        players: Mutex::new(HashMap::new()),
        room_broadcast_channel: Mutex::new(HashMap::new()),
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
