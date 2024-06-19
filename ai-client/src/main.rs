use std::{
    io::{self, stdin, Read},
    thread::sleep,
    time::Duration,
};

use chrono::{DateTime, Utc};
use common::{Actioner, Connect, GameAction, GameEvent, GameMessage, GameServer};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        accept, connect,
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
};
use tracing::{error, info};
use tracing_subscriber::{fmt::format::FmtSpan, util::SubscriberInitExt, EnvFilter};

struct AI {
    username: String,
    lobby: String,
}

impl AI {
    fn handle_event(&self, username: String, gamestate: GameServer) -> Option<GameMessage> {
        let action = self.decide_action(gamestate);

        if let Some(chosen) = action {
            return Some(GameMessage {
                username: username,
                message: GameEvent {
                    action: chosen,
                    origin: Actioner::Player(self.username.clone()),
                },
                timestamp: Utc::now(),
            });
        }
        return None;
    }

    fn decide_action(&self, gamestate: GameServer) -> Option<GameAction> {
        if gamestate.curr_player_turn.unwrap_or("".to_string()) == self.username {
            return None;
        }

        let action = match gamestate.state {
            common::GameState::Bid => GameAction::Bid(0),
            common::GameState::Pregame => return None,
            common::GameState::Play => {
                let player = gamestate.players.get(&self.username).unwrap();
                GameAction::PlayCard(player.hand.get(0).unwrap().clone())
            }
        };
        Some(action)
    }
}

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

    let ai = AI {
        username: username.clone(),
        lobby: channel.clone(),
    };

    let (mut socket, response) = connect("ws://localhost:3000/ws").expect("Can't connect");
    let _ = socket.send(Message::Pong(vec![1, 2, 3]));
    sleep(Duration::from_secs(2));

    let res = socket.send(Message::Text(
        json!(Connect {
            username: username.clone(),
            channel: channel,
            secret: Some("sky_ai".to_string()) // secret: None
        })
        .to_string(),
    ));

    let mut gamestate: GameServer;

    loop {
        // while()
        sleep(Duration::from_secs(1));
        while let Ok(message) = socket.read() {
            println!("Message recieved: {}", message);

            match serde_json::from_str(message.into_text().unwrap().as_str()) {
                Ok(gamestate) => gamestate,
                Err(err) => {
                    // info!("Error deserializing gamestate: {}", err);
                }
            };
        }

        println!("Waiting for user input...");
        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let mut input_chars = user_input.trim().chars().collect::<Vec<char>>();

        // let mut input_chars = user_input.chars().collect::<Vec<char>>();
        info!("Chars: {:?}", input_chars);

        if input_chars.is_empty() {
            continue;
        }

        match input_chars[0] {
            'b' => {
                info!("Requesting Bid");
                _ = socket.send(Message::Text(
                    json!(GameMessage {
                        username: username.clone(),
                        message: GameEvent {
                            action: GameAction::Bid(input_chars[1].to_digit(10).unwrap() as i32),
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
                //                 input_chars[1].to_digit(10).unwrap() as i32
                //             ),
                //             origin: Actioner::Player(username.clone())
                //         },
                //         timestamp: Utc::now()
                //     })
                //     .to_string(),
                // ));
                info!("'p' not implemented yet")
            }
            's' => {
                info!("Requesting StartGame");
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
            'c' => {
                info!("Requesting CurrentState");
                _ = socket.send(Message::Text(
                    json!(GameMessage {
                        username: username.clone(),
                        message: GameEvent {
                            action: GameAction::CurrentState,
                            origin: Actioner::Player(username.clone())
                        },
                        timestamp: Utc::now()
                    })
                    .to_string(),
                ))
            }
            _ => {}
        }
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

    //     // sleep(Duration::from_secs(10));
    //     sleep(Duration::from_secs(2));
    // }
}
