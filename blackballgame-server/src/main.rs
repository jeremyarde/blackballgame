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
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tracing::info;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

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

// impl PartialOrd for Card {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         // match self.id.partial_cmp(&other.id) {
//         //Some(core::cmp::Ordering::Equal) => {}
//         //ord => return ord,
//         // }
//         match self.suit.partial_cmp(&other.suit) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         match self.value.partial_cmp(&other.value) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         // self.played_by.partial_cmp(&other.played_by)
//     }
// }

/// Shorthand for the transmit half of the message channel.
type Tx = SplitSink<WebSocket, Message>;

/// Shorthand for the receive half of the message channel.
// type Rx = SplitSink<WebSocket, Message>;
type Rx = SplitStream<WebSocket>;

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

//  helper to print contents of messages to stdout. Has special treatment for Close.
fn process_message(msg: Message, who: SocketAddr, mut game: Arc<AppState>) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {who} somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>> {who} sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {who} sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(())
}

/// Actual websocket statemachine (one will be spawned per connection)
// async fn handle_socket(mut socket: WebSocket, who: SocketAddr, mut state: Arc<AppState>) {
//     // send a ping (unsupported by some browsers) just to kick things off and get a response
//     if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
//         println!("Pinged {who}...");
//     } else {
//         println!("Could not send ping {who}!");
//         // no Error here since the only thing we can do is to close the connection.
//         // If we can not send messages, there is no way to salvage the statemachine anyway.
//         return;
//     }

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, mut state: Arc<AppState>) {
    let mut username = String::new();
    let mut channel = String::new();
    let mut tx = None::<broadcast::Sender<String>>;

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
                        .send(Message::Text("Failed to connect".to_string()))
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
                // channel = connect.channel.clone();
                // println!("channel value: {}", channel);
                let room = rooms.entry(connect.channel).or_insert_with(RoomState::new);
                println!("room users: {:?}", room.users);
                println!("room tx: {:?}", room.tx);

                tx = Some(room.tx.clone());
                if !room.users.lock().await.contains(&connect.username) {
                    println!("room did not contain user, adding them...");
                    room.users.lock().await.insert(connect.username.to_owned());
                    username = connect.username.clone();
                } else {
                    sender
                        .send(Message::Text("Username already taken.".to_string()))
                        .await;
                }
            }

            break;
        } else {
            sender
                .send(Message::Text("Wrong message format.".into()))
                .await;
        }
    }

    let tx = tx.unwrap();
    let mut rx = tx.subscribe();

    tx.send("someone has joined".into());
    let mut recv_messages = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.clone())).await.is_err() {
                break;
            }
            println!("recieved: {}", msg);
        }
    });

    let mut send_messages = {
        let tx = tx.clone();
        let name = username.clone();
        tokio::spawn(async move {
            while let Some(Ok(Message::Text(text))) = receiver.next().await {
                println!("{} says {}", name, text);
                let _ = tx.send(format!("{}: {}", name, text));
            }
        })
    };

    tokio::select! {
        _ = (&mut send_messages) => recv_messages.abort(),
        _ = (&mut recv_messages) => send_messages.abort(),
    };

    // {
    //     let mut rooms = state.rooms.lock().await;
    //     // channel = connect.channel.clone();
    //     // println!("channel value: {}", channel);
    //     let room = rooms.entry(connect.channel).or_insert_with(RoomState::new);
    //     println!("room users: {:?}", room.users);
    //     println!("room tx: {:?}", room.tx);

    //     tx = Some(room.tx.clone());
    //     if !room.users.lock().await.contains(&connect.username) {
    //         println!("room did not contain user, adding them...");
    //         room.users.lock().await.insert(connect.username.to_owned());
    //         username = connect.username.clone();
    //     }
    // }
    // if username.is_empty() {
    //     let _ = socket
    //         .send(Message::Text(String::from("Username already taken.")))
    //         .await;
    //     return;
    // }

    // let tx = tx.unwrap();
    // let mut rx = tx.subscribe();

    // let joined = format!("{} joined the chat!", username);
    // let _ = tx.send(joined);

    // let mut recv_messages = tokio::spawn(async move {
    //     while let Ok(msg) = rx.recv().await {
    //         if sender.send(Message::Text(msg)).await.is_err() {
    //             break;
    //         }
    //         println!("{}", msg);
    //     }
    // });

    // let mut send_messages = {
    //     let tx = tx.clone();
    //     let name = username.clone();
    //     tokio::spawn(async move {
    //         while let Some(Ok(Message::Text(text))) = receiver.next().await {
    //             println!("{} says {}", name, text);
    //             let _ = tx.send(format!("{}: {}", name, text));
    //         }
    //     })
    // };

    // tokio::select! {
    //     _ = (&mut send_messages) => recv_messages.abort(),
    //     _ = (&mut recv_messages) => send_messages.abort(),
    // };

    // let left = format!("{} left the chat!", username);
    // let _ = tx.send(left);
    // let mut rooms = state.rooms.lock().await;
    // rooms
    //     .get_mut(&channel)
    //     .unwrap()
    //     .users
    //     .lock()
    //     .await
    //     .remove(&username);

    // if rooms.get_mut(&channel).unwrap().users.lock().await.len() == 0 {
    //     rooms.remove(&channel);
    // }
}

