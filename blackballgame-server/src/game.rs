use std::{collections::HashMap, fmt};

use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::info;

use crate::{
    client::{GameClient, PlayerRole},
    GameMessage,
};

#[derive(Serialize, Clone, Debug)]
pub struct FullGameState {
    trump: Suit,
    state: GameState,
    cards: Vec<Card>,
}

impl GameServer {
    pub fn process_event(&mut self, events: Vec<GameMessage>) -> Option<FullGameState> {
        info!("[TODO] Processing an event");
        // return None;
        for event in events.iter() {
            match &event.message.action {
                GameAction::PlayCard(card) => todo!(),
                GameAction::Bid(bid) => {
                    if self.state != GameState::Deal {
                        return None;
                    }

                    self.update_bid(event.username.clone(), &bid);
                }
                GameAction::Deal => self.deal(),
                GameAction::StartGame => {
                    self.state = GameState::Deal;
                }
                GameAction::GetHand => todo!(),
            }
            info!("processing: {:?}", event);
        }
        return Some(self.get_state());
    }

    pub fn new() -> Self {
        // let (tx, rx) = broadcast::channel(10);

        let mut server = GameServer {
            players: HashMap::new(),
            deck: create_deck(),
            curr_round: 1,
            trump: Suit::Heart,
            dealing_order: vec![],
            play_order: vec![],
            // dealer_id: deal_play_order[0],
            bids: HashMap::new(),
            wins: HashMap::new(),
            score: HashMap::new(),
            state: GameState::Pregame,

            // send and recieve here
            tx: broadcast::channel(10).0,
            event_log: vec![],
            event_queue: vec![],
            // tx,
            // rx,
        };
        server
    }

    pub fn get_state(&self) -> FullGameState {
        return FullGameState {
            trump: self.trump.clone(),
            state: self.state.clone(),
            cards: vec![],
        };
    }

    fn add_player(
        &mut self,
        player_id: String,
        rx: SplitStream<WebSocket>,
        sender: SplitSink<WebSocket, Message>,
        role: PlayerRole,
    ) {
        self.players
            .insert(player_id.clone(), GameClient::new(player_id, role));
    }

