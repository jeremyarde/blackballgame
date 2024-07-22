use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::Method;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use common::GameState;
use dioxus::prelude::server_fn::client::browser;
use futures_util::StreamExt;
use include_dir::Dir;
use include_dir::File;
use mime_guess::mime;
use mime_guess::Mime;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio::sync::Mutex;

use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tracing::info;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use websocket::ws_handler;
use websocket::AppState;

mod admin;
mod websocket;

static FRONTEND_DIR: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/dist");

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("blackballgame=debug".parse().unwrap())
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

    // let corslayer = CorsLayer::new()
    // .allow_methods([Method::POST, Method::GET])
    // .allow_headers([
    //     axum::http::header::CONTENT_TYPE,
    //     axum::http::header::ACCEPT,
    //     // axum::http::header::AUTHORIZATION,
    //     // axum::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
    //     // axum::http::header::ACCESS_CONTROL_REQUEST_METHOD,
    //     // axum::http::HeaderName::from_static("x-auth-token"),
    //     // axum::http::HeaderName::from_static("x-sid"),
    //     axum::http::HeaderName::from_static("session_id"),
    //     // axum::http::HeaderName::from_static("credentials"),
    // ])
    // // .allow_headers(Any)
    // // .allow_credentials(true)
    // .allow_origin(origins)
    // // .allow_origin(Any)
    // .expose_headers([
    //     axum::http::header::CONTENT_ENCODING,
    //     axum::http::HeaderName::from_static("session_id"),
    // ]);

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let assets_dir =
        PathBuf::from("/Users/jarde/Documents/code/blackballgame/blackballgame-client/dist");

    // let serverstate: Arc<AppState> = Arc::new(allgames);
    let serverstate = Arc::new(AppState {
        rooms: Mutex::new(HashMap::new()),
        // players: Mutex::new(HashMap::new()),
        room_broadcast_channel: Mutex::new(HashMap::new()),
        lobby_to_game_channel_send: Mutex::new(HashMap::new()),
        // lobby_to_game_channel_recv: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/rooms", get(get_rooms).post(create_room))
        .route("/health", get(|| async { "ok" }))
        .route(
            "/*path",
            get(|path| async { serve_asset(Some(path)).await }),
        )
        .route(
            "/",
            get(|| async { serve_asset(Some(Path("index.html".to_string()))).await }),
        )
        .layer(cors)
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

#[axum::debug_handler]
pub async fn get_rooms(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rooms = match state.rooms.try_lock() {
        Ok(rooms) => rooms.keys().cloned().collect::<Vec<String>>(),
        Err(_) => vec![String::from("Could not get rooms")],
    };

    rooms.join("\n")
    // rooms
}

#[derive(Deserialize)]
pub struct CreateGameRequest {
    lobby_code: String,
}

#[derive(Serialize)]
pub struct CreateGameResponse {
    lobby_code: String,
}

pub async fn create_room(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateGameRequest>,
) -> impl IntoResponse {
    info!("Creating a new lobby");
    // check if lobby code already exists and don't create a new game

    let mut rooms = state.rooms.lock().await;
    if rooms.contains_key(&request.lobby_code) {
        info!("Room \"{}\" already exists.", request.lobby_code);
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateGameResponse {
                lobby_code: request.lobby_code,
            }),
        );
    }

    let newgame = GameState::new();
    rooms.insert(request.lobby_code.clone(), newgame);

    let broadcast_channel = tokio::sync::broadcast::channel(10).0;
    state
        .room_broadcast_channel
        .lock()
        .await
        .insert(request.lobby_code.clone(), broadcast_channel);

    info!("Success. Created lobby: {}", request.lobby_code);

    (
        StatusCode::CREATED,
        Json(CreateGameResponse {
            lobby_code: request.lobby_code.clone(),
        }),
    )
}
