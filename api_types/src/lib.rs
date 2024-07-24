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
    pub lobbies: Vec<String>,
}
