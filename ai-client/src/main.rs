use std::{
    io::{self, stdin, Read},
    thread::sleep,
    time::Duration,
};

use chrono::{DateTime, Utc};
use common::{Actioner, Connect, GameAction, GameEvent, GameMessage};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{accept, connect, Message},
};
use tracing::{error, info};
use tracing_subscriber::{fmt::format::FmtSpan, util::SubscriberInitExt, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        // .with_env_filter(
        //     EnvFilter::from_default_env().add_directive("ai-client=debug".parse().unwrap()),
        // )
        .with_span_events(FmtSpan::FULL)
        // .with_thread_names(true) // only says "tokio-runtime-worker"
        .with_thread_ids(true)
        .finish()
        .init();

    let username = "ai".to_string();
    let channel = "a".to_string();

    let (mut socket, _) = connect("ws://127.0.0.1:3000/ws").unwrap();

    let res = socket.send(Message::Text(
        json!(Connect {
            username: username.clone(),
            channel: channel,
            secret: Some("sky_ai".to_string()) // secret: None
        })
        .to_string(),
    ));
    while socket.can_read() {
        let msg = socket.read();
        println!("Message: {:?}", msg)
    }

    // loop {
    //     info!("Attempting to read from socket");

    //     let msg = socket.read();
    //     match msg {
    //         Ok(message) => info!("Got message: {}", message),
    //         Err(err) => error!("Got error: {}", err),
    //     }
    //     // match msg {
    //     //     Some(x) => match x {
    //     //         Ok(msg) => info!("Found message: {}", msg),
    //     //         Err(err) => info!("Got error: {}", err),
    //     //     },
    //     //     None => info!("No message available yet"),
    //     // };

    //     info!("waiting on user input...");
    //     let mut user_input = String::new();

    //     info!("Requesting CurrentState");
    //     let _ = socket.send(Message::Text(
    //         json!(GameMessage {
    //             username: username.clone(),
    //             message: GameEvent {
    //                 action: GameAction::CurrentState,
    //                 origin: Actioner::Player(username.clone())
    //             },
    //             timestamp: Utc::now()
    //         })
    //         .to_string(),
    //     ));

    //     // io::stdin().read_line(&mut user_input).unwrap();

    //     // let user_input = user_input.trim();
    //     // if user_input.is_empty() {
    //     //     info!("No input to process");
    //     //     continue;
    //     // }

    //     // let mut input_chars = user_input.chars().collect::<Vec<char>>();
    //     // info!("Chars: {:?}", input_chars);
    //     // match input_chars[0] {
    //     //     'b' => {
    //     //         info!("Requesting Bid");
    //     //         _ = writer.send(Message::Text(
    //     //             json!(GameMessage {
    //     //                 username: username.clone(),
    //     //                 message: GameEvent {
    //     //                     action: GameAction::Bid(input_chars[1].to_digit(10).unwrap() as i32),
    //     //                     origin: Actioner::Player(username.clone())
    //     //                 },
    //     //                 timestamp: Utc::now()
    //     //             })
    //     //             .to_string(),
    //     //         ));
    //     //     }

    //     //     'p' => {
    //     //         // _ = socket.send(Message::Text(
    //     //         //     json!(GameMessage {
    //     //         //         username: username.clone(),
    //     //         //         message: GameEvent {
    //     //         //             action: GameAction::PlayCard(
    //     //         //                 input_chars[1].to_digit(10).unwrap() as i32
    //     //         //             ),
    //     //         //             origin: Actioner::Player(username.clone())
    //     //         //         },
    //     //         //         timestamp: Utc::now()
    //     //         //     })
    //     //         //     .to_string(),
    //     //         // ));
    //     //         info!("'p' not implemented yet")
    //     //     }
    //     //     's' => {
    //     //         info!("Requesting StartGame");
    //     //         _ = writer.send(Message::Text(
    //     //             json!(GameMessage {
    //     //                 username: username.clone(),
    //     //                 message: GameEvent {
    //     //                     action: GameAction::StartGame,
    //     //                     origin: Actioner::Player(username.clone())
    //     //                 },
    //     //                 timestamp: Utc::now()
    //     //             })
    //     //             .to_string(),
    //     //         ));
    //     //     }
    //     //     'c' => {
    //     //         info!("Requesting CurrentState");
    //     //         _ = writer.send(Message::Text(
    //     //             json!(GameMessage {
    //     //                 username: username.clone(),
    //     //                 message: GameEvent {
    //     //                     action: GameAction::CurrentState,
    //     //                     origin: Actioner::Player(username.clone())
    //     //                 },
    //     //                 timestamp: Utc::now()
    //     //             })
    //     //             .to_string(),
    //     //         ))
    //     //     }
    //     //     _ => {}
    //     // }

    //     // sleep(Duration::from_secs(10));
    //     sleep(Duration::from_secs(2));
    // }
}
