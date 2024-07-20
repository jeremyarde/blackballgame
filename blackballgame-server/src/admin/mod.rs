use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::{IntoResponse},
};
use dioxus::prelude::*;

use crate::websocket::AppState;

#[axum::debug_handler]
pub async fn app_endpoint(
    Path(action): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // let games = state.rooms.lock().await.clone();
    // return Ok(json!({games: games} ));

    let test = state
        .rooms
        .lock()
        .await
        .iter()
        .map(|(roomid, gamestate)| {
            return format!("{} has {} players", roomid, gamestate.players.len());
        })
        .collect::<Vec<String>>();

    let rooms = if test.len() > 0 {
        test.join("\n")
    } else {
        "No rooms".to_string()
    };
    let text = format!("Rooms:\n{}", rooms);

    text
}

#[server]
async fn post_server_data(data: String) -> Result<(), ServerFnError> {
    println!("Server received: {}", data);

    Ok(())
}
