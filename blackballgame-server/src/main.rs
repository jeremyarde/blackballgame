#![allow(warnings)]

use core::error;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use api_types::CreateGameRequest;
use api_types::CreateGameResponse;
use api_types::GetLobbiesResponse;
use api_types::GetLobbyResponse;
use axum::body::Body;
use axum::extract::ws::Message;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::Method;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::middleware;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use common::Connect;
use common::GameMessage;
use common::GameState;
use dioxus::prelude::server_fn::client::browser;
use futures_util::StreamExt;
use include_dir::Dir;
use include_dir::File;
use mime_guess::mime;
use mime_guess::Mime;
use nanoid::nanoid_gen;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use tokio::sync::Mutex;

use tokio::sync::RwLock;
use tokio::time::sleep;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tracing::info;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use websocket::ws_handler;
use websocket::AppState;
// use websocket::GameRoomState;

mod admin;
mod game;
mod websocket;

static FRONTEND_DIR: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/dist");

#[derive(Clone, Debug, Deserialize, Serialize)]
enum InternalMessage {
    Game(GameMessage),
    Server(Connect),
    // WsAction(WsAction),
}

const ROOT: &str = "";
const DEFAULT_FILES: [&str; 1] = ["index.html"];
const NOT_FOUND: &str = "404.html";

async fn serve_asset(path: Option<Path<String>>) -> impl IntoResponse {
    info!("Attempting to serve file: {:?}", path);
    let serve_file =
        |file: &File, mime_type: Option<Mime>, cache: Duration, code: Option<StatusCode>| {
            Response::builder()
                .status(code.unwrap_or(StatusCode::OK))
                .header(
                    header::CONTENT_TYPE,
                    mime_type.unwrap_or(mime::TEXT_HTML).to_string(),
                )
                .header(
                    header::CACHE_CONTROL,
                    format!("max-age={}", cache.as_secs_f32()),
                )
                .body(Body::from(file.contents().to_owned()))
                .unwrap()
        };

    let serve_not_found = || match FRONTEND_DIR.get_file(NOT_FOUND) {
        Some(file) => serve_file(file, None, Duration::ZERO, Some(StatusCode::NOT_FOUND)),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File Not Found"))
            .unwrap(),
    };

    let serve_default = |path: &str| {
        info!("Serving default: {}", path);
        for default_file in DEFAULT_FILES.iter() {
            let default_file_path = PathBuf::from(path).join(default_file);

            if FRONTEND_DIR.get_file(default_file_path.clone()).is_some() {
                return serve_file(
                    FRONTEND_DIR.get_file(default_file_path).unwrap(),
                    None,
                    Duration::ZERO,
                    None,
                );
            }
        }

        serve_not_found()
    };

    match path {
        Some(Path(path)) => {
            if path == ROOT {
                return serve_default(&path);
            }

            FRONTEND_DIR.get_file(&path).map_or_else(
                || match FRONTEND_DIR.get_dir(&path) {
                    Some(_) => serve_default(&path),
                    None => serve_not_found(),
                },
                |file| {
                    let mime_type =
                        mime_guess::from_path(PathBuf::from(path.clone())).first_or_octet_stream();
                    let cache = if mime_type == mime::TEXT_HTML {
                        Duration::ZERO
                    } else {
                        Duration::from_secs(60 * 60 * 24)
                    };

                    serve_file(file, Some(mime_type), cache, None)
                },
            )
        }
        None => serve_not_found(),
    }
}

#[axum::debug_handler]
pub async fn get_rooms(
    // State(Arc(AppState { rooms, .. })): State<Arc<Mutex<AppState>>>,
    // State(state): State<GameRoomState>,
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    // let rooms = match state.rooms.try_lock() {
    //     Ok(rooms) => rooms.keys().cloned().collect::<Vec<String>>(),
    //     Err(_) => vec![String::from("Could not get rooms")],
    // };
    (
        StatusCode::OK,
        Json(GetLobbiesResponse {
            lobbies: state
                .read()
                .await
                .rooms
                .keys()
                .cloned()
                .collect::<Vec<String>>(),
        }),
    )
}

#[derive(Debug, strum_macros::AsRefStr, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
enum ServerError {
    InternalServerError,
    BadRequest,
    NotFound(String),
}

#[derive(Debug, strum_macros::AsRefStr, Serialize, Clone)]
enum ClientError {
    LOGIN_FAIL,
    NO_AUTH,
    INVALID_PARAMS,
    SERVICE_ERROR,
    NotFound(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        // println!("->> {:<12} - {self:?}", "INTO_RES");

        let mut response = StatusCode::INTERNAL_SERVER_ERROR.into_response();
        response.extensions_mut().insert(self);
        response
    }
}

impl ServerError {
    pub fn client_status_and_error(&self) -> (StatusCode, ClientError) {
        // #[allow(unreachable_patterns)]
        match self {
            ServerError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ClientError::SERVICE_ERROR,
            ),
            ServerError::BadRequest => (StatusCode::BAD_REQUEST, ClientError::INVALID_PARAMS),
            ServerError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, ClientError::NotFound(msg.clone()))
            }
        }
    }
}

