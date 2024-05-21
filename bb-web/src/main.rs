#![allow(non_snake_case)]
#![feature(unboxed_closures)]

use std::fmt::{self, Display};

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{info, Level};

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[component]
fn App() -> Element {
    // let ws = use_signal(|| )
    let ws = use_coroutine(|rx| async move {
        let websocket = Client::default()
            .get("wss://echo.websocket.org/")
            .upgrade()
            .send()
            .await
            .unwrap()
            .into_websocket()
            .await
            .unwrap();

        let (mut tx, mut rx) = websocket.split();
    });

    let mut bid = use_signal(|| 0);
    let mut bid_error = use_signal(|| "".to_string());
    let mut played_cards: Signal<Vec<Card>> = use_signal(|| vec![]);
    let mut hand: Signal<Vec<Card>> = use_signal(|| {
        vec![
            Card {
                id: 1,
                suit: Suit::Heart,
                value: 1,
                played_by: Some("me".to_string()),
            },
            Card {
                id: 2,
                suit: Suit::Diamond,
                value: 1,
                played_by: Some("me".to_string()),
            },
            Card {
                id: 3,
                suit: Suit::Club,
                value: 1,
                played_by: Some("me".to_string()),
            },
            Card {
                id: 4,
                suit: Suit::Spade,
                value: 1,
                played_by: Some("me".to_string()),
            },
        ]
    });

    rsx! {
        link { rel: "stylesheet", href: "main.css" }
        div { class: "bg-red-100 w-screen h-screen grid grid-rows-3",
            h2 { "jeremy was here" }
            div { class: "bg-blue-200 ",
                "Play area"
                div { class: "size-8 flex flex-row justify-center w-full h-full",
                    {played_cards.read().iter().enumerate().map(|(i, card)| {
                        rsx!{CardComponent {card: card.clone(), handle_click: move |val| {hand.push(val);
                                played_cards.remove(i);
                            }}}
                        })}
                }
            }
            div { class: "flex w-full h-full flex-row justify-between",
                "Hand, controls"
                div { class: "size-8 w-full h-full flex flex-row justify-center",
                    {hand.read().iter().enumerate().map(|(i, card)| {
                        rsx!{CardComponent {card: card.clone(), handle_click: move |val| {played_cards.push(val);
                            hand.remove(i);
                        }}}
                    })}
                }
                div {
                    input {
                        // we tell the component what to render
                        value: "{bid}",
                        r#type: "number",
                        // and what to do when the value changes
                        oninput: move |event| {
                            if event.value().parse::<i32>().is_err() {}
                            bid.set(event.value().parse::<i32>().unwrap());
                        }
                    }
                    button {
                        class: "bg-blue-300 border border-solid w-full h-full hover:bg-green",
                        onclick: move |event| info!("sending bid"),
                        "Send bid"
                    }
                    label { "{bid_error}" }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
pub enum Suit {
    Heart,
    Diamond,
    Club,
    Spade,
    NoTrump,
}
impl fmt::Display for Suit {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        let suit = match self {
            Suit::Heart => "H",
            Suit::Diamond => "D",
            Suit::Club => "C",
            Suit::Spade => "S",
            Suit::NoTrump => "N",
        };

        write!(f, "{suit}",)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    id: usize,
    suit: Suit,
    value: i32,
    played_by: Option<String>,
}

#[derive(Clone, Props)]
struct CardProps<F: 'static + Clone + FnMut(Card)> {
    pub card: Card,
    // #[props(!optional)]
    pub handle_click: F,
    // pub play_card: Fn,
}

impl<F: 'static + Clone + FnMut(Card)> PartialEq for CardProps<F> {
    fn eq(&self, other: &Self) -> bool {
        self.card == other.card
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

#[component]
fn CardComponent<F: Clone + FnMut(Card)>(mut props: CardProps<F>) -> Element {
    rsx!(
        div {
            class: "size-20 border border-solid bg-white hover:bg-green-200",
            onclick: move |evt| {
                (props.handle_click)(props.card.clone());
            },
            "{props.card.suit} {props.card.value}"
        }
    )
}
