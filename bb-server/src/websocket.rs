use axum::extract::ws::Message;
use axum::extract::FromRef;
use axum::extract::Path;
use axum::extract::Query;
use axum_extra::headers::UserAgent;
use chrono::Utc;
// use common::Connect;
use common::Destination;
use common::GameAction;
use common::GameEventResult;
use common::PlayerSecret;
use futures_util::TryStreamExt;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::timeout::ResponseBodyTimeout;
use tracing::debug;
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

#[derive(Debug)]
pub struct AppState {
    pub rooms: HashMap<String, GameState>,
    pub room_broadcast_channel: HashMap<String, tokio::sync::broadcast::Sender<GameState>>,
    pub lobby_to_game_channel_send: HashMap<String, tokio::sync::mpsc::Sender<GameMessage>>,
    pub game_thread_channel: tokio::sync::mpsc::UnboundedSender<GameMessage>,
    pub game_to_client_sender: tokio::sync::broadcast::Sender<GameEventResult>,
    // pub game_threads: HashMap<String, tokio::task::JoinHandle<()>>,
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

#[axum::debug_handler]
pub async fn ws_handler(
    // Path(room_code): Path<String>,
    // ws: WebSocketUpgrade,
    // user_agent: Option<TypedHeader<headers::UserAgent>>,
    // ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    state: State<Arc<RwLock<AppState>>>,
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
            user_agent, state,
        )
    })
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    user_agent: String,
    State(state): State<Arc<RwLock<AppState>>>,
) {
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
    // let user_ip = format("{}{}"who.ip().to_string();
    let user_ip = who.to_string();
    info!("User ip: {user_ip}");

    let (mut sender, mut receiver) = socket.split();

    let bcast_channel: Sender<Value> = tokio::sync::broadcast::channel(10).0;

    // recieving messages from clients, passing to game
    let recv_user_ip = user_ip.clone();
    let gamesender = state.write().await.game_thread_channel.clone();
    let mut recv_messages_from_clients = tokio::spawn(async move {
        info!(
            "[CLIENT-RECEIVER] Reciever for user={} is now ready to accept messages.",
            recv_user_ip
        );
        let mut error_counter = 0;
        while error_counter < 10 {
            info!("[CLIENT-RECEIVER] looping over messages now");
            while let Some(Ok(Message::Text(msg))) = receiver.next().await {
                info!("[CLIENT-RECEIVER] reciever got message");
                let internalmsg = match serde_json::from_str::<GameMessage>(&msg) {
                    Ok(mut gm) => {
                        match &mut gm.action {
                            // GameAction::PlayCard(card) => todo!(),
                            // GameAction::Bid(_) => todo!(),
                            // GameAction::Ack => todo!(),
                            // GameAction::StartGame(setup_game_options) => todo!(),
                            // GameAction::Deal => todo!(),
                            // GameAction::CurrentState => todo!(),
                            GameAction::Connect(player_details) => {
                                player_details.ip = Some(recv_user_ip.clone());
                            }
                            GameAction::JoinGame(player_details) => {
                                player_details.ip = Some(recv_user_ip.clone());
                            }
                            _ => {}
                        }
                        // if let GameAction::JoinGame(ref mut playerdetails) = &mut gm.action {
                        //     playerdetails.ip = recv_user_ip.clone();
                        // }

                        let _ = gamesender.send(gm);
                    }
                    Err(err) => info!("[CLIENT-RECEIVER] Error deserializing GameMessage: {}", err),
                };
                error_counter = 0;
            }
            info!(
                "[CLIENT-RECEIVER] message loop exited. Error count: {}",
                error_counter
            );
            error_counter += 1;
        }
        info!(
            "[CLIENT-RECEIVER] Exiting reciever thread for user={}",
            who.clone()
        );
    });

    let mut broadcast_channel = state.read().await.game_to_client_sender.subscribe();
    // let from_game_broadcast = &state.read().await.gamechannel_broadcast_send.clone();

    // let this_username = username.clone();
    // let this_lobby_code = lobby_code.clone();
    let mut send_messages_to_client = tokio::spawn(async move {
        info!(
            "[CLIENT-SENDER] Sender for user={} is now ready to accept messages.",
            who
        );

        // let user_ip_addr = user_ip.clone();

        while let Ok(game_event_result) = broadcast_channel.recv().await {
            debug!(
                "[CLIENT-SENDER] Got a message from broadcast channel: {:?}",
                game_event_result
            );

            match game_event_result.dest {
                Destination::Lobby(players) => {
                    let _ = sender
                        .send(Message::Text(
                            json!(game_event_result.msg.clone()).to_string(),
                        ))
                        .await;
                }
                Destination::User(player) => {
                    // todo!("User IP is not set for the ai client...");
                    if player.ip.is_some() && player.ip.clone().unwrap() == user_ip {
                        let _ = sender
                            .send(Message::Text(
                                json!(game_event_result.msg.clone()).to_string(),
                            ))
                            .await;
                    }
                    // let _ = sender
                    //     .send(Message::Text(json!(game_event_result.clone()).to_string()))
                    //     .await;
                }
            }

            // let _ = sender
            //     .send(Message::Text(json!(game_event_result.clone()).to_string()))
            //     .await;
        }

        info!("[CLIENT-SENDER] Exiting sender thread for user={}", who);
    });

    info!("Threads are now running...");
    tokio::select! {
        _ = (&mut send_messages_to_client) => recv_messages_from_clients.abort(),
        _ = (&mut recv_messages_from_clients) => send_messages_to_client.abort(),
    };
    // tokio::join!(recv_messages_from_clients);
    info!("We lost the listening thread");
}
