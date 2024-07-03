#![allow(non_snake_case)]

use std::collections::HashMap;

use common::GameState;
use dioxus::prelude::*;
use tracing::Level;

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[component]
fn App() -> Element {
    // Build cool things ✌️
    let mut get_games_future = use_resource(|| async move {
        reqwest::get("localhost:8080/admin/games")
            .await
            .unwrap()
            .json::<HashMap<String, GameState>>()
            .await
    });
    rsx! {

        match &*get_games_future.read_unchecked() {
            Some(Ok(response)) => rsx! {
                button { onclick: move |_| get_games_future.restart(), "Click to fetch another doggo" }
                div { "games listed here"}
            },
            Some(Err(_)) => rsx! { div { "Loading dogs failed" } },
            None => rsx! { div { "Loading dogs..." } },
        }
    }
}
