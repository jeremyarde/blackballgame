use std::{io, os::unix::net::SocketAddr};

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use tokio::sync::mpsc;

use crate::{Card, PlayerState, Rx, Suit, Tx};

#[derive(Debug)]
pub struct GameClient {
    pub id: String,
    pub hand: Vec<Card>,
    pub order: i32,
    pub trump: Suit,
    pub round: i32,
    pub state: PlayerState,
    // don't need if we are using JS
    // pub rx: Rx,
    // pub tx: Tx,
}

impl GameClient {
    pub fn new(id: String) -> Self {
        // let (tx, rx) = mpsc::unbounded_channel();

        return GameClient {
            id,
            state: PlayerState::Idle,
            hand: vec![],
            order: 0,
            round: 0,
            trump: Suit::Heart,
            // rx: rx,
            // tx: tx,
        };
    }

    pub fn clear_hand(&mut self) {
        self.hand = vec![];
    }

    pub fn play_card(&mut self, valid_choices: &Vec<Card>) -> (usize, Card) {
        let mut input = String::new();
        println!("Player {}, Select the card you want to play", self.id);

        for (i, card) in self.hand.iter().enumerate() {
            println!("{}: {}", i, card);
        }

        println!("Valid cards:");
        for (i, card) in valid_choices.iter().enumerate() {
            println!("{}: {}", i, card);
        }

        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");

        let mut parse_result = input.trim().parse::<i32>();
        while parse_result.is_err()
            || !(0..self.hand.len()).contains(&(parse_result.clone().unwrap() as usize))
        {
            println!(
                "{:?} is invalid, please enter a valid card position.",
                parse_result
            );
            input = String::new();
            io::stdin()
                .read_line(&mut input)
                .expect("error: unable to read user input");
            parse_result = input.trim().parse::<i32>();
        }
        println!("range: {:?}, selected: {}", (0..self.hand.len() - 1), input);

        return (
            parse_result.clone().unwrap() as usize,
            self.hand[(parse_result.unwrap()) as usize].clone(),
        );
    }

    pub fn get_client_bids(&mut self, allowed_bids: &Vec<i32>) -> i32 {
        println!("Your hand:");
        self.hand.iter().for_each(|card| println!("{}", card));

        let mut input = String::new();
        // let mut valid = 0;
        println!("How many tricks do you want?");
        println!("{:#?}", allowed_bids);

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
