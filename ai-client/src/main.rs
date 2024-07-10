use std::{
    env, io,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread::{self, sleep},
    time::Duration,
};

use chrono::Utc;
use common::{
    Actioner, Connect, GameAction, GameEvent, GameMessage, GameState, GameplayState,
    SetupGameOptions,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::tungstenite::{connect, Message};
use tracing::info;
use tracing_subscriber::{fmt::format::FmtSpan, util::SubscriberInitExt};

struct AI {
    username: String,
    lobby: String,
    secret_key: String,
}

fn get_bid(gamestate: &GameState) -> GameAction {
    let round_num = gamestate.curr_round;
    let bid_total: i32 = gamestate.bids.values().sum();
    let total_players = gamestate.players.len();
    let bid_order = gamestate.bid_order.clone();

    let mut my_bid = 0;

    if round_num == bid_total {
        my_bid = 1;
    }

    return GameAction::Bid(my_bid);
}

impl AI {
    fn create_action_from_user_input(&self, gamestate: &GameState) -> GameAction {
        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let mut input_chars = user_input.trim().chars().collect::<Vec<char>>();

        // let mut input_chars = user_input.chars().collect::<Vec<char>>();
        info!("Chars: {:?}", input_chars);

        let action = match input_chars[0] {
            'b' => {
                info!("Requesting Bid");
                // _ = socket.send(Message::Text(
                //     json!(GameMessage {
                //         username: username.clone(),
                //         message: GameEvent {
                //             action: GameAction::Bid(input_chars[1].to_digit(10).unwrap() as i32),
                //             origin: Actioner::Player(username.clone())
                //         },
                //         timestamp: Utc::now()
                //     })
                //     .to_string(),
                // ));
                GameAction::Bid(input_chars[1].to_digit(10).unwrap() as i32)
            }
            'p' => {
                info!("Requesting to play a card");
                let cards = GameState::get_hand_from_encrypted(
                    gamestate
                        .players
                        .get(&self.username)
                        .unwrap()
                        .encrypted_hand
                        .clone(),
                    &self.secret_key,
                );

                let cardindex = input_chars[1].to_digit(10).unwrap() as usize;

                if cardindex > cards.len() {
                    info!("Chosen card location is not valid, try again");
                }
                let card = cards.get(cardindex).unwrap();

                info!("Playing card: {}", card);
                GameAction::PlayCard(card.clone())
            }
            's' => {
                info!("Requesting StartGame");
                GameAction::StartGame(SetupGameOptions::new())
            }
            'c' => {
                info!("Requesting CurrentState");
                GameAction::CurrentState
            }
            _ => GameAction::CurrentState,
        };

        return action;
    }

    fn handle_event(&self, username: String, gamestate: GameState) -> Option<GameMessage> {
        let action = self.decide_action(&gamestate);

        if let Some(chosen) = action {
            return Some(GameMessage {
                username: username,
                message: GameEvent {
                    action: chosen,
                    // origin: Actioner::Player(self.username.clone()),
                },
                timestamp: Utc::now(),
            });
        }
        return None;
    }

    fn decide_action(&self, gamestate: &GameState) -> Option<GameAction> {
        let action = match &gamestate.gameplay_state {
            common::GameplayState::Bid => get_bid(gamestate),
            common::GameplayState::Pregame => return None,
            common::GameplayState::Play(ps) => {
                let player = gamestate.players.get(&self.username).unwrap();
                let cards = GameState::get_hand_from_encrypted(
                    gamestate
                        .players
                        .get(&self.username)
                        .unwrap()
                        .encrypted_hand
                        .clone(),
                    &self.secret_key,
                );
                info!("Cards: {:?}", cards);
                GameAction::PlayCard(cards.get(0).unwrap().clone())
            }
            GameplayState::PostRound => GameAction::Deal,
        };
        Some(action)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let secret = &args.get(1);

    println!("Starting using secret {secret:?}");

    tracing_subscriber::fmt()
        // .with_env_filter(
        //     EnvFilter::from_default_env().add_directive("ai-client=debug".parse().unwrap()),
        // )
        .with_span_events(FmtSpan::FULL)
        // .with_thread_names(true) // only says "tokio-runtime-worker"
        .with_thread_ids(true)
        .finish()
        .init();

    let debug_mode = Arc::new(Mutex::new(false));
    let debug_mode_clone = Arc::clone(&debug_mode);
    // let stdin_channel = spawn_stdin_channel();
    // let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();

        match buffer.trim() {
            "d" => {
                info!("Debug ON");
                let mut inner = debug_mode_clone.lock().unwrap();
                *inner = true;
            }
            "off" => {
                info!("Debug OFF");
                let mut inner = debug_mode_clone.lock().unwrap();
                *inner = false;
            }
            _ => {}
        }

        sleep(Duration::from_secs(5));
    });

    let username = "ai".to_string();
    let channel = "a".to_string();

    let mut ai = AI {
        username: username.clone(),
        lobby: channel.clone(),
        secret_key: if secret.is_some() {
            secret.unwrap().clone()
        } else {
            String::new()
        },
    };

    let (mut socket, response) = connect("ws://0.0.0.0:8080/ws").expect("Can't connect");
    let _ = socket.send(Message::Pong(vec![1, 2, 3]));
    sleep(Duration::from_secs(2));

    // let connect_secret =  if secret.is_some() {Some()} else{None};

    let connect_action = if secret.is_some() {
        Connect {
            username: username.clone(),
            channel: channel,
            // secret: Some("sky_ai".to_string()),
            secret: Some(secret.unwrap().clone()),
        }
    } else {
        Connect {
            username: username.clone(),
            channel: channel,
            secret: None,
        }
    };

    let res = socket.send(Message::Text(json!(connect_action).to_string()));

    let mut gamestate: Option<GameState> = None;
    let mut num_error_status_messages = 0;

    loop {
        info!("Sleeping while waiting for messages");
        sleep(Duration::from_secs(1));

        // Read until we exhaust all messages, then try to
        while let Ok(Message::Text(message)) = socket.read() {
            println!("Message recieved: {}", message);

            #[derive(Deserialize)]
            struct ClientSecret {
                client_secret: String,
            }
            match serde_json::from_str::<ClientSecret>(&message) {
                Ok(x) => ai.secret_key = x.client_secret,
                Err(_) => {}
            }

            match serde_json::from_str::<GameState>(&message) {
                Ok(val) => {
                    info!("Setting gamestate");
                    let currplayer = val.curr_player_turn.clone().unwrap_or("".to_string());
                    if currplayer.ne(&connect_action.username) {
                        // update gamestate with new values
                        info!("Not our turn, updated state and waiting.");
                        gamestate = Some(val);
                    } else {
                        info!("Its our turn now, deciding on an action");
                        let mut action = ai.decide_action(&val);

                        // our turn + errors increased = we caused an issue
                        if val.system_status.len() > num_error_status_messages {
                            info!("Error messages increased, setting debug mode ON.");
                            *debug_mode.lock().unwrap() = true;
                        }

                        num_error_status_messages = val.system_status.len();

                        if *debug_mode.lock().unwrap() == true {
                            info!("AI chose an action, send it? (y, n) {:?}", action);
                            let mut user_input = String::new();
                            std::io::stdin().read_line(&mut user_input).unwrap();
                            let inputaction = user_input.trim();

                            if inputaction.eq("n") {
                                action = Some(ai.create_action_from_user_input(&val));
                                // continue;
                            }

                            info!("debug is currently on");
                        } else {
                            info!("debug is currently off");
                        }

                        if let Some(todo) = action {
                            _ = socket.send(Message::Text(
                                json!(GameMessage {
                                    username: username.clone(),
                                    message: GameEvent { action: todo },
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
}
