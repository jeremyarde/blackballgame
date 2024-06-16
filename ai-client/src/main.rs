use std::{
    io::{self, stdin, Read},
    thread::sleep,
    time::Duration,
};

use chrono::{DateTime, Utc};
use common::{Actioner, Connect, GameAction, GameEvent, GameMessage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::{accept, connect, Message};

fn main() {
    let username = "ai".to_string();
    let channel = "a".to_string();

    let (mut socket, response) = connect("ws://127.0.0.1:3000/ws").unwrap();

    let res = socket.send(Message::Text(
        json!(Connect {
            username: username.clone(),
            channel: channel,
            secret: Some("sky_ai".to_string()) // secret: None
        })
        .to_string(),
    ));

    loop {
        let msg = socket.read();
        match msg {
            Ok(x) => {
                println!("Received: {}", x);
            }
            Err(_) => println!("Message could not be read"),
        };

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).unwrap();
        let mut input_chars = user_input.chars();
        match input_chars.nth(0).unwrap() {
            'b' => {
                _ = socket.send(Message::Text(
                    json!(GameMessage {
                        username: username.clone(),
                        message: GameEvent {
                            action: GameAction::Bid(
                                input_chars.nth(1).unwrap().to_digit(10).unwrap() as i32
                            ),
                            origin: Actioner::Player(username.clone())
                        },
                        timestamp: Utc::now()
                    })
                    .to_string(),
                ));
            }

            'p' => {
                // _ = socket.send(Message::Text(
                //     json!(GameMessage {
                //         username: username.clone(),
                //         message: GameEvent {
                //             action: GameAction::PlayCard(
                //                 input_chars.nth(1).unwrap().to_digit(10).unwrap() as i32
                //             ),
                //             origin: Actioner::Player(username.clone())
                //         },
                //         timestamp: Utc::now()
                //     })
                //     .to_string(),
                // ));
                println!("'p' not implemented yet")
            }
            's' => {
                _ = socket.send(Message::Text(
                    json!(GameMessage {
                        username: username.clone(),
                        message: GameEvent {
                            action: GameAction::StartGame,
                            origin: Actioner::Player(username.clone())
                        },
                        timestamp: Utc::now()
                    })
                    .to_string(),
                ));
            }
            _ => {}
        }
        // match msg {
        //     Message::Text(msg) => todo!(),
        //     _ => {}
        // }

        sleep(Duration::from_secs(10));
    }
}
