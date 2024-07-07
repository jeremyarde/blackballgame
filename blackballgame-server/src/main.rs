use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use admin::app_endpoint;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::Method;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::delete;
use axum::routing::get;
use axum::Router;
use axum_extra::headers;
use axum_extra::TypedHeader;
use common::GameClient;
use common::GameMessage;
use common::GameState;
use dioxus::prelude::DioxusRouterExt;
use dioxus::prelude::ServeConfig;
use futures_util::SinkExt;
use futures_util::StreamExt;
use include_dir::Dir;
use include_dir::File;
use mime_guess::mime;
use mime_guess::Mime;
use nanoid::nanoid_gen;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio::sync::Mutex;

use tokio::time::sleep;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tracing::info;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use common::PlayerRole;
use websocket::ws_handler;
use websocket::AppState;
use websocket::SharedState;

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
            EnvFilter::from_default_env().add_directive("blackballgame=debug".parse().unwrap()),
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
        .allow_origin(Any);

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
