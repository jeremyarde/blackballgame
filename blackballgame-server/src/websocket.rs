use axum::extract::ws::Message;
use axum::extract::FromRef;
use axum::extract::Path;
use axum::extract::Query;
use common::PlayerSecret;
use futures_util::TryStreamExt;
use serde_json::Value;
use tokio::task::JoinHandle;
use tower_http::timeout::ResponseBodyTimeout;
use tracing::error;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::response::IntoResponse;

use axum_extra::headers;
use axum_extra::TypedHeader;
use common::GameMessage;
use common::GameState;
use futures_util::SinkExt;
use futures_util::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::Receiver;

use tokio::time::sleep;
use tracing::info;

use axum::extract::ws::{WebSocket, WebSocketUpgrade};

use common::PlayerRole;

pub type SharedState = Arc<RwLock<AppState>>;

// #[derive(Debug)]
// pub struct AppState {
//     pub rooms: RwLock<HashMap<String, GameState>>,
//     pub room_broadcast_channel: RwLock<HashMap<String, tokio::sync::broadcast::Sender<GameState>>>,
//     pub lobby_to_game_channel_send: RwLock<HashMap<String, tokio::sync::mpsc::Sender<GameMessage>>>,
//     pub game_threads: RwLock<HashMap<String, tokio::task::JoinHandle<()>>>,
// }

#[derive(Debug)]
pub struct AppState {
    pub rooms: HashMap<String, GameState>,
    pub room_broadcast_channel: HashMap<String, tokio::sync::broadcast::Sender<GameState>>,
    pub lobby_to_game_channel_send: HashMap<String, tokio::sync::mpsc::Sender<GameMessage>>,
    pub game_thread_channel: tokio::sync::mpsc::UnboundedSender<Message>,
    // pub game_threads: HashMap<String, tokio::task::JoinHandle<()>>,
}

#[derive(Serialize)]
pub struct ServerMessage {
    message: String,
    from: String,
}

// the api specific state
// #[derive(Clone)]
// pub struct GameRoomState {
//     pub rooms: HashMap<String, GameState>,
// }

// support converting an `AppState` in an `ApiState`
// impl FromRef<Arc<RwLock<AppState>>> for GameRoomState {
//     async fn from_ref(app_state: &Arc<RwLock<AppState>>) -> GameRoomState {
//         app_state.lock().await.rooms.clone()
//     }
// }

