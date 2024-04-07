use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq)]
enum Suit {
    Heart,
    Diamond,
    Club,
    Spade,
    NoTrump,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suit = match self {
            &Self::Heart => "H",
            &Self::Diamond => "D",
            &Self::Club => "C",
            &Self::Spade => "S",
            &Self::NoTrump => "None",
        };
        write!(f, "{}", suit)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct Card {
    id: usize,
    suit: Suit,
    value: i32,
    played_by: Option<i32>,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} {}]", self.value, self.suit)
    }
}

// impl PartialOrd for Card {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         // match self.id.partial_cmp(&other.id) {
//         //Some(core::cmp::Ordering::Equal) => {}
//         //ord => return ord,
//         // }
//         match self.suit.partial_cmp(&other.suit) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         match self.value.partial_cmp(&other.value) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         // self.played_by.partial_cmp(&other.played_by)
//     }
// }

#[derive(Debug, Clone)]
struct GameServer {
    players: Vec<GameClient>,
    deck: Vec<Card>,
    round: i32,
    trump: Suit,
    player_order: Vec<i32>,
    dealer: i32,
    bids: HashMap<i32, i32>,
    wins: HashMap<i32, i32>,
}

#[derive(Debug, Clone)]
struct GameClient {
    id: i32,
    hand: Vec<Card>,
    order: i32,
    trump: Suit,
    round: i32,
    state: PlayerState,
}

#[derive(Debug, Clone, Copy)]
enum PlayerState {
    Idle,
    RequireInput,
}

use std::io::{self, Read};

impl GameClient {
    fn new(id: i32) -> Self {
        return GameClient {
            id,
            state: PlayerState::Idle,
            hand: vec![],
            order: 0,
            round: 0,
            trump: Suit::Heart,
        };
    }

    fn clear_hand(&mut self) {
        self.hand = vec![];
    }

    fn play_card(&mut self) -> Card {
        let mut input = String::new();
        println!("Player {}, Select the card you want to play", self.id);

        for (i, card) in self.hand.iter().enumerate() {
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

            println!("debug/ {:?}, {:?}", input, parse_result);
        }
        println!("range: {:?}, selected: {}", (0..self.hand.len() - 1), input);

        return self.hand[(parse_result.unwrap()) as usize].clone();
    }

    fn get_client_bids(&mut self) -> i32 {
        let mut input = String::new();
        let mut valid = 0;
        println!("How many tricks do you want?");

        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");

        loop {
            let client_bid = input.trim().parse::<i32>();
            if client_bid.is_err() {
                continue;
            } else {
                return client_bid.unwrap();
            }
        }
    }
}

enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
}

fn validate_bid(bid: &i32, curr_round: i32, curr_bids: HashMap<i32, i32>) -> Result<i32, BidError> {
    // can bid between 0..=round number
    // dealer can't bid a number that will equal the round number
    if *bid > curr_round {
        return Err(BidError::High);
    }

    if *bid < 0 {
        BidError::Low;
    }

    if (bid + curr_bids.into_iter().sum::<i32>()) == curr_round {
        return Err(BidError::EqualsRound);
    }

    return Ok(bid);
}

impl GameServer {
    fn get_random_card(&mut self) -> Option<Card> {
        fastrand::shuffle(&mut self.deck);
        return self.deck.pop();
    }

    fn advance_trump(&mut self) {
        match self.trump {
            Suit::Heart => self.trump = Suit::Diamond,
            Suit::Diamond => self.trump = Suit::Club,
            Suit::Club => self.trump = Suit::Spade,
            Suit::Spade => self.trump = Suit::NoTrump,
            Suit::NoTrump => self.trump = Suit::Heart,
        }
    }

    fn bids(&self) {
        for client in &self.players {
            let bid = &client.get_client_bids();

            loop {
                match validate_bid(&bid, self.round, self.bids) {
                    Ok(x) => {
                        println!("bid was: {}", x);
                    }
                    Err(_) => {
                        println!("Error with bid.")
                    }
                }

            }
        }
    }

    fn play_round(&mut self) {
        for handnum in 0..self.round {
            // need to use a few things to see who goes first
            // 1. highest bid (at round start)
            // 2. person who won the trick in last round goes first, then obey existing order

            // ask for input from each client in specific order (first person after dealer)
            let mut played_cards: Vec<Card> = vec![];

            let mut curr_winning_card: Option<Card> = None;

            for x in &self.player_order {
                let player = self.players.get_mut(*x as usize).unwrap();
                let card = player.play_card();
                played_cards.push(card.clone());

                if curr_winning_card.is_none() {
                    curr_winning_card = Some(card);
                } else {
                    let curr = curr_winning_card.clone().unwrap();
                    if card.suit == curr.suit && card.value > curr.value {
                        curr_winning_card = Some(card.clone());
                    }
                    if card.suit == self.trump && curr.suit == self.trump && card.value > curr.value
                    {
                        curr_winning_card = Some(card);
                    }
                }

                println!("Curr winning card: {:?}", curr_winning_card);
            }

            println!(
                "End turn, trump={:?}, played cards={:#?}",
                self.trump, played_cards
            );
            let winning_player_id = curr_winning_card.clone().unwrap().played_by;
        }

        // end of round
        // 1. figure out who lost, who won
        // 2. empty player hands, shuffle deck
        // 3. redistribute cards based on the round
        for player in &mut self.players {
            player.clear_hand();
        }
        self.advance_trump();
        self.round += 1;

        // match curr_winning_card {
        //     Some(x) => x.played_by,
        //     None => println!("Error finding winning card. This is bad"),
        // }

        // let winning_card = played_cards.sort_by(|card| card);
    }

    fn deal(&mut self) {
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.round {
            // get random card, give to a player
            for playerid in self.player_order.clone() {
                let card = self.get_random_card().unwrap();
                let mut player: &mut GameClient = self.players.get_mut(playerid as usize).unwrap();
                // .get(playerid).unwrap();

                let mut new_card = card.clone();
                new_card.played_by = Some(player.id.clone());
                player.hand.push(new_card);
            }
        }
    }

    fn player_status(&self) {
        println!("{:?}", self.players);
    }
}

fn create_deck() -> Vec<Card> {
    let mut cards = vec![];

    // 14 = Ace
    let mut cardid = 0;
    for value in 2..=14 {
        cards.push(Card {
            id: cardid,
            suit: Suit::Heart,
            value: value,
            played_by: None,
        });
        cards.push(Card {
            id: cardid + 1,
            suit: Suit::Diamond,
            value: value,
            played_by: None,
        });
        cards.push(Card {
            id: cardid + 2,
            suit: Suit::Club,
            played_by: None,

            value: value,
        });
        cards.push(Card {
            id: cardid + 3,
            suit: Suit::Spade,
            value: value,
            played_by: None,
        });
        cardid += 4;
    }

    return cards;
}

fn main() {
    let players: Vec<GameClient> = (0..3).into_iter().map(|id| GameClient::new(id)).collect();
    let dealerid = fastrand::usize(..&players.len()) as i32;

    let mut server = GameServer {
        players: players.clone(),
        deck: create_deck(),
        round: 2,
        trump: Suit::Heart,
        player_order: vec![0, 1],
        dealer: dealerid,
        bids: HashMap::new(),
        wins: HashMap::new(),
    };

    players.iter().for_each(|player| {
        server.bids.insert(player.id, 0);
        server.wins.insert(player.id, 0);
    });

    // stages of the game
    server.deal();
    server.bids();
    server.play_round();
    // server.update_scores();

    println!("Player status: {:#?}", server.player_status());
}
