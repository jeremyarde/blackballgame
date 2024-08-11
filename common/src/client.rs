use std::io;

use tracing::info;

use crate::{Card, GameClient, PlayerDetails, PlayerRole};

impl GameClient {
    pub fn new(id: String, role: PlayerRole, user_ip: String) -> Self {
        GameClient {
            id: id.clone(),
            hand: vec![],
            role,
            // encrypted_hand: vec![],
            encrypted_hand: String::new(),
            num_cards: 0,
            details: PlayerDetails {
                username: id.clone(),
                ip: user_ip,
            },
        }
    }

    pub fn clear_hand(&mut self) {
        self.hand = vec![];
    }

    pub fn play_card(&mut self, valid_choices: &Vec<Card>) -> (usize, Card) {
        let mut input = String::new();
        info!("Player {}, Select the card you want to play", self.id);

        for (i, card) in self.hand.iter().enumerate() {
            info!("{}: {}", i, card);
        }

        info!("Valid cards:");
        for (i, card) in valid_choices.iter().enumerate() {
            info!("{}: {}", i, card);
        }

        let mut parse_result = input.trim().parse::<i32>();
        while parse_result.is_err()
            || !(0..self.hand.len()).contains(&(parse_result.clone().unwrap() as usize))
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
        info!("range: {:?}, selected: {}", (0..self.hand.len() - 1), input);

        (
            parse_result.clone().unwrap() as usize,
            self.hand[(parse_result.unwrap()) as usize].clone(),
        )
    }

    pub fn get_client_bids(&mut self, allowed_bids: &Vec<i32>) -> i32 {
        info!("Your hand:");
        self.hand.iter().for_each(|card| info!("{}", card));

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