impl ServerMessage {
    fn from(message: String, from: &str) -> Self {
        ServerMessage {
            message,
            from: from.to_string(),
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    state: State<Arc<RwLock<AppState>>>,
    // Path(room_code): Path<Option<String>>,
) -> impl IntoResponse {
    info!("ws_handler - got request");
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    // info!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| {
        handle_socket(
            socket, addr, // room_code,
            state,
        )
    })
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    // room_code: Params,
    // room_code: Option<String>,
    State(state): State<Arc<RwLock<AppState>>>,
    // Path(room_code): Path<String>,
) {
    let mut username = String::new();
    let mut lobby_code = String::new();
    let mut game_thread_running = false;
    // let mut tx = None::<Sender<String>>;
    let mut tx_from_game_to_client = None::<tokio::sync::broadcast::Sender<GameState>>;
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

    {

        // initial connection and game setup
        // while let Some(Ok(msg)) = receiver.next().await {
        //     info!("Recieved: {:?}", msg);

        //     let mut message_content = String::new();
        //     if let Message::Text(content) = msg {
        //         message_content = content;
        //     } else {
        //         info!("Message from user was not in Text format: {:?}", msg);
        //         let _ = sender
        //             .send(Message::Text(
        //                 json!(ServerMessage {
        //                     message: format!(
        //                         "Wrong message format, try to connect again. Message: {:?}",
        //                         msg
        //                     ),
        //                     from: username.clone()
        //                 })
        //                 .to_string(),
        //             ))
        //             .await;
        //         continue;
        //     };

        //     info!("message from js: {}", message_content);

        //     #[derive(Deserialize, Debug)]
        //     struct Connect {
        //         username: String,
        //         channel: String,
        //         secret: Option<String>,
        //     }

        //     let connect: Connect = match serde_json::from_str::<Connect>(&message_content) {
        //         Ok(connect) => {
        //             info!("connect value: {:?}", connect);
        //             if connect.username.is_empty() || connect.channel.is_empty() {
        //                 info!("Username or channel is empty");
        //                 let _ = sender
        //                     .send(Message::Text(
        //                         json!(ServerMessage {
        //                             message: "Username or channel is empty".to_string(),
        //                             from: "System".into()
        //                         })
        //                         .to_string(),
        //                     ))
        //                     .await;
        //                 continue;
        //             }
        //             connect
        //         }
        //         Err(err) => {
        //             info!("{} had error: {}", &message_content, err);
        //             let _ = sender
        //                 .send(Message::Text(
        //                     json!(ServerMessage {
        //                         message: "Failed to connect".into(),
        //                         from: "Server".into()
        //                     })
        //                     .to_string(),
        //                 ))
        //                 .await;
        //             continue;
        //         }
        //     };

        //     info!(
        //         "{:?} is trying to connect to {:?}",
        //         connect.username, connect.channel
        //     );

        //     // {
        //     //     let mut rooms = &mut state.write().await.rooms;
        //     //     info!("All rooms: {:?}", rooms.keys());
        //     //     lobby_code = connect.channel.clone(); // set lobby code as what we connected with

        //     //     let player_role: PlayerRole;

        //     //     // Check if there is a game already ongoing, and connect to the channel if yes
        //     //     let mut connected = false;

        //     //     let game = match rooms.get_mut(&connect.channel) {
        //     //         Some(gamestate) => {
        //     //             info!("Game already ongoing, joining");
        //     //             game_thread_running = false;
        //     //             player_role = PlayerRole::Player;
        //     //             // We may have already created the lobby
        //     //             {
        //     //                 let (lobby_sender, lobby_reciever) = tokio::sync::mpsc::channel(10);
        //     //                 let mut clientchannels =
        //     //                     &mut state.write().await.lobby_to_game_channel_send;
        //     //                 if clientchannels.contains_key(&connect.channel) {
        //     //                     info!("Client channel already exists");
        //     //                 } else {
        //     //                     clientchannels.insert(connect.channel.clone(), lobby_sender);
        //     //                 }
        //     //                 recv_channel = Some(lobby_reciever);
        //     //             }

        //     //             // if player name is chosen and secret
        //     //             let saved_secret = gamestate.players_secrets.get(&connect.username);
        //     //             match (
        //     //                 gamestate.players.contains_key(&connect.username),
        //     //                 (saved_secret.is_some()
        //     //                     && connect.secret.is_some()
        //     //                     && saved_secret.unwrap().eq(&connect.secret.unwrap())),
        //     //             ) {
        //     //                 // Username taken, secrets match
        //     //                 (true, true) => {
        //     //                     info!("Secrets match, attempting to reconnect");

        //     //                     let _ = sender
        //     //                         .send(Message::Text(
        //     //                             json!(ServerMessage {
        //     //                                 message: format!("{} joined the game.", connect.username),
        //     //                                 from: "System".to_string(),
        //     //                             })
        //     //                             .to_string(),
        //     //                         ))
        //     //                         .await;

        //     //                     let channels = &state.read().await.room_broadcast_channel;
        //     //                     tx_from_game_to_client =
        //     //                         Some(channels.get(&connect.channel).unwrap().clone());

        //     //                     username = connect.username.clone();
        //     //                     connected = true;
        //     //                 }
        //     //                 (true, false) => {
        //     //                     info!("Username taken or secrets don't match");
        //     //                     let _ = sender
        //     //                             .send(Message::Text(
        //     //                                 json!(ServerMessage {
        //     //                                     message: "Username taken, attempted to reconnect or secrets did not match.".to_string(),
        //     //                                     from: "System".to_string(),
        //     //                                 })
        //     //                                 .to_string(),
        //     //                             ))
        //     //                             .await;

        //     //                     connected = false;
        //     //                 }
        //     //                 (false, _) => {
        //     //                     info!("Username available in lobby, connecting");
        //     //                     let _ = sender
        //     //                         .send(Message::Text(
        //     //                             json!(ServerMessage {
        //     //                                 message: format!("{} joined the game.", connect.username),
        //     //                                 from: "System".to_string(),
        //     //                             })
        //     //                             .to_string(),
        //     //                         ))
        //     //                         .await;

        //     //                     let channels = &mut state.write().await.room_broadcast_channel;
        //     //                     tx_from_game_to_client =
        //     //                         Some(channels.get(&connect.channel).unwrap().clone());

        //     //                     gamestate.add_player(connect.username.clone(), player_role);
        //     //                     let client_secret =
        //     //                         gamestate.players_secrets.get(&connect.username).unwrap();

        //     //                     let _ = sender
        //     //                         .send(Message::Text(
        //     //                             json!(PlayerSecret {
        //     //                                 client_secret: client_secret.clone(),
        //     //                             })
        //     //                             .to_string(),
        //     //                         ))
        //     //                         .await;

        //     //                     username = connect.username.clone();
        //     //                     connected = true;
        //     //                 }
        //     //                 (_, _) => {
        //     //                     info!("Username is already taken, asking user to choose new one");
        //     //                     let _ = sender
        //     //                         .send(Message::Text(
        //     //                             json!(ServerMessage {
        //     //                                 message: "Username already taken.".to_string(),
        //     //                                 from: username.clone(),
        //     //                             })
        //     //                             .to_string(),
        //     //                         ))
        //     //                         .await;

        //     //                     connected = false;
        //     //                 }
        //     //             }

        //     //             gamestate
        //     //         }
        //     //         None => {
        //     //             // this is not pretty
        //     //             info!("Created a new game");
        //     //             let _ = sender
        //     //                 .send(Message::Text(
        //     //                     json!(ServerMessage {
        //     //                         message: "User created a new game".to_string(),
        //     //                         from: "System".into()
        //     //                     })
        //     //                     .to_string(),
        //     //                 ))
        //     //                 .await;
        //     //             game_thread_running = true;
        //     //             player_role = PlayerRole::Leader;
        //     //             let server = GameState::new();
        //     //             rooms.insert(connect.channel.clone(), server);

        //     //             // channel that exists to transmit game state to each player
        //     //             let broadcast_channel = tokio::sync::broadcast::channel(10).0;
        //     //             {
        //     //                 let mut channels = &mut state.write().await.room_broadcast_channel;
        //     //                 channels.insert(connect.channel.clone(), broadcast_channel.clone());
        //     //             }
        //     //             tx_from_game_to_client = Some(broadcast_channel);

        //     //             // setting up
        //     //             {
        //     //                 let (lobby_sender, lobby_reciever) = tokio::sync::mpsc::channel(10);
        //     //                 let mut clientchannels =
        //     //                     &mut state.write().await.lobby_to_game_channel_send;
        //     //                 clientchannels.insert(connect.channel.clone(), lobby_sender);
        //     //                 recv_channel = Some(lobby_reciever);
        //     //             }

        //     //             let gamestate = rooms.get_mut(&connect.channel).unwrap();
        //     //             gamestate.add_player(connect.username.clone(), player_role);

        //     //             username = connect.username.clone();

        //     //             let client_secret = gamestate.players_secrets.get(&connect.username).unwrap();

        //     //             let _ = sender
        //     //                 .send(Message::Text(
        //     //                     json!({"client_secret": client_secret}).to_string(),
        //     //                 ))
        //     //                 .await;

        //     //             connected = true;

        //     //             gamestate
        //     //         }
        //     //     };

        //     //     info!("room users: {:?}", game.players);
        //     //     info!("Connected? {}", connected);
        //     //     if connected {
        //     //         break;
        //     //     }
        //     // }
        // }
    }

    {

        // info!("Subscribing to game messages...");
        // let mut rx = match tx_from_game_to_client.as_ref() {
        //     Some(x) => x.subscribe(),
        //     None => {
        //         error!("tx_from_game_to_client is None for user \"{}\"", username);
        //         return;
        //     }
        // };
        // info!("Subscribed to game messages - SUCCESS");

        // Recieve messages from client
        // let username_for_recv = username.clone();
        // let lobby_sender;
        // {
        //     let channels = &state.read().await.lobby_to_game_channel_send;
        //     info!(
        //         "Attempting to get lobby_sender for lobby {}, user {}",
        //         lobby_code, username
        //     );
        //     let mysender = match channels.get(&lobby_code) {
        //         Some(snd) => snd.clone(),
        //         None => panic!("Can't join a game that doesn't exist"),
        //     };

        //     lobby_sender = Some(mysender.clone());
        // }
    }

    // initial connection and game setup
    while let Ok(Some(msg)) = receiver.try_next().await {
        info!("Recieved: {:?}", msg);

        let mut message_content = String::new();
        if let Message::Text(content) = msg {
            message_content = content;
        } else {
            info!("Message from user was not in Text format: {:?}", msg);
            let _ = sender
                .send(Message::Text(
                    json!(ServerMessage {
                        message: format!(
                            "Wrong message format, try to connect again. Message: {:?}",
                            msg
                        ),
                        from: username.clone()
                    })
                    .to_string(),
                ))
                .await;
            continue;
        };

        #[derive(Deserialize, Debug)]
        struct Connect {
            username: String,
            channel: String,
            secret: Option<String>,
        }

        let connect: Connect = match serde_json::from_str::<Connect>(&message_content) {
            Ok(connect) => {
                info!("connect value: {:?}", connect);
                if connect.username.is_empty() || connect.channel.is_empty() {
                    info!("Username or channel is empty");
                    let _ = sender
                        .send(Message::Text(
                            json!(ServerMessage {
                                message: "Username or channel is empty".to_string(),
                                from: "System".into()
                            })
                            .to_string(),
                        ))
                        .await;
                    info!("Waiting for new connect message");
                    continue;
                }
                connect
            }
            Err(err) => {
                info!("{} had error: {}", &message_content, err);
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

        username = connect.username.clone();
        lobby_code = connect.channel.clone();
        break;
    }

    info!("Connected successfully to lobby {}", lobby_code);

    let bcast_channel: Sender<Value> = tokio::sync::broadcast::channel(10).0;

    let username_for_send = username.clone();
    // recieving messages from clients, passing to game
    let gamesender = state.write().await.game_thread_channel.clone();
    let mut recv_messages_from_clients = tokio::spawn(async move {
        info!(
            "Reciever for user={} is now ready to accept messages.",
            "username_for_recv"
        );
        loop {
            // let Some(Ok(Message::Text(msg))) = receiver.next().await {
            //     info!("Reciever got message");
            // };
            while let Some(Ok(msg)) = receiver.next().await {
                info!("Client: reciever got message");

                // info!("Attempt to deserialize GameMessage: {:?}", msg);
                // let gamemessage: GameMessage = match serde_json::from_str(&msg) {
                //     Ok(x) => x,
                //     Err(err) => {
                //         info!("Error deserializing GameMessage: {}", err);
                //         continue;
                //     }
                // };
                let _ = gamesender.send(msg);
                info!("[CLIENT] Sent message to game thread");
            }
            info!("[CLIENT] failed to get message");
        }
        info!(
            "[CLIENT] Exiting reciever thread for user={}",
            username.clone()
        );
    });

    let username_for_send = username_for_send.clone();

    // sending messages to client
    // let mut send_messages_to_client = {
    //     let recv_from_game = &state.read().await.room_broadcast_channel;
    //     let recv_broadcast = recv_from_game.get(&lobby_code).unwrap();
    //     let mut myrecv = recv_broadcast.subscribe();

    //     tokio::spawn(async move {
    //         info!(
    //             "Sender for user={} is now ready to accept messages.",
    //             username_for_send
    //         );
    //         // recieve message from a channel subscribed to events from any client
    //         while let Ok(text) = myrecv.recv().await {
    //             // while let Ok(text) = gamesender..await {
    //             // send message back to original client
    //             let _ = sender.send(Message::Text(json!(text).to_string())).await;
    //         }

    //         info!("Exiting sender thread for user={}", username_for_send);
    //     })
    // };

    // tokio::select! {
    //     _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
    //     _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
    // };

    info!("Waiting for joining thread to finish");
    tokio::join!(recv_messages_from_clients);
    info!("We lost the listening thread");
}
