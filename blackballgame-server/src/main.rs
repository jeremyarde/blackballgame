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
use chrono::format;
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
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tracing::debug;
use tracing::info;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

use crate::client::PlayerRole;

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

    server.play_game(max_rounds);
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
    let mut channel = String::new();
    // let mut tx = None::<Sender<String>>;
    let mut internal_send = None::<tokio::sync::broadcast::Sender<String>>;

    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged {who}...");
    } else {
        println!("Could not send ping {who}!");
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        println!("Recieved: {:?}", msg);
        // try to connect at first
        if let Message::Text(name) = msg {
            println!("message from js: {}", name);

            #[derive(Deserialize, Debug)]
            struct Connect {
                username: String,
                channel: String,
            }

            let connect: Connect = match serde_json::from_str(&name) {
                Ok(connect) => {
                    println!("connect value: {:?}", connect);
                    connect
                }
                Err(err) => {
                    println!("{} had error: {}", &name, err);
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

            println!(
                "{:?} is trying to connect to {:?}",
                connect.username, connect.channel
            );

            {
                let mut rooms = state.rooms.lock().await;
                println!("All rooms: {:?}", rooms);
                // channel = connect.channel.clone();
                // println!("channel value: {}", channel);

                let mut player_role: PlayerRole;

                let game = match rooms.get_mut(&connect.channel) {
                    Some(x) => {
                        println!("Game already ongoing, joining");
                        player_role = PlayerRole::Player;
                        x
                    }
                    None => {
                        // this is not pretty
                        println!("Created a new game");
                        player_role = PlayerRole::Leader;
                        let server = GameServer::new();
                        rooms.insert(connect.channel.clone(), server);
                        rooms.get_mut(&connect.channel).unwrap()
                    }
                };

                let tx = Some(game.tx.clone());
                // println!("All rooms: {:?}", &rooms);
                println!("room users: {:?}", game.players);
                // println!("room tx: {:?}", game.tx);

                // tx = Some(game.tx.clone());
                if !game.players.contains_key(&connect.username) {
                    println!("room did not contain user, adding them...");
                    let _ = sender
                        .send(Message::Text(
                            json!(ServerMessage {
                                message: format!("Joined the game: {}", username),
                                from: "Server".to_string(),
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

    // let (send_chnl, mut rec_chnl) = tokio::sync::mpsc::channel::<GameMessage>(10);
    // let (internal_send, mut _internal_recv) = tokio::sync::broadcast::channel(42);
    // internal_send.sub
    let tx = internal_send.unwrap();
    let mut rx = tx.subscribe();

    // Recieve from client
    let channel_for_recv = channel.clone();
    let username_for_recv = username.clone();

    let mut recv_messages_from_clients = tokio::spawn(async move {
        while let Some(Ok(Message::Text(msg))) = receiver.next().await {
            let mut gameguard = state.rooms.lock().await;
            let game = gameguard.get_mut(&channel_for_recv).unwrap();
            let gamestate = game.get_state();
            // process the game and send a different message
            let messagetosend = json!(ServerMessage::from(
                json!(gamestate).to_string(),
                username_for_recv.clone().as_str()
            ))
            .to_string();

            for display_name in state.rooms.lock().await.keys() {
                println!("Games: {}", &display_name);
            }

            if game.tx.send(messagetosend).is_err() {
                info!("Could not send a message, breaking");
                break;
            }
        }
    });

    let channel_for_send = channel.clone();
    let username_for_send = username.clone();

    let mut send_messages_to_client = {
        // let tx = tx.clone();
        // let name = username.clone();
        tokio::spawn(async move {
            // println!("send_messages_to_client | {} --", name);
            while let Ok(text) = rx.recv().await {
                // while let Some(text) = rx.recv().await {
                println!("-> {:?}", text);

                // if let Message::Text(text) = text {
                let game_message: GameMessage = match serde_json::from_str(&text) {
                    Ok(x) => x,
                    Err(err) => {
                        println!("Error deserializing game message: {}", err);
                        continue;
                    }
                };

                // let game = state.rooms.lock().await.get_mut(&channel).unwrap();

                let _ = sender
                    .send(Message::Text(
                        json!(ServerMessage::from(
                            format!("something happened in game: {}", channel),
                            &username_for_send
                        ))
                        .to_string(),
                    ))
                    .await;

                // for (key, mut player) in state
                //     .rooms
                //     .lock()
                //     .await
                //     .get_mut(&channel)
                //     .unwrap()
                //     .players
                //     .iter_mut()
                // {
                //     if player
                //         .sender
                //         .send(Message::Text("hey this better work :)".to_string()))
                //         .await
                //         .is_err()
                //     {
                //         break;
                //     }
                // }
            }
        })
    };

    tokio::select! {
        _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
        _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
    };
}

#[derive(Deserialize, Debug, Serialize)]
pub struct GameMessage {
    username: String,
    message: String,
    timestamp: DateTime<Utc>,
}

fn fun_name(message: &GameMessage, name: &String) -> ControlFlow<()> {
    println!("{} says {:?}", name, message);
    // let servermessage = ServerMessage {
    //     message: "hey, this is nice".to_string(),
    //     from: name.clone(),
    // };
    // let _ = tx.send(json!(servermessage).to_string());

    ControlFlow::Continue(())
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
    // println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn root() -> &'static str {
    "Hello, World"
}

#[derive(Debug)]
struct AppState {
    // rooms: Mutex<HashMap<String, RoomState>>,
    rooms: Mutex<HashMap<String, GameServer>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("blackballgame-server=info".parse().unwrap()),
        )
        .with_span_events(FmtSpan::FULL)
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

    // loop {
    //     tracing::info!("Waiting for inbound TcpStream");
    //     // Asynchronously wait for an inbound TcpStream.
    //     let (stream, addr) = listener.accept().await.unwrap();

    //     tracing::info!("Got message, beginning game...");

    //     // Clone a handle to the `Shared` state for the new connection.
    //     let state = Arc::clone(&serverstate);

    //     // Spawn our handler to be run asynchronously.
    //     tokio::spawn(async move {
    //         tracing::debug!("accepted connection");
    //         if let Err(e) = server_process(state, stream, addr).await {
    //             tracing::info!("an error occurred; error = {:?}", e);
    //         }
    //     });
    // }
}
