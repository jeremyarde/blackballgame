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
    Actioner, Connect, GameAction, GameActionResponse, GameMessage, GameState, GameplayState,
    PlayerDetails, SetupGameOptions,
};

use serde::{Deserialize, Serialize};
use serde_json::{error, json};
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
    // let bid_order = gamestate.bid_order.clone();

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
                let cards = GameState::decrypt_player_hand(
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
                timestamp: Utc::now(),
                action: chosen,
                lobby: gamestate.lobby_code.clone(),
            });
        }
        return None;
    }

    fn decide_action(&self, gamestate: &GameState) -> Option<GameAction> {
        let action = match &gamestate.gameplay_state {
            common::GameplayState::Bid => get_bid(gamestate),
            common::GameplayState::Pregame => return None,
            common::GameplayState::PostHand(ps) => return None,
            common::GameplayState::Play(ps) => {
                // let player = gamestate.players.get(&self.username).unwrap();
                let cards = GameState::decrypt_player_hand(
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
            GameplayState::End => GameAction::Ack,
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

    let message = socket.read().unwrap(); // read the ping message
    info!("Server ping: {:?}, responding with pong", message);
    let _ = socket.send(Message::Pong(vec![1, 2, 3]));
    sleep(Duration::from_secs(2));

    // let connect_secret =  if secret.is_some() {Some()} else{None};

    let connect_action = if secret.is_some() {
        GameMessage {
            username: username.clone(),
            action: GameAction::Connect(PlayerDetails {
                username: username.clone(),
                client_secret: Some(secret.unwrap().clone()),
                ip: None,
                lobby: channel.clone(),
            }),
            timestamp: Utc::now(),
            lobby: channel.clone(),
        }
    } else {
        GameMessage {
            username: username.clone(),
            action: GameAction::Connect(PlayerDetails {
                username: username.clone(),
                lobby: channel.clone(),
                ip: None,
                client_secret: None,
            }),
            timestamp: Utc::now(),
            lobby: channel.clone(),
        }
    };

    let res = socket.send(Message::Text(json!(connect_action).to_string()));

    // info!("Connection results: {:?}", res);
    let mut gamestate: Option<GameState> = None;
    let mut num_error_status_messages = 0;

    let connection_success = socket.read().unwrap();
    match connection_success {
        Message::Text(x) => {
            match serde_json::from_str::<Connect>(&x) {
                Ok(connectmessage) => {}
                Err(err) => todo!(),
            };
        }
        // Message::Binary(vec) => todo!(),
        // Message::Ping(vec) => todo!(),
        // Message::Pong(vec) => todo!(),
        // Message::Close(close_frame) => todo!(),
        // Message::Frame(frame) => todo!(),
        _ => {}
    }

    loop {
        // sleep(Duration::from_secs(1));

        info!("Sleeping while waiting for messages");
        // Read until we exhaust all messages, then try to
        while let Ok(msg) = socket.read() {
            // let msg = match message {
            //     Ok(x) => x,
            //     Err(err) => {
            //         tracing::error!("Error reading message: {:?}", err);
            //         break;
            //     }
            // };

            let text = match msg {
                Message::Text(text) => {
                    tracing::info!("Message recieved: {}", text);
                    // serde_json::from_str::<GameMessage>(&text).unwrap()
                    text
                }
                _ => {
                    tracing::error!("Unknown message type: {:?}", msg);
                    break;
                }
            };

            // GameEventResult { dest: User(PlayerDetails { username: "ai", ip: "", client_secret: "sky_669zn8s6ji4q" }), msg: Connect(Connect { username: "ai", channel: "a", secret: Some("sky_669zn8s6ji4q") }) }

            match serde_json::from_str::<GameActionResponse>(&text).unwrap() {
                common::GameActionResponse::Connect(con) => {
                    info!("Got connect message: {con:?}");
                    ai.secret_key = con.secret.unwrap_or(String::new());
                }
                common::GameActionResponse::GameState(gs) => {
                    info!("Got game state: {gs:?}");
                    gamestate = Some(gs);
                }
                common::GameActionResponse::Message(text) => {
                    info!("Got message, not sure what to do with it: {text}");
                }
            }

            let gs = gamestate.clone().unwrap();
            let currplayer = gs.curr_player_turn.clone().unwrap_or("".to_string());
            if currplayer.ne(&connect_action.username) {
                // update gamestate with new values
                info!("Not our turn, updated state and waiting.");
                gamestate = Some(gs);
            } else {
                info!("Its our turn now, deciding on an action");
                let mut action = ai.decide_action(&gs);

                // our turn + errors increased = we caused an issue
                if gs.system_status.len() > num_error_status_messages {
                    info!("Error messages increased, setting debug mode ON.");
                    *debug_mode.lock().unwrap() = true;
                }

                num_error_status_messages = gs.system_status.len();

                if *debug_mode.lock().unwrap() == true {
                    info!("AI chose an action, send it? (y, n) {:?}", action);
                    let mut user_input = String::new();
                    std::io::stdin().read_line(&mut user_input).unwrap();
                    let inputaction = user_input.trim();

                    if inputaction.eq("n") {
                        action = Some(ai.create_action_from_user_input(&gs));
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
                            action: todo,
                            timestamp: Utc::now(),
                            lobby: gs.lobby_code.clone(),
                        })
                        .to_string(),
                    ));
                }
            }
        }

        info!("Message had an error");
        break;
    }

    info!("Exiting");
}