    pub fn setup_game(&mut self, max_rounds: Option<i32>) {
        // let num_players = 3;
        // let max_rounds = Some(3);
        let mut deal_play_order: Vec<String> =
            self.players.iter().map(|(id, player)| id.clone()).collect();
        fastrand::shuffle(&mut deal_play_order);

        let mut play_order = deal_play_order.clone();
        let first = play_order.remove(0);
        play_order.push(first);

        self.players.iter().for_each(|(id, player)| {
            self.bids.insert(id.clone(), 0);
            self.wins.insert(id.clone(), 0);
            self.score.insert(id.clone(), 0);
        });

        let num_players = self.players.len() as i32;

        let max_rounds = if max_rounds.is_some() {
            max_rounds.unwrap()
        } else if 52i32.div_euclid(num_players) > 9 {
            9
        } else {
            52i32.div_euclid(num_players)
        };

        tracing::info!("Players: {}\nRounds: {}", num_players, max_rounds);

        for round in 1..=max_rounds {
            tracing::info!("\n-- Round {} --", round);

            tracing::info!("\t/debug: deal order: {:#?}", self.dealing_order);
            tracing::info!("\t/debug: play order: {:#?}", self.play_order);

            self.deal();
            // self.bids();
            // self.play_round();
            for handnum in 1..=self.curr_round {
                tracing::info!(
                    "--- Hand #{}/{} - Trump: {}---",
                    handnum,
                    self.curr_round,
                    self.trump
                );
                // need to use a few things to see who goes first
                // 1. highest bid (at round start)
                // 2. person who won the trick in last round goes first, then obey existing order

                // ask for input from each client in specific order (first person after dealer)
                let mut played_cards: Vec<Card> = vec![];

                let mut curr_winning_card: Option<Card> = None;

                for player_id in self.play_order.iter() {
                    let player = self.players.get_mut(player_id).unwrap();

                    let valid_cards_to_play = player
                        .hand
                        .iter()
                        .filter_map(|card| {
                            match is_played_card_valid(
                                &played_cards,
                                &player.hand,
                                card,
                                &self.trump,
                            ) {
                                Ok(x) => Some(x),
                                Err(err) => None,
                            }
                        })
                        .collect::<Vec<Card>>();

                    let (loc, mut card) = player.play_card(&valid_cards_to_play);
                    loop {
                        match is_played_card_valid(
                            &played_cards.clone(),
                            &mut player.hand,
                            &card.clone(),
                            &self.trump,
                        ) {
                            Ok(x) => {
                                tracing::info!("card is valid");
                                card = x;
                                // remove the card from the players hand
                                player.hand.remove(loc);
                                break;
                            }
                            Err(e) => {
                                tracing::info!("card is NOT valid: {:?}", e);
                                (_, card) = player.play_card(&valid_cards_to_play);
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

                    tracing::info!(
                        "Curr winning card: {:?}",
                        curr_winning_card.clone().unwrap()
                    );
                }

                tracing::info!("End turn, trump={:?}, played cards:", self.trump);
                played_cards
                    .clone()
                    .iter()
                    .for_each(|c| tracing::info!("{}", c));

                let win_card = curr_winning_card.unwrap();
                tracing::info!(
                    "Player {:?} won. Winning card: {:?}",
                    win_card.played_by,
                    win_card
                );
                let winner = win_card.played_by;
                if let Some(x) = self.wins.get_mut(&winner.unwrap()) {
                    *x = *x + 1;
                }
            }

            // end of round
            // 1. figure out who lost, who won
            // 2. empty player hands, shuffle deck
            // 3. redistribute cards based on the round

            tracing::info!("Bids won: {:#?}\nBids wanted: {:#?}", self.wins, self.bids);
            for player_id in self.play_order.iter() {
                let player = self.players.get_mut(player_id).unwrap();

                if self.wins.get(&player.id) == self.bids.get(&player.id) {
                    let bidscore = self.bids.get(&player.id).unwrap() + 10;
                    let curr_score = self.score.get_mut(&player.id).unwrap();
                    *curr_score += bidscore;
                }

                // resetting the data structures for a round before round start
                self.wins.insert(player.id.clone(), 0);
                self.bids.insert(player.id.clone(), 0);
                player.clear_hand();
            }
            // self.clear_previous_round();
            self.advance_trump();
            self.curr_round += 1;
            let curr_dealer = self.dealing_order.remove(0);
            self.dealing_order.push(curr_dealer);

            let first_player = self.play_order.remove(0);
            self.play_order.push(first_player);

            tracing::info!("Player status: {:#?}", self.player_status());
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

    fn update_bid(&mut self, player_id: String, bid: &i32) -> Result<(), String> {
        tracing::info!("Player {} to bid", player_id);
        let mut client = self.players.get_mut(&player_id).unwrap();

        match validate_bid(
            &bid,
            self.curr_round,
            &self.bids,
            self.dealing_order[0] == client.id,
        ) {
            Ok(x) => {
                tracing::info!("bid was: {}", x);
                self.bids.insert(client.id.clone(), x);
                return Ok(());
            }
            Err(e) => {
                tracing::info!("Error with bid: {:?}", e);
                return Err("Bid not valid".to_string());
            }
        }
    }

    fn play_round(&mut self) {
        for handnum in 1..=self.curr_round {
            tracing::info!(
                "--- Hand #{}/{} - Trump: {}---",
                handnum,
                self.curr_round,
                self.trump
            );
            // need to use a few things to see who goes first
            // 1. highest bid (at round start)
            // 2. person who won the trick in last round goes first, then obey existing order

            // ask for input from each client in specific order (first person after dealer)
            let mut played_cards: Vec<Card> = vec![];

            let mut curr_winning_card: Option<Card> = None;

            for player_id in self.play_order.iter() {
                let player = self.players.get_mut(player_id).unwrap();

                let valid_cards_to_play = player
                    .hand
                    .iter()
                    .filter_map(|card| {
                        match is_played_card_valid(&played_cards, &player.hand, card, &self.trump) {
                            Ok(x) => Some(x),
                            Err(err) => None,
                        }
                    })
                    .collect::<Vec<Card>>();

                let (loc, mut card) = player.play_card(&valid_cards_to_play);
                loop {
                    match is_played_card_valid(
                        &played_cards.clone(),
                        &mut player.hand,
                        &card.clone(),
                        &self.trump,
                    ) {
                        Ok(x) => {
                            tracing::info!("card is valid");
                            card = x;
                            // remove the card from the players hand
                            player.hand.remove(loc);
                            break;
                        }
                        Err(e) => {
                            tracing::info!("card is NOT valid: {:?}", e);
                            (_, card) = player.play_card(&valid_cards_to_play);
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

                tracing::info!(
                    "Curr winning card: {:?}",
                    curr_winning_card.clone().unwrap()
                );
            }

            tracing::info!("End turn, trump={:?}, played cards:", self.trump);
            played_cards
                .clone()
                .iter()
                .for_each(|c| tracing::info!("{}", c));

            let win_card = curr_winning_card.unwrap();
            tracing::info!(
                "Player {:?} won. Winning card: {:?}",
                win_card.played_by,
                win_card
            );
            let winner = win_card.played_by;
            if let Some(x) = self.wins.get_mut(&winner.unwrap()) {
                *x = *x + 1;
            }
        }
    }

    fn deal(&mut self) {
        tracing::info!("=== Dealing ===");
        tracing::info!("Dealer: {}", self.dealing_order[0]);
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.curr_round {
            // get random card, give to a player
            for player_id in self.dealing_order.iter() {
                let card = get_random_card(&mut self.deck).unwrap();
                let player: &mut GameClient = self.players.get_mut(player_id).unwrap();

                let mut new_card = card.clone();
                new_card.played_by = Some(player.id.clone());
                player.hand.push(new_card);
            }
        }
    }

    fn player_status(&self) {
        // tracing::info!("{:?}", self.players);
        tracing::info!("Score:\n{:?}", self.score);
    }
}

fn get_random_card(mut deck: &mut Vec<Card>) -> Option<Card> {
    fastrand::shuffle(&mut deck);
    return deck.pop();
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

#[derive(Debug)]
pub struct GameServer {
    pub players: HashMap<String, GameClient>,
    deck: Vec<Card>,
    curr_round: i32,
    trump: Suit,
    dealing_order: Vec<String>,
    play_order: Vec<String>,
    // dealer_id: i32,
    bids: HashMap<String, i32>,
    wins: HashMap<String, i32>,
    score: HashMap<String, i32>,
    state: GameState,

    pub tx: broadcast::Sender<FullGameState>,
    pub event_log: Vec<GameEvent>,
    pub event_queue: Vec<GameEvent>,
    // rx: broadcast::Receiver<String>,
    //     tx: broadcast::Sender<String>,
    //     rx: SplitStream<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub struct GameEvent {
    action: GameAction,
    origin: Actioner,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum GameAction {
    PlayCard(Card),
    Bid(i32),
    Deal,
    StartGame,
    GetHand,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Actioner {
    System,
    Player(String),
}

#[derive(Debug, Clone, Copy)]
pub enum PlayerState {
    Idle,
    RequireInput,
}

#[derive(Debug)]
pub enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
}


fn validate_bid(
    bid: &i32,
    curr_round: i32,
    curr_bids: &HashMap<String, i32>,
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
pub enum PlayedCardError {
    DidNotFollowSuit,
    CantUseTrump,
}

fn is_played_card_valid(
    played_cards: &Vec<Card>,
    hand: &Vec<Card>,
    played_card: &Card,
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
            return Ok(played_card.clone());
        } else {
            return Ok(played_card.clone());
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
    return Ok(played_card.clone());
}

pub enum EventType {
    PlayCard(Card),
    DealCard(Card),
    WinHand,
    WinRound,
    Bid(i32),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum GameState {
    Deal,
    Bid,
    Play,
    Pregame,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Suit {
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Card {
    id: usize,
    suit: Suit,
    value: i32,
    played_by: Option<String>,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let played_by = if self.played_by.is_some() {
            format!(" (Player {:?})", self.played_by)
        } else {
            String::new()
        };
        write!(f, "[{} {}]{}", self.value, self.suit, played_by)
    }
}
