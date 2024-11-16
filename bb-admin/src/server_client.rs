pub mod server_client {
    use api_types::GetLobbiesResponse;
    use reqwest::Client;
    use tracing::info;

    pub struct ServerClient {
        client: Client,
        server_url: String,
    }

    impl ServerClient {
        pub fn new(server_url: String) -> Self {
            Self {
                client: Client::new(),
                server_url,
            }
        }

        pub async fn get_rooms(&self) -> Result<GetLobbiesResponse, reqwest::Error> {
            info!("[SERVER-CLIENT] Getting rooms");
            let resp = reqwest::Client::new()
                .get(format!("{}{}", self.server_url, "/rooms"))
                .send()
                .await;

            match resp {
                Ok(data) => {
                    return Ok(data
                        .json::<GetLobbiesResponse>()
                        .await
                        .expect("Failed to parse lobbies"))
                }
                Err(err) => {
                    // log::info!("Request failed with error: {err:?}")
                    return Ok(GetLobbiesResponse { lobbies: vec![] });
                }
            }
        }
    }
}