pub async fn main_response_mapper(
    // ctx: Option<SessionContext>,
    uri: Uri,
    req_method: Method,
    res: Response,
) -> Response {
    // println!("->> {:<12} - main_response_mapper", "RES_MAPPER");
    let uuid = nanoid_gen(10);

    // -- Get the eventual response error.
    let service_error = res.extensions().get::<ServerError>();
    let client_status_error = service_error.map(|se| se.client_status_and_error());

    // -- If client error, build the new reponse.
    let error_response = client_status_error
        .as_ref()
        .map(|(status_code, client_error)| {
            let error_message = match client_error {
                ClientError::LOGIN_FAIL => "Login failed",
                ClientError::NO_AUTH => "No auth",
                ClientError::INVALID_PARAMS => "Invalid params",
                ClientError::SERVICE_ERROR => "Service error",
                ClientError::NotFound(msg) => msg,
            };
            let client_error_body = json!({
                "error": {
                    "type": client_error.as_ref(),
                    "req_uuid": uuid.to_string(),
                    "message": error_message,
                }
            });

            info!("client_error_body: {client_error_body}");

            // Build the new response from the client_error_body
            (*status_code, Json(client_error_body)).into_response()
        });

    // Build and log the server log line.
    let client_error = client_status_error.unzip().1;
    // TODO: Need to hander if log_request fail (but should not fail request)
    let _ = log_request(uuid, req_method, uri, service_error, client_error).await;

    println!();
    error_response.unwrap_or(res)
}

pub async fn log_request(
    uuid: String,
    req_method: Method,
    uri: Uri,
    // ctx: Option<SessionContext>,
    service_error: Option<&ServerError>,
    client_error: Option<ClientError>,
) -> anyhow::Result<()> {
    let mut log_line = String::new();
    log_line.push_str(&format!("{uuid} {req_method} {uri}"));
    // if let Some(ctx) = ctx {
    //     log_line.push_str(&format!(" {ctx}"));
    // }
    if let Some(service_error) = service_error {
        log_line.push_str(&format!(" {service_error:?}"));
    }
    if let Some(client_error) = client_error {
        log_line.push_str(&format!(" {client_error:?}"));
    }

    println!("{log_line}");
    Ok(())
}

#[axum::debug_handler]
pub async fn get_room(
    // State(Arc(AppState { rooms, .. })): State<Arc<Mutex<AppState>>>,
    // State(state): State<GameRoomState>,
    State(state): State<Arc<RwLock<AppState>>>,
    Path(room_code): Path<String>,
) -> anyhow::Result<Json<GetLobbyResponse>, ServerError> {
    // let rooms = match state.rooms.try_lock() {
    //     Ok(rooms) => rooms,
    //     Err(_) => return Err(ServerError::InternalServerError),
    // };
    {
        let state = state.read().await;
        let room = match state.rooms.get(&room_code) {
            Some(room) => room,
            None => {
                return Err(ServerError::NotFound(format!(
                    "Room \"{}\" not found",
                    room_code
                )))
            }
        };
        let players = room.players.keys().cloned().collect::<Vec<String>>();
        return Ok(Json(GetLobbyResponse {
            lobby_code: room_code,
            players: players,
        }));
    }

    // rooms
}

pub async fn create_room(
    // State(Arc(Mutex(AppState { rooms, .. }))): State<Arc<Mutex<AppState>>>,
    // State(state): State<GameRoomState>,
    State(state): State<Arc<RwLock<AppState>>>,

    Json(request): Json<CreateGameRequest>,
) -> impl IntoResponse {
    info!("Creating a new lobby");
    // check if lobby code already exists and don't create a new game

    let channel = request.lobby_code.clone();
    {
        match state.try_write() {
            Ok(mut appstate) => {
                if appstate.rooms.contains_key(&channel) {
                    info!("Room \"{}\" already exists.", channel);
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!(CreateGameResponse {
                            lobby_code: channel,
                        })),
                    );
                }

                let newgame = GameState::new();
                appstate.rooms.insert(channel.clone(), newgame);
                info!("Success. Created lobby: {}", request.lobby_code);
                return (
                    StatusCode::CREATED,
                    Json(json!(CreateGameResponse {
                        lobby_code: request.lobby_code.clone(),
                    })),
                );
            }
            Err(err) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!("[DEBUG] Not able to get rooms lock")),
                )
            }
        };
    }
}

