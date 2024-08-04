// use std::{sync::Arc, time::Duration};

// use axum::extract::State;
// // use chrono::Duration;

// use common::{GameMessage, GameState};
// use tokio::{
//     sync::{mpsc::Sender, Mutex},
//     task::JoinHandle,
//     time::sleep,
// };
// use tracing::info;

// use crate::websocket::AppState;

// pub async fn start_game_thread(
//     // State(AppState { rooms, .. }): State<Arc<Mutex<AppState>>>,
//     state: State<Arc<Mutex<AppState>>>,
//     lobby_code: String,
//     mut recv: tokio::sync::mpsc::Receiver<GameMessage>,
//     // broadcast: Sender<GameState>,
// ) -> JoinHandle<()> {
//     // let (snd, recv) = state
//     //     .lobby_to_game_channel_send
//     //     .lock()
//     //     .await
//     //     .get(&lobby_code)
//     //     .unwrap();
//     let mut game_loop = {
//         tokio::spawn(async move {
//             let event_cap = 5;
//             info!("Starting up game");
//             loop {
//                 let mut game_messages = Vec::with_capacity(event_cap);

//                 info!("Waiting for messages");
//                 recv.recv_many(&mut game_messages, event_cap).await;

//                 if game_messages.is_empty() {
//                     sleep(Duration::from_millis(2000)).await;
//                     continue;
//                 }

//                 info!("Got messages");
//                 info!("Messages: {:?}", game_messages);
//                 {
//                     let mut rooms = &mut state.lock().await.rooms;
//                     let game = rooms.get_mut(&lobby_code).unwrap();

//                     let gamestate = game.process_event(game_messages);

//                     let _ = broadcast.send(gamestate);
//                 }

//                 sleep(Duration::from_millis(500)).await;
//             }
//         })
//     };

//     game_loop
// }
