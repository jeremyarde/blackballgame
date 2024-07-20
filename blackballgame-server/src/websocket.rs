use axum::extract::ws::Message;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

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
use tokio::sync::Mutex;

use tokio::time::sleep;
use tracing::info;

use axum::extract::ws::{WebSocket, WebSocketUpgrade};

use common::PlayerRole;

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug)]
pub struct AppState {
    pub rooms: Mutex<HashMap<String, GameState>>,
    // players: Mutex<HashMap<(String, String), SplitSink<WebSocket, axum::extract::ws::Message>>>, // gameid, playerid
    pub room_broadcast_channel: Mutex<HashMap<String, tokio::sync::broadcast::Sender<GameState>>>,
    pub lobby_to_game_channel_send: Mutex<HashMap<String, tokio::sync::mpsc::Sender<GameMessage>>>,
    // lobby_to_game_channel_recv: Mutex<HashMap<String, tokio::sync::mpsc::Receiver<GameMessage>>>,
    // lobby_message_queue
}

#[derive(Serialize)]
pub struct ServerMessage {
    message: String,
    from: String,
}

impl ServerMessage {
    fn from(message: String, from: &str) -> Self {
        ServerMessage {
            message,
            from: from.to_string(),
        }
    }
}
async fn handle_socket(mut socket: WebSocket, who: SocketAddr, state: Arc<AppState>) {
    let mut username = String::new();
    let mut lobby_code = String::new();
    let mut created_new_game = false;
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

                let player_role: PlayerRole;

                // Check if there is a game already ongoing, and connect to the channel if yes
                let mut connected = false;

                let game = match rooms.get_mut(&connect.channel) {
                    Some(gamestate) => {
                        info!("Game already ongoing, joining");
                        created_new_game = false;
                        player_role = PlayerRole::Player;

                        // if player name is chosen and secret
                        let saved_secret = gamestate.players_secrets.get(&connect.username);
                        match (
                            gamestate.players.contains_key(&connect.username),
                            (saved_secret.is_some()
                                && connect.secret.is_some()
                                && saved_secret.unwrap().eq(&connect.secret.unwrap())),
                        ) {
                            // Username taken, secrets match
                            (true, true) => {
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

                                let channels = state.room_broadcast_channel.lock().await;
                                tx_from_game_to_client =
                                    Some(channels.get(&connect.channel).unwrap().clone());

                                username = connect.username.clone();
                                connected = true;
                            }
                            (true, false) => {
                                info!("Username taken or secrets don't match");
                                let _ = sender
                                    .send(Message::Text(
                                        json!(ServerMessage {
                                            message: format!("Username taken, attempted to reconnect or secrets did not match."),
                                            from: "System".to_string(),
                                        })
                                        .to_string(),
                                    ))
                                    .await;

                                connected = false;
                            }
                            (false, _) => {
                                println!("Username available in lobby, connecting");
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

                                let channels = state.room_broadcast_channel.lock().await;
                                tx_from_game_to_client =
                                    Some(channels.get(&connect.channel).unwrap().clone());

                                gamestate.add_player(connect.username.clone(), player_role);
                                let client_secret =
                                    gamestate.players_secrets.get(&connect.username).unwrap();

                                let _ = sender
                                    .send(Message::Text(
                                        json!({"client_secret": client_secret}).to_string(),
                                    ))
                                    .await;

                                username = connect.username.clone();
                                connected = true;
                            }
                            (_, _) => {
                                info!("Username is already taken, asking user to choose new one");
                                let _ = sender
                                    .send(Message::Text(
                                        json!(ServerMessage {
                                            message: "Username already taken.".to_string(),
                                            from: username.clone(),
                                        })
                                        .to_string(),
                                    ))
                                    .await;

                                connected = false;
                            }
                        }

                        gamestate
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
                        let server = GameState::new();
                        rooms.insert(connect.channel.clone(), server);

                        // channel that exists to transmit game state to each player
                        let broadcast_channel = tokio::sync::broadcast::channel(10).0;
                        {
                            let mut channels = state.room_broadcast_channel.lock().await;
                            channels.insert(connect.channel.clone(), broadcast_channel.clone());
                        }
                        tx_from_game_to_client = Some(broadcast_channel);

                        // setting up
                        let (lobby_sender, lobby_reciever) = tokio::sync::mpsc::channel(10);
                        {
                            let mut clientchannels = state.lobby_to_game_channel_send.lock().await;
                            clientchannels.insert(connect.channel.clone(), lobby_sender);

                            // let mut clientchannels = state.lobby_to_game_channel_recv.lock().await;
                            // clientchannels.insert(connect.channel.clone(), lobby_reciever);
                            recv_channel = Some(lobby_reciever);
                        }

                        let gamestate = rooms.get_mut(&connect.channel).unwrap();
                        gamestate.add_player(connect.username.clone(), player_role);

                        username = connect.username.clone();

                        let client_secret =
                            gamestate.players_secrets.get(&connect.username).unwrap();

                        let _ = sender
                            .send(Message::Text(
                                json!({"client_secret": client_secret}).to_string(),
                            ))
                            .await;

                        connected = true;

                        gamestate
                    }
                };

                info!("room users: {:?}", game.players);
                info!("Connected? {}", connected);
                if connected {
                    break;
                }
            }
        } else {
            info!("Message from user was not in Text format");

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
        }
    }

    info!("Subscribing to game messages...");

    let mut rx = tx_from_game_to_client.as_ref().unwrap().subscribe();
    info!("Subscribed to game messages - SUCCESS");
    // Recieve messages from client
    let username_for_recv = username.clone();
    let lobby_sender;
    {
        let channels = state.lobby_to_game_channel_send.lock().await;

        let mysender = match channels.get(&lobby_code) {
            Some(x) => x,
            None => panic!("Can't join a game that doesn't exist"),
        };

        lobby_sender = Some(mysender.clone());
    }

    // recieving messages from clients, passing to game
    let mut recv_messages_from_clients = tokio::spawn(async move {
        info!(
            "Reciever for user={} is now ready to accept messages.",
            username_for_recv
        );
        loop {
            let Some(Ok(Message::Text(msg))) = receiver.next().await else {
                continue;
            };

            info!("Attempt to deserialize GameMessage: {}", msg);
            let gamemessage: GameMessage = match serde_json::from_str(&msg) {
                Ok(x) => x,
                Err(err) => {
                    info!("Error deserializing GameMessage: {}", err);
                    continue;
                }
            };

            let _ = lobby_sender.clone().unwrap().send(gamemessage).await;
        }
        // info!("Exiting reciever thread for user={}", username_for_recv);
    });

    let username_for_send = username.clone();

    // sending messages to client
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

            info!("Exiting sender thread for user={}", username_for_send);
        })
    };

    // GAME SERVER THREAD, updates state from user input
    // only create a new thread when the first person has created a game
    if created_new_game && recv_channel.is_some() {
        let mut recv_channel_inner = recv_channel.unwrap();
        let internal_broadcast_clone = tx_from_game_to_client.unwrap().clone();
        let mut game_loop = {
            tokio::spawn(async move {
                let event_cap = 5;
                info!("Starting up game");
                loop {
                    let mut game_messages = Vec::with_capacity(event_cap);

                    info!("Waiting for messages");
                    recv_channel_inner
                        .recv_many(&mut game_messages, event_cap)
                        .await;

                    if game_messages.is_empty() {
                        sleep(Duration::from_millis(2000)).await;
                        continue;
                    }

                    info!("Got messages");
                    println!("Messages: {:?}", game_messages);
                    {
                        let mut rooms = state.rooms.lock().await;
                        let game = rooms.get_mut(&lobby_code).unwrap();

                        let gamestate = game.process_event(game_messages);

                        let _ = internal_broadcast_clone.send(gamestate);
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

pub async fn ws_handler(
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

pub async fn get_rooms(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rooms = match state.rooms.try_lock() {
        Ok(rooms) => rooms.keys().map(|key| key.clone()).collect::<Vec<String>>(),
        Err(_) => vec![String::from("Could not get rooms")],
    };

    rooms.join("\n")
    // rooms
}
