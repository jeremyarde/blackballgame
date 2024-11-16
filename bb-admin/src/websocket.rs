pub mod websocket_connection {
    use api_types::{GetLobbiesResponse, GetLobbyResponse, Lobby};
    use chrono::Utc;
    use common::{
        Card, Connect, Destination, GameAction, GameActionResponse, GameEventResult, GameMessage,
        GameState, GameVisibility, GameplayState, PlayState, PlayerDetails, SetupGameOptions, Suit,
    };
    use dioxus::prelude::*;
    use dioxus_elements::link;
    use dotenvy::dotenv;

    use futures_util::stream::{SplitSink, SplitStream};
    use futures_util::TryStreamExt;
    use futures_util::{SinkExt, StreamExt};
    use manganis::{asset, Asset, ImageAsset, ImageAssetBuilder};
    use reqwest::Client;
    use reqwest_websocket::{websocket, WebSocket};
    use reqwest_websocket::{Message, RequestBuilderExt};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use tracing::{info, Level};

    use crate::ServerConfig;

    pub struct WebsocketConnection {
        pub config: ServerConfig,
        pub client: Client,
        pub listener: Option<SplitStream<WebSocket>>,
        pub sender: Option<SplitSink<WebSocket, Message>>,
        pub connected: bool,
        pub game_id: Option<String>,
        pub player_id: Option<String>,
        pub player_name: Option<String>,
        pub game_state: Option<GameState>,
        pub error: Option<String>,
    }

    impl WebsocketConnection {
        pub fn new(server_config: ServerConfig) -> Self {
            Self {
                config: server_config,
                client: Client::new(),
                listener: None,
                sender: None,
                connected: false,
                game_id: None,
                player_id: None,
                player_name: None,
                game_state: None,
                error: None,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum WsState {
        Pause,
        Resume,
    }

    #[derive(Debug)]
    pub enum WsError {
        ConnectionFailed,
        MessageSendFailed,
        InvalidMessage,
    }

    impl WebsocketConnection {
        pub async fn connect_websocket(&mut self) -> Result<WebSocket, WsError> {
            info!(
                "WebSocketConnection: Attempting to connect to websocket server: {}",
                self.config.server_ws_url
            );

            let response = Client::default()
                .get(self.config.server_ws_url.clone())
                .upgrade() // Prepares the WebSocket upgrade.
                .send()
                .await
                .unwrap();

            // Turns the response into a WebSocket stream.
            let mut websocket = response.into_websocket().await.unwrap();
            info!("WebSocketConnection: Connected to websocket server");
            self.connected = true;
            return Ok(websocket);
        }
    }

    pub async fn send_message(
        ws: &mut WebSocket,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        ws.send(Message::Text(message.to_string())).await?;
        Ok(())
    }
}
