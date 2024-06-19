use std::{fmt, io};

use common::{Card, GameClient, PlayerRole, Suit};
use serde::Serialize;
use tracing::info;

struct ServerGameClient {
    client: GameClient,
}

impl ServerGameClient {
    pub fn new(
        id: String,
        // sender: SplitSink<WebSocket, Message>,
        role: PlayerRole,
    ) -> Self {
        // let (tx, rx) = mpsc::unbounded_channel();

        ServerGameClient {
            client: GameClient {
                id,
                // state: PlayerState::Idle,
                hand: vec![],
                order: 0,
                round: 0,
                trump: Suit::Heart,
                role: PlayerRole::Player,
                // rx: rx,
                // tx: tx,
                // sender,
            },
        }
    }

    pub fn clear_hand(&mut self) {
        self.client.hand = vec![];
    }

    pub fn play_card(&mut self, valid_choices: &Vec<Card>) -> (usize, Card) {
        let mut input = String::new();
        info!(
            "Player {}, Select the card you want to play",
            self.client.id
        );

        for (i, card) in self.client.hand.iter().enumerate() {
            info!("{}: {}", i, card);
        }

        info!("Valid cards:");
        for (i, card) in valid_choices.iter().enumerate() {
            info!("{}: {}", i, card);
        }

        // this should probably just grab an event from the queue and check if its the right player
        // io::stdin()
        //     .read_line(&mut input)
        //     .expect("error: unable to read user input");

        let mut parse_result = input.trim().parse::<i32>();
        while parse_result.is_err()
            || !(0..self.client.hand.len()).contains(&(parse_result.clone().unwrap() as usize))
        {
            info!(
                "{:?} is invalid, please enter a valid card position.",
                parse_result
            );
            input = String::new();
            io::stdin()
                .read_line(&mut input)
                .expect("error: unable to read user input");
            parse_result = input.trim().parse::<i32>();
        }
        info!("range: {:?}, selected: {}", (0..self.client.hand.len() - 1), input);

        (
            parse_result.clone().unwrap() as usize,
            self.client.hand[(parse_result.unwrap()) as usize].clone(),
        )
    }

    pub fn get_client_bids(&mut self, allowed_bids: &Vec<i32>) -> i32 {
        info!("Your hand:");
        self.client.hand.iter().for_each(|card| info!("{}", card));

        let mut input = String::new();
        // let mut valid = 0;
        info!("How many tricks do you want?");
        info!("{:#?}", allowed_bids);

        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");

        loop {
            let client_bid = input.trim().parse::<i32>();
            if client_bid.is_err() {
                continue;
            } else {
                let bid = client_bid.unwrap();
                if allowed_bids.contains(&bid) {
                    return bid;
                } else {
                    continue;
                }
            }
        }
    }
}