#[tokio::main]
async fn main() {
    // console_subscriber::init();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("blackballgame=debug".parse().unwrap())
                .add_directive("blackballgame-server=debug".parse().unwrap())
                // .add_directive("tokio=debug".parse().unwrap())
                .add_directive("axum::rejection=trace".parse().unwrap())
                .add_directive("axum=debug".parse().unwrap())
                // .add_directive("runtime=trace".parse().unwrap())
                .add_directive("common=debug".parse().unwrap()),
        )
        .with_span_events(FmtSpan::FULL)
        // .with_thread_names(true) // only says "tokio-runtime-worker"
        .with_thread_ids(true)
        .finish()
        .init();

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_origin(Any);

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let assets_dir =
        PathBuf::from("/Users/jarde/Documents/code/blackballgame/blackballgame-client/dist");

    // let serverstate: Arc<AppState> = Arc::new(allgames);
    // let serverstate = Arc::new(AppState {
    //     rooms: Mutex::new(HashMap::new()),
    //     room_broadcast_channel: Mutex::new(HashMap::new()),
    //     lobby_to_game_channel_send: Mutex::new(HashMap::new()),
    //     // game_threads: Mutex::new(HashMap::new()),
    // });

    // get to the game thread
    let (gamechannel_send, mut gamechannel_recv) =
        tokio::sync::mpsc::unbounded_channel::<Message>();
    // let (gamechannel_broadcast_send, mut gamechannel_broadcast_recv) =
    //     tokio::sync::broadcast::channel::<Value>(10);
    let (gamechannel_broadcast_send, mut gamechannel_broadcast_recv) =
        tokio::sync::mpsc::unbounded_channel::<Value>();

    let serverstate = Arc::new(RwLock::new(AppState {
        // rooms: HashMap::new(),
        room_broadcast_channel: HashMap::new(),
        lobby_to_game_channel_send: HashMap::new(),
        game_thread_channel: gamechannel_send,
        rooms: HashMap::new(),
    }));

    let mut stateclone = Arc::clone(&serverstate);

    let mut game_to_client_loop = {
        tokio::spawn(async move {
            // let mut messages = Vec::with_capacity(event_cap);

            info!("Game to client - Waiting for messages");
            // gamechannel_broadcast_recv
            //     .recv_many(&mut game_messages, event_cap)
            //     .await;

            while let Ok((msg)) = gamechannel_broadcast_recv.try_recv() {
                info!("Got message: {}", msg);
                // messages.push(msg);
            }
            info!("[GAME-CLIENT] done recieving messages from game");
        })
    };

    let mut game_loop = {
        tokio::spawn(async move {
            let event_cap = 5;
            // let mut rooms: HashMap<String, GameState> = HashMap::new();
            info!("[GAME] - starting thread");
            while let Some(Message::Text(msg)) = gamechannel_recv.recv().await {
                info!("[GAME]: Got message: {}", msg);
            }
            info!("[GAME]: Failed to get message");
            // let mut game_messages = Vec::with_capacity(event_cap);

            // info!("Waiting for messages");
            // gamechannel_recv
            //     .recv_many(&mut game_messages, event_cap)
            //     .await;

            // #[derive(Deserialize, Debug)]
            // struct MyGameMessage {
            //     lobby_code: String,
            //     message: GameMessage,
            // }

            // let mygamemessages = game_messages
            //     .iter()
            //     .map(|val| {
            //         let v = match val {
            //             Message::Text(x) => x,
            //             _ => {}
            //         };
            //         serde_json::from_str::<InternalMessage>(v.as_str()).unwrap()
            //     })
            //     .collect::<Vec<InternalMessage>>();

            // info!("Got messages in game thread: {:?}", mygamemessages);

            // let game_messages: Vec<MyGameMessage> =
            //     match serde_json::from_str::<Vec<MyGameMessage>>(&game_messages) {
            //         Ok(x) => x,
            //         Err(err) => {
            //             info!("Error deserializing GameMessage: {}", err);
            //             continue;
            //         }
            //     };

            // if mygamemessages.is_empty() {
            //     sleep(Duration::from_millis(2000)).await;
            //     continue;
            // }

            // info!("Got messages");
            // info!("Messages: {:?}", mygamemessages);
            // {
            //     for message in mygamemessages {
            //         let rooms = &mut stateclone.write().await.rooms;
            //         let game = rooms.get_mut(&message.lobby_code).unwrap();
            //         let gamestate = game.process_event(vec![message.message]);
            //         let _ = gamechannel_broadcast_send.send(json!(gamestate));
            //     }
            // }

            // sleep(Duration::from_millis(500)).await;
            info!("[GAME]: Exited?");
        })
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/games/ws", get(ws_handler))
        .route("/rooms", get(get_rooms).post(create_room))
        .route("/rooms/:room_code", get(get_room))
        .route("/rooms/:room_code/ws", get(ws_handler))
        .route("/health", get(|| async { "ok" }))
        // .route(
        //     "/*path",
        //     get(|path| async { serve_asset(Some(path)).await }),
        // )
        // .route(
        //     "/",
        //     get(|| async { serve_asset(Some(Path("index.html".to_string()))).await }),
        // )
        .layer(cors)
        // .layer(middleware::map_response(main_response_mapper)) // does not behave nicely
        // .route("/ui".get(ServeDir::new(assets_dir).append_index_html_on_directories(true)))
        // .route("/game", get(Game))
        .with_state(serverstate);

    // run our app with hyper, listening globally on port 3000
    let port = "0.0.0.0:8080";
    info!("Serving application on {}", port);
    let listener = tokio::net::TcpListener::bind(port).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
