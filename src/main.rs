use std::collections::HashMap;
use std::fmt;
use std::io;
use std::ops::Rem;

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
    dealing_order: Vec<i32>,
    play_order: Vec<i32>,
    dealer: i32,
    bids: HashMap<i32, i32>,
    wins: HashMap<i32, i32>,
    score: HashMap<i32, i32>,
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

    fn play_card(&mut self) -> (usize, Card) {
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

        return (
            parse_result.clone().unwrap() as usize,
            self.hand[(parse_result.unwrap()) as usize].clone(),
        );
    }

    fn get_client_bids(&mut self) -> i32 {
        println!("Your hand:");
        self.hand.iter().for_each(|card| println!("{}", card));

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

#[derive(Debug)]
enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
}

fn validate_bid(
    bid: &i32,
    curr_round: i32,
    curr_bids: &HashMap<i32, i32>,
    is_dealer: bool,
) -> Result<i32, BidError> {
    // can bid between 0..=round number
    // dealer can't bid a number that will equal the round number
    if *bid > curr_round {
        return Err(BidError::High);
    }

    if *bid < 0 {
        return Err(BidError::Low);
    }
    let bid_sum = curr_bids.values().sum::<i32>();
    if is_dealer && (bid + bid_sum) == curr_round {
        return Err(BidError::EqualsRound);
    }

    return Ok(bid.clone());
}

#[derive(Debug, Clone, Copy)]
enum PlayedCardError {
    DidNotFollowSuit,
    CantUseTrump,
}

fn is_played_card_valid(
    played_cards: &Vec<Card>,
    hand: &mut Vec<Card>,
    played_card: Card,
    trump: &Suit,
) -> Result<Card, PlayedCardError> {
    // rules for figuring out if you can play a card:
    // 1. must follow suit if available
    // 2. can't play trump to start a round unless that is all the player has

    if played_cards.len() == 0 {
        if played_card.suit == *trump {
            // all cards in hand must be trump
            for c in hand {
                if c.suit != *trump {
                    return Err(PlayedCardError::CantUseTrump);
                }
            }
            return Ok(played_card);
        } else {
            return Ok(played_card);
        }
    }

    let led_suit = played_cards.get(0).unwrap().suit.clone();
    if led_suit != played_card.suit {
        // make sure player does not have that suit
        for c in hand {
            if c.suit == led_suit {
                return Err(PlayedCardError::DidNotFollowSuit);
            }
        }
    }
    return Ok(played_card);
}

impl GameServer {
    fn play_game(&mut self) {
        let num_players = self.players.len() as i32;
        let max_rounds = if 52i32.div_euclid(num_players) > 9 {
            9
        } else {
            52i32.div_euclid(num_players)
        };

        println!("Players: {}\nRounds: {}", num_players, max_rounds);

        for round in 1..max_rounds {
            println!("\n-- Round {} --", round);
            self.deal();
            self.bids();
            self.play_round();

            // end of round
            // 1. figure out who lost, who won
            // 2. empty player hands, shuffle deck
            // 3. redistribute cards based on the round

            println!("Bids won: {:#?}\nBids wanted: {:#?}", self.wins, self.bids);
            for player in &mut self.players {
                if self.wins.get(&player.id) == self.bids.get(&player.id) {
                    println!("debug/ player won what they wanted, adding to score");
                    let bidscore = self.bids.get(&player.id).unwrap() + 10;
                    let curr_score = self.score.get_mut(&player.id).unwrap();
                    *curr_score += bidscore;
                }

                player.clear_hand();
            }
            self.advance_trump();
            self.round += 1;

            println!("Player status: {:#?}", self.player_status());
        }
        // stages of the game
    }

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

    fn bids(&mut self) {
        println!("=== Bidding ===");
        println!("Trump is {}", self.trump);

        for client in &mut self.players {
            let mut bid = client.get_client_bids();

            loop {
                match validate_bid(&bid, self.round, &self.bids, self.dealer == client.id) {
                    Ok(x) => {
                        println!("bid was: {}", x);
                        self.bids.insert(client.id, x);
                        break;
                    }
                    Err(e) => {
                        println!("Error with bid: {:?}", e);
                        bid = client.get_client_bids();
                    }
                }
            }
        }
        println!("Biding over, bids are: {:?}", self.bids);
    }

    fn play_round(&mut self) {
        for handnum in 0..self.round {
            println!("--- Hand #{}/{} ---", handnum, self.round);
            // need to use a few things to see who goes first
            // 1. highest bid (at round start)
            // 2. person who won the trick in last round goes first, then obey existing order

            // ask for input from each client in specific order (first person after dealer)
            let mut played_cards: Vec<Card> = vec![];

            let mut curr_winning_card: Option<Card> = None;

            for x in &self.dealing_order {
                let player = self.players.get_mut(*x as usize).unwrap();

                let (loc, mut card) = player.play_card();
                loop {
                    match is_played_card_valid(
                        &played_cards.clone(),
                        &mut player.hand,
                        card.clone(),
                        &self.trump,
                    ) {
                        Ok(x) => {
                            println!("card is valid");
                            card = x;
                            // remove the card from the players hand
                            player.hand.remove(loc);
                            break;
                        }
                        Err(e) => {
                            println!("card is NOT valid: {:?}", e);
                            (_, card) = player.play_card();
                        }
                    }
                }
                played_cards.push(card.clone());

                // logic for finding the winning card
                if curr_winning_card.is_none() {
                    curr_winning_card = Some(card);
                } else {
                    let curr = curr_winning_card.clone().unwrap();
                    if card.suit == curr.suit && card.value > curr.value {
                        curr_winning_card = Some(card.clone());
                    }
                    if card.suit == self.trump
                        && curr.suit == self.trump
                        && card.clone().value > curr.value
                    {
                        curr_winning_card = Some(card);
                    }
                }

                println!(
                    "Curr winning card: {:?}",
                    curr_winning_card.clone().unwrap()
                );
            }

            println!("End turn, trump={:?}, played cards:", self.trump);
            played_cards.clone().iter().for_each(|c| println!("{}", c));

            let win_card = curr_winning_card.unwrap();
            let winner = win_card.played_by;
            println!("Player {:?} won. Winning card: {}", winner, win_card);

            if let Some(x) = self.wins.get_mut(&winner.unwrap()) {
                *x = *x + 1;
            }
        }
    }

    fn deal(&mut self) {
        println!("=== Dealing ===");
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.round {
            // get random card, give to a player
            for playerid in self.dealing_order.clone() {
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
        // println!("{:?}", self.players);
        println!("Score:\n{:?}", self.score);
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
    let num_players = 2;
    let players: Vec<GameClient> = (0..num_players)
        .into_iter()
        .map(|id| GameClient::new(id))
        .collect();
    let mut deal_play_order: Vec<i32> = players.iter().map(|player| player.id.clone()).collect();
    fastrand::shuffle(&mut deal_play_order);

    let mut server = GameServer {
        players: players.clone(),
        deck: create_deck(),
        round: 1,
        trump: Suit::Heart,
        dealing_order: deal_play_order.clone(),
        play_order: deal_play_order,
        dealer: 1,
        bids: HashMap::new(),
        wins: HashMap::new(),
        score: HashMap::new(),
    };

    players.iter().for_each(|player| {
        server.bids.insert(player.id, 0);
        server.wins.insert(player.id, 0);
        server.score.insert(player.id, 0);
    });

    server.play_game();
}
