use std::{borrow::Borrow, collections::HashMap, fmt};

use axum::extract::ws::{Message, WebSocket};
use bevy::utils::info;
use chrono::Utc;
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::{self, Sender};
use tracing::info;

use crate::{
    client::{GameClient, PlayerRole},
    GameMessage,
};

impl GameServer {
    pub fn update_to_next_state(&mut self) -> GameState {
        let newstate = match self.state {
            GameState::Bid => GameState::Play,
            // GameState::Play => GameState::PostRound,
            GameState::Pregame => GameState::Bid,
            GameState::Play => GameState::Bid,
            // GameState::PostRound => GameState::Bid,
            // GameState::PreRound => GameState::Bid,
        };
        self.state = newstate;
        return self.state;
    }

    pub fn process_event_pregame(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            // GameAction::PlayCard(_) => todo!(),
            // GameAction::Bid(_) => todo!(),
            GameAction::StartGame => self.setup_game(None),
            // GameAction::Deal => todo!(),
            _ => return None,
        }

        return Some(self.get_state());
    }

    pub fn process_event_bid(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            GameAction::Bid(bid) => {
                let res = self.update_bid(event.username.clone(), &bid);
                if res.is_ok() {
                    self.advance_player_turn();
                }
                if self.is_bidding_over() {
                    self.update_to_next_state();
                }
            }
            _ => {
                // None;
            }
        }

        return Some(self.get_state());
    }

    pub fn process_event_play(&mut self, event: GameMessage) -> Option<GameServer> {
        let player_id = event.username.clone();

        match &event.message.action {
            GameAction::PlayCard(card) => {
                let player = self.players.get_mut(&player_id).unwrap();

                match is_played_card_valid(
                    &self.curr_played_cards.clone(),
                    &mut player.hand,
                    &card.clone(),
                    &self.trump,
                ) {
                    Ok(x) => {
                        tracing::info!("card is valid");

                        // remove the card from the players hand
                        let mut cardloc: Option<usize> = None;
                        player.hand.iter().enumerate().for_each(|(i, c)| {
                            if c.id == card.id {
                                return cardloc = Some(i);
                            }
                        });

                        if let Some(loc) = cardloc {
                            player.hand.remove(loc);
                            self.curr_played_cards.push(card.clone());
                        }

                        self.curr_winning_card = Some(find_winning_card(
                            self.curr_played_cards.clone(),
                            self.trump.clone(),
                        ));

                        self.advance_player_turn();
                    }
                    Err(e) => {
                        info!("card is NOT valid: {:?}", e);
                        self.broadcast_message(format!("Card is not valid: {:?}", e));
                    }
                }
            }
            _ => {}
        }

        // in theory everyone played a card
        if self.curr_played_cards.len() == self.players.len() {
            self.end_hand();
        }

        // if all hands have been played, then we can end the round
        if self.wins.values().sum::<i32>() == self.curr_round {
            self.end_round();
        }

        return Some(self.get_state());
    }

    pub fn process_event(
        &mut self,
        events: Vec<GameMessage>,
        sender: &Sender<GameServer>,
        // player_id: String,
    ) {
        info!("[TODO] Processing an event");
        self.event_log.extend(events.clone());
        for event in events {
            // check if its the player's turn
            if self.state == GameState::Play
                && event
                    .username
                    .ne(&self.curr_player_turn.clone().unwrap_or("".into()))
            {
                info!(
                    "{}'s turn, not {}'s turn.",
                    self.curr_player_turn.clone().unwrap(),
                    event.username
                );
                self.broadcast_message(format!(
                    "{}'s turn, not {}'s turn.",
                    self.curr_player_turn.clone().unwrap(),
                    event.username
                ));
                // continue because we have multiple messages
                continue;
            }

            let state = match self.state {
                // Allow new players to join
                GameState::Pregame => self.process_event_pregame(event),
                // Get bids from all players
                GameState::Bid => self.process_event_bid(event),
                // Play cards starting with after dealer
                // Get winner once everyones played and start again with winner of round
                GameState::Play => self.process_event_play(event),
                // Find winner after
                // GameState::PostRound => self.process_postround(event),
            };

            // if let Some(state) = state {
            //     let _ = sender.send(state);
            // }
            // always send state for now
            let _ = sender.send(self.get_state());
        }
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
            // tx: broadcast::channel(10).0,
            event_log: vec![],
            // event_queue: vec![],
            curr_played_cards: vec![],
            curr_player_turn: None,
            curr_winning_card: None,
            system_status: vec![],
            // tx,
            // rx,
        };
        server
    }

    pub fn get_state(&self) -> Self {
        return self.clone();
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

    pub fn end_hand(&mut self) {
        tracing::info!("End turn, trump={:?}, played cards:", self.trump);
        self.curr_played_cards
            .clone()
            .iter()
            .for_each(|c| tracing::info!("{}", c));

        let winner = self.curr_winning_card.clone().unwrap().played_by.unwrap();

        if let Some(x) = self.wins.get_mut(&winner) {
            *x = *x + 1;
        }

        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
        self.curr_player_turn = Some(winner); // person who won the hand plays first next hand
    }

    pub fn end_round(&mut self) {
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
            self.bids.clear();
            self.deck = create_deck();
            player.clear_hand();
        }
        self.advance_trump();
        self.curr_round += 1;
        let curr_dealer = self.dealing_order.remove(0);
        self.dealing_order.push(curr_dealer);

        let first_player = self.play_order.remove(0);
        self.play_order.push(first_player);
        self.curr_played_cards = vec![];
        self.curr_player_turn = Some(self.play_order[0].clone());
        self.curr_winning_card = None;
        self.deal();
        self.update_to_next_state();

        tracing::info!("Player status: {:#?}", self.player_status());
    }

    pub fn broadcast_message(&mut self, message: String) {
        self.system_status.push(message);
    }

    pub fn setup_game(&mut self, max_rounds: Option<i32>) {
        if self.players.len() == 1 {
            // Should maybe send a better message
            // self.system_status.push("Not enough players".into());
            self.broadcast_message("Not enough players".to_string());
            return;
        }
        let mut deal_play_order: Vec<String> =
            self.players.iter().map(|(id, player)| id.clone()).collect();
        fastrand::shuffle(&mut deal_play_order);

        let mut play_order = deal_play_order.clone();
        let first = play_order.remove(0);
        play_order.push(first);

        self.play_order = play_order;
        self.dealing_order = deal_play_order;
        self.curr_player_turn = Some(self.play_order.get(0).unwrap().clone());

        self.players.iter().for_each(|(id, player)| {
            // self.bids.insert(id.clone(), 0);
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

        self.deal();
        self.update_to_next_state();

        tracing::info!("Players: {}\nRounds: {}", num_players, max_rounds);
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

    fn advance_player_turn(&mut self) {
        let mut next_player_idx = 0;
        for (i, player) in self.play_order.iter().enumerate() {
            if player == &self.curr_player_turn.clone().unwrap() {
                next_player_idx = i + 1;
            }
        }

        if next_player_idx == self.play_order.len() {
            self.curr_player_turn = Some(self.play_order.get(0).unwrap().clone());
        } else {
            self.curr_player_turn = Some(self.play_order.get(next_player_idx).unwrap().clone());
        }
    }

    fn update_bid(&mut self, player_id: String, bid: &i32) -> Result<i32, String> {
        tracing::info!("Player {} to bid", player_id);

        if self.curr_player_turn.clone().unwrap() != player_id {
            self.system_status
                .push(format!("Not player {}'s turn.", player_id));
            return Err("Not player {}'s turn.".to_string());
        }
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
                return Ok(x);
            }
            Err(e) => {
                tracing::info!("Error with bid: {:?}", e);
                self.broadcast_message(format!("Error with bid: {:?}", e));
                return Err("Bid not valid".to_string());
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

    fn is_bidding_over(&self) -> bool {
        // check if everyone has a bid
        return self.bids.keys().len() == self.players.len();
    }

    // fn process_postround(&mut self, event: GameMessage) -> Option<GameServer> {
    //     self.curr_played_cards = vec![];
    //     self.curr_winning_card = None;
    //     self.update_to_next_state();

    //     return Some(self.get_state());
    // }
}

fn find_winning_card(curr_played_cards: Vec<Card>, trump: Suit) -> Card {
    // scenarios for current played card
    // 1. Same suit, higher value = win, lower = lose
    // 2. Different suit, not trump = lose
    // 3. trump, higher than other trump = win, lower = lose
    let mut curr_winning_card = curr_played_cards[0].clone();
    for card in curr_played_cards {
        if card.suit == curr_winning_card.suit && card.value > curr_winning_card.value {
            curr_winning_card = card.clone();
        }

        if card.suit == trump && curr_winning_card.suit != trump {
            curr_winning_card = card.clone();
        }

        if card.suit == trump
            && curr_winning_card.suit == trump
            && card.value > curr_winning_card.value
        {
            curr_winning_card = card.clone();
        }
    }

    tracing::info!("Curr winning card: {:?}", curr_winning_card);
    return curr_winning_card;
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

#[derive(Debug, Clone, Serialize)]
pub struct GameServer {
    pub players: HashMap<String, GameClient>,
    deck: Vec<Card>,
    curr_round: i32,
    trump: Suit,
    dealing_order: Vec<String>,
    curr_played_cards: Vec<Card>,
    curr_player_turn: Option<String>,
    curr_winning_card: Option<Card>,
    play_order: Vec<String>,
    // dealer_id: i32,
    bids: HashMap<String, i32>,
    wins: HashMap<String, i32>,
    score: HashMap<String, i32>,
    state: GameState,
    // pub tx: broadcast::Sender<FullGameState>,
    pub event_log: Vec<GameMessage>,
    pub system_status: Vec<String>,
    // pub event_queue: Vec<GameEvent>,
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
    // Player actions
    PlayCard(Card),
    Bid(i32),

    // System actions
    StartGame,
    Deal,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Actioner {
    System,
    Player(String),
}

#[derive(Debug, Clone, Serialize)]
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
    // Deal,
    Bid,
    Play,
    Pregame,
    // PostRound,
    // PreRound,
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

mod tests {
    use super::{find_winning_card, Card, Suit};

    #[test]
    fn test_finding_winning_card() {
        let cards = vec![
            Card {
                id: 44,
                played_by: Some("person".to_string()),
                suit: Suit::Heart,
                value: 13,
            },
            Card {
                id: 51,
                played_by: Some("spade".to_string()),
                suit: Suit::Spade,
                value: 14,
            },
        ];
        let trump = Suit::Spade;
        let res = find_winning_card(cards, trump);
        println!("Winning: {}", res);
        assert!(res.id == 51)
    }

    #[test]
    fn test_finding_winning_card_same_suit() {
        let cards = vec![
            Card {
                id: 1,
                played_by: Some("person".to_string()),
                suit: Suit::Heart,
                value: 1,
            },
            Card {
                id: 2,
                played_by: Some("spade".to_string()),
                suit: Suit::Heart,
                value: 2,
            },
            Card {
                id: 3,
                played_by: Some("spade".to_string()),
                suit: Suit::Spade,
                value: 14,
            },
        ];
        let trump = Suit::Heart;
        let res = find_winning_card(cards, trump);
        println!("Winning: {}", res);
        assert!(res.id == 2)
    }

    #[test]
    fn test_finding_winning_card_no_trump_first_suit_wins() {
        let cards = vec![
            Card {
                id: 1,
                played_by: Some("person".to_string()),
                suit: Suit::Diamond,
                value: 1,
            },
            Card {
                id: 2,
                played_by: Some("spade".to_string()),
                suit: Suit::Club,
                value: 2,
            },
            Card {
                id: 3,
                played_by: Some("spade".to_string()),
                suit: Suit::Spade,
                value: 14,
            },
        ];
        let trump = Suit::Heart;
        let res = find_winning_card(cards, trump);
        println!("Winning: {}", res);
        assert!(res.id == 1)
    }
}
