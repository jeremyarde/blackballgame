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
        let action = self.decide_action(&gamestate);

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

    fn decide_action(&self, gamestate: &GameServer) -> Option<GameAction> {
        // if gamestate
        //     .curr_player_turn
        //     .clone()
        //     .unwrap_or("".to_string())
        //     .eq(&self.username)
        // {
        //     return None;
        // }

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

    let connect_action = Connect {
        username: username.clone(),
        channel: channel,
        secret: Some("sky_ai".to_string()), // secret: None
    };

    let res = socket.send(Message::Text(json!(connect_action).to_string()));

    let mut gamestate: Option<GameServer> = None;

    loop {
        // while()
        sleep(Duration::from_secs(1));

        // Read until we exhaust all messages, then try to
        while let Ok(Message::Text(message)) = socket.read() {
            println!("Message recieved: {}", message);

            match serde_json::from_str::<GameServer>(&message) {
                Ok(val) => {
                    info!("Setting gamestate");
                    // gamestate = Some(val.clone());
                    let currplayer = val.curr_player_turn.clone().unwrap_or("".to_string());
                    if currplayer.ne(&connect_action.username) {
                        // update gamestate with new values
                        info!("Not our turn, updated state and waiting.");
                        gamestate = Some(val);
                    } else {
                        info!("Its our turn now, deciding on an action");
                        let action = ai.decide_action(&val);
                        info!("AI chose an action, send it? {:?}", action);

                        let mut user_input = String::new();
                        std::io::stdin().read_line(&mut user_input).unwrap();
                        let inputaction = user_input.trim();

                        if inputaction.eq("n") {
                            continue;
                        }

                        if let Some(todo) = action {
                            _ = socket.send(Message::Text(
                                json!(GameMessage {
                                    username: username.clone(),
                                    message: GameEvent {
                                        action: todo,
                                        origin: Actioner::Player(username.clone())
                                    },
                                    timestamp: Utc::now()
                                })
                                .to_string(),
                            ));
                        }
                    }
                }
                Err(err) => {
                    info!("Message was not game state: {}", err);
                }
            };
        }
    }

    // if let Some(ref game) = gamestate {
    //     info!("After socket read, AI making a move...");
    //     let action: Option<GameAction> = ai.decide_action(&game);

    //     info!("AI chose an action, send it? {:?}", action);

    //     let mut user_input = String::new();
    //     std::io::stdin().read_line(&mut user_input).unwrap();
    //     let inputaction = user_input.trim();

    //     if inputaction.eq("n") {
    //         continue;
    //     }

    //     if let Some(todo) = action {
    //         _ = socket.send(Message::Text(
    //             json!(GameMessage {
    //                 username: username.clone(),
    //                 message: GameEvent {
    //                     action: todo,
    //                     origin: Actioner::Player(username.clone())
    //                 },
    //                 timestamp: Utc::now()
    //             })
    //             .to_string(),
    //         ));
    //     }
    // }

    //     println!("Waiting for user input...");
    //     let mut user_input = String::new();
    //     std::io::stdin().read_line(&mut user_input).unwrap();
    //     let mut input_chars = user_input.trim().chars().collect::<Vec<char>>();

    //     // let mut input_chars = user_input.chars().collect::<Vec<char>>();
    //     info!("Chars: {:?}", input_chars);

    //     if input_chars.is_empty() {
    //         continue;
    //     }

    //     match input_chars[0] {
    //         'b' => {
    //             info!("Requesting Bid");
    //             _ = socket.send(Message::Text(
    //                 json!(GameMessage {
    //                     username: username.clone(),
    //                     message: GameEvent {
    //                         action: GameAction::Bid(input_chars[1].to_digit(10).unwrap() as i32),
    //                         origin: Actioner::Player(username.clone())
    //                     },
    //                     timestamp: Utc::now()
    //                 })
    //                 .to_string(),
    //             ));
    //         }

    //         'p' => {
    //             // _ = socket.send(Message::Text(
    //             //     json!(GameMessage {
    //             //         username: username.clone(),
    //             //         message: GameEvent {
    //             //             action: GameAction::PlayCard(
    //             //                 input_chars[1].to_digit(10).unwrap() as i32
    //             //             ),
    //             //             origin: Actioner::Player(username.clone())
    //             //         },
    //             //         timestamp: Utc::now()
    //             //     })
    //             //     .to_string(),
    //             // ));
    //             info!("'p' not implemented yet")
    //         }
    //         's' => {
    //             info!("Requesting StartGame");
    //             _ = socket.send(Message::Text(
    //                 json!(GameMessage {
    //                     username: username.clone(),
    //                     message: GameEvent {
    //                         action: GameAction::StartGame,
    //                         origin: Actioner::Player(username.clone())
    //                     },
    //                     timestamp: Utc::now()
    //                 })
    //                 .to_string(),
    //             ));
    //         }
    //         'c' => {
    //             info!("Requesting CurrentState");
    //             _ = socket.send(Message::Text(
    //                 json!(GameMessage {
    //                     username: username.clone(),
    //                     message: GameEvent {
    //                         action: GameAction::CurrentState,
    //                         origin: Actioner::Player(username.clone())
    //                     },
    //                     timestamp: Utc::now()
    //                 })
    //                 .to_string(),
    //             ))
    //         }
    //         _ => {}
    //     }
    // }
}