//     // receive single message from a client (we can either receive or send with socket).
//     // this will likely be the Pong for our Ping or a hello message from client.
//     // waiting for message from a client will block this task, but will not block other client's
//     // connections.
//     if let Some(msg) = socket.recv().await {
//         if let Ok(msg) = msg {
//             if process_message(msg, who, state.clone()).is_break() {
//                 return;
//             }
//         } else {
//             println!("client {who} abruptly disconnected");
//             return;
//         }
//     }

//     // By splitting socket we can send and receive at the same time. In this example we will send
//     // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
//     let (mut sender, mut receiver) = socket.split();

//     // Spawn a task that will push several messages to the client (does not matter what client does)
//     let mut send_task = tokio::spawn(async move {
//         let mut input = String::new();

//         while input != "q" {
//             io::stdin()
//                 .read_line(&mut input)
//                 .expect("error: unable to read user input");

//             let cleaned = input.trim();
//             println!("Sending {} -> {}", cleaned, who);
//             sender.send(Message::Text(cleaned.to_string())).await;
//             // input = String::new();
//         }

//         println!("Sending close to {who}...");
//         if let Err(e) = sender
//             .send(Message::Close(Some(CloseFrame {
//                 code: axum::extract::ws::close_code::NORMAL,
//                 reason: Cow::from("Goodbye"),
//             })))
//             .await
//         {
//             println!("Could not send Close due to {e}, probably it is ok?");
//         }
//         1
//     });

//     // This second task will receive messages from client and print them on server console
//     let mut recv_task = tokio::spawn(async move {
//         let mut cnt = 0;
//         while let Some(Ok(msg)) = receiver.next().await {
//             cnt += 1;
//             // print message and break if instructed to do so
//             if process_message(msg, who, state.clone()).is_break() {
//                 break;
//             }
//         }
//         cnt
//     });

//     // If any one of the tasks exit, abort the other.
//     tokio::select! {
//         rv_a = (&mut send_task) => {
//             match rv_a {
//                 Ok(a) => println!("{a} messages sent to {who}"),
//                 Err(a) => println!("Error sending messages {a:?}")
//             }
//             recv_task.abort();
//         },
//         rv_b = (&mut recv_task) => {
//             match rv_b {
//                 Ok(b) => println!("Received {b} messages"),
//                 Err(b) => println!("Error receiving messages {b:?}")
//             }
//             send_task.abort();
//         }
//     }

//     // returning from the handler closes the websocket connection
//     println!("Websocket context {who} destroyed");
// }

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

struct AppState {
    rooms: Mutex<HashMap<String, RoomState>>,
}

struct RoomState {
    users: Mutex<HashSet<String>>,
    tx: broadcast::Sender<String>,
}

impl RoomState {
    fn new() -> Self {
        Self {
            users: Mutex::new(HashSet::new()),
            tx: broadcast::channel(69).0,
        }
    }
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
