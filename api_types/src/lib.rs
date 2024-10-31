use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CreateGameResponse {
    pub lobby_code: String,
}

#[derive(Deserialize)]
pub struct CreateGameRequest {
    pub lobby_code: String,
}
#[derive(Deserialize, Serialize)]
pub struct GetLobbiesResponse {
    pub lobbies: Vec<Lobby>,
}
#[derive(Deserialize, Serialize)]
pub struct GetLobbyResponse {
    pub lobby: Lobby,
}

#[derive(Deserialize, Serialize)]
pub struct Lobby {
    pub name: String,
    pub players: Vec<String>,
    pub max_players: usize,
    pub game_mode: String,
}
