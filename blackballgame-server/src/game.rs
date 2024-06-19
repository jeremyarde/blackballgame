use std::{collections::HashMap, fmt};

use axum::extract::ws::{Message, WebSocket};
use bevy::utils::info;
use common::{
    create_deck, Card, GameAction, GameClient, GameMessage, GameServer, GameState, PlayerRole, Suit,
};
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Sender;
use tracing::info;

/*
THINGS TO FIX
1. fix dealer not updating nicely
2. Player who bid most not needing to go first
3. Errors for game related things should be an enum
*/

struct MyGameServer {
    server: GameServer,
}

impl MyGameServer {
    pub fn update_to_next_state(&mut self) -> GameState {
        let newstate = match self.server.state {
            GameState::Bid => GameState::Play,
            // GameState::Play => GameState::PostRound,
            GameState::Pregame => GameState::Bid,
            GameState::Play => GameState::Bid,
            // GameState::PostRound => GameState::Bid,
            // GameState::PreRound => GameState::Bid,
        };

        tracing::info!(
            "=== Transition: {:?} -> {:?} ===",
            self.server.state,
            newstate
        );
        self.server.state = newstate;
        self.server.state
    }

    pub fn process_event_pregame(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            // GameAction::PlayCard(_) => todo!(),
            // GameAction::Bid(_) => todo!(),
            GameAction::StartGame => self.server.setup_game(None),
            // GameAction::Deal => todo!(),
            _ => return None,
        }

        Some(self.server.get_state())
    }

    pub fn process_event_bid(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            GameAction::Bid(bid) => {
                let res = self.server.update_bid(event.username.clone(), &bid);

                if res.is_ok() {
                    self.server.curr_player_turn = Some(advance_player_turn(
                        &self.server.curr_player_turn.clone().unwrap(),
                        &self.server.player_order,
                    ));
                }

                if self.server.is_bidding_over() {
                    // need to update who plays next based on the bids
                    self.server.curr_player_turn =
                        Some(update_curr_player_from_bids(&self.server.bid_order));
                    self.server.update_to_next_state();
                }
            }
            _ => {
                // None;
            }
        }

        Some(self.server.get_state())
    }

    pub fn process_event_play(&mut self, event: GameMessage) -> Option<GameServer> {
        let player_id = event.username.clone();

        match &event.message.action {
            GameAction::PlayCard(card) => {
                let player = self.server.players.get_mut(&player_id).unwrap();

                match is_played_card_valid(
                    &self.server.curr_played_cards.clone(),
                    &mut player.hand,
                    &card.clone(),
                    &self.server.trump,
                ) {
                    Ok(x) => {
                        tracing::info!("card is valid");

                        // remove the card from the players hand
                        let mut cardloc: Option<usize> = None;
                        player.hand.iter().enumerate().for_each(|(i, c)| {
                            if c.id == card.id {
                                cardloc = Some(i)
                            }
                        });

                        if let Some(loc) = cardloc {
                            player.hand.remove(loc);
                            self.server.curr_played_cards.push(card.clone());
                        }

                        self.server.curr_winning_card = Some(find_winning_card(
                            self.server.curr_played_cards.clone(),
                            self.server.trump.clone(),
                        ));

                        self.server.curr_player_turn = Some(advance_player_turn(
                            &self.server.curr_player_turn.clone().unwrap(),
                            &self.server.player_order,
                        ));
                    }
                    Err(e) => {
                        info!("card is NOT valid: {:?}", e);
                        self.server
                            .broadcast_message(format!("Card is not valid: {:?}", e));
                    }
                }
            }
            _ => {}
        }

        // in theory everyone played a card
        if self.server.curr_played_cards.len() == self.server.players.len() {
            self.server.end_hand();
        }

        // if all hands have been played, then we can end the round
        if self.server.wins.values().sum::<i32>() == self.server.curr_round {
            self.server.end_round();
        }

        Some(self.server.get_state())
    }

    pub fn process_event(
        &mut self,
        events: Vec<GameMessage>,
        sender: &Sender<GameServer>,
        // player_id: String,
    ) {
        info!("[TODO] Processing an event");
        self.server.event_log.extend(events.clone());
        for event in events {
            if event.message.action == GameAction::CurrentState {
                let _ = sender.send(self.server.get_state());
                continue;
            }

            // check if its the player's turn
            if self.server.state == GameState::Play
                && event
                    .username
                    .ne(&self.server.curr_player_turn.clone().unwrap_or("".into()))
            {
                info!(
                    "{}'s turn, not {}'s turn.",
                    self.server.curr_player_turn.clone().unwrap(),
                    event.username
                );
                self.server.broadcast_message(format!(
                    "{}'s turn, not {}'s turn.",
                    self.server.curr_player_turn.clone().unwrap(),
                    event.username
                ));
                // continue because we have multiple messages
                continue;
            }

            let state = match self.server.state {
                // Allow new players to join
                GameState::Pregame => self.server.process_event_pregame(event),
                // Get bids from all players
                GameState::Bid => self.server.process_event_bid(event),
                // Play cards starting with after dealer
                // Get winner once everyones played and start again with winner of round
                GameState::Play => self.server.process_event_play(event),
                // Find winner after
                // GameState::PostRound => self.server.process_postround(event),
            };

            // if let Some(state) = state {
            //     let _ = sender.send(state);
            // }
            // always send state for now
            let _ = sender.send(self.server.get_state());
        }
    }

    pub fn get_state(&self) -> Self {
        let mut cloned = self.server.clone();
        cloned.deck = vec![];
        // cloned.players = HashMap::new();
        cloned
    }

    fn add_player(
        &mut self,
        player_id: String,
        rx: SplitStream<WebSocket>,
        sender: SplitSink<WebSocket, Message>,
        role: PlayerRole,
    ) {
        self.server
            .players
            .insert(player_id.clone(), GameClient::new(player_id, role));
    }

    pub fn end_hand(&mut self) {
        tracing::info!("End turn, trump={:?}, played cards:", self.server.trump);
        self.server
            .curr_played_cards
            .clone()
            .iter()
            .for_each(|c| tracing::info!("{}", c));

        let winner = self
            .server
            .curr_winning_card
            .clone()
            .unwrap()
            .played_by
            .unwrap();

        if let Some(x) = self.server.wins.get_mut(&winner) {
            *x += 1;
        }

        self.server.curr_played_cards = vec![];
        self.server.curr_winning_card = None;
        // self.server.bid_order = vec![];
        self.server.curr_player_turn = Some(winner); // person who won the hand plays first next hand
    }

    pub fn end_round(&mut self) {
        tracing::info!(
            "Bids won: {:#?}\nBids wanted: {:#?}",
            self.server.wins,
            self.server.bids
        );
        for (player_id, player) in self.server.players.iter_mut() {
            // let player = self.server.players.get_mut(player_id).unwrap();

            if self.server.wins.get(&player.id) == self.server.bids.get(&player.id) {
                let bidscore = self.server.bids.get(&player.id).unwrap() + 10;
                let curr_score = self.server.score.get_mut(&player.id).unwrap();
                *curr_score += bidscore;
            }

            // resetting the data structures for a round before round start
            self.server.wins.insert(player.id.clone(), 0);
            player.clear_hand();
        }
        self.server.bids.clear();
        self.server.deck = create_deck();
        self.server.advance_trump();
        self.server.advance_dealer();
        self.server.curr_player_turn = Some(advance_player_turn(
            &self.server.curr_player_turn.clone().unwrap(),
            &self.server.player_order,
        ));
        self.server.curr_round += 1;

        self.server.curr_played_cards = vec![];
        self.server.curr_winning_card = None;
        self.server.bid_order = vec![];
        self.server.deal();
        self.server.update_to_next_state();

        tracing::info!("Player status: {:#?}", self.server.player_status());
    }

    pub fn broadcast_message(&mut self, message: String) {
        self.server.system_status.push(message);
    }

    pub fn setup_game(&mut self, max_rounds: Option<i32>) {
        if self.server.players.len() == 1 {
            // Should maybe send a better message
            // self.server.system_status.push("Not enough players".into());
            self.server
                .broadcast_message("Not enough players".to_string());
            return;
        }
        let mut deal_play_order: Vec<String> = self
            .server
            .players
            .iter()
            .map(|(id, player)| id.clone())
            .collect();
        fastrand::shuffle(&mut deal_play_order);

        self.server.player_order = deal_play_order;
        self.server.curr_dealer = self.server.player_order[0].clone();
        // person after dealer
        self.server.curr_player_turn = Some(self.server.player_order.get(1).unwrap().clone());

        self.server.players.iter().for_each(|(id, player)| {
            // self.server.bids.insert(id.clone(), 0);
            self.server.wins.insert(id.clone(), 0);
            self.server.score.insert(id.clone(), 0);
        });

        let num_players = self.server.players.len() as i32;

        let max_rounds = if max_rounds.is_some() {
            max_rounds.unwrap()
        } else if 52i32.div_euclid(num_players) > 9 {
            9
        } else {
            52i32.div_euclid(num_players)
        };

        self.server.deal();
        self.server.update_to_next_state();

        tracing::info!("Players: {}\nRounds: {}", num_players, max_rounds);
    }

    fn get_random_card(&mut self) -> Option<Card> {
        fastrand::shuffle(&mut self.server.deck);
        self.server.deck.pop()
    }

    fn advance_trump(&mut self) {
        match self.server.trump {
            Suit::Heart => self.server.trump = Suit::Diamond,
            Suit::Diamond => self.server.trump = Suit::Club,
            Suit::Club => self.server.trump = Suit::Spade,
            Suit::Spade => self.server.trump = Suit::NoTrump,
            Suit::NoTrump => self.server.trump = Suit::Heart,
        }
    }

    fn update_bid(&mut self, player_id: String, bid: &i32) -> Result<i32, String> {
        tracing::info!("Player {} to bid", player_id);

        if self.server.curr_player_turn.clone().unwrap() != player_id {
            self.server
                .system_status
                .push(format!("Not player {}'s turn.", player_id));
            return Err("Not player {}'s turn.".to_string());
        }
        let client = self.server.players.get_mut(&player_id).unwrap();

        match validate_bid(
            bid,
            self.server.curr_round,
            &self.server.bids,
            self.server.curr_dealer == client.id,
        ) {
            Ok(x) => {
                tracing::info!("bid was: {}", x);
                self.server.bids.insert(client.id.clone(), x);
                self.server.bid_order.push((client.id.clone(), x));
                Ok(x)
            }
            Err(e) => {
                tracing::info!("Error with bid: {:?}", e);
                self.server
                    .broadcast_message(format!("Error with bid: {:?}", e));
                Err("Bid not valid".to_string())
            }
        }
    }

    fn deal(&mut self) {
        tracing::info!("=== Dealing ===");
        tracing::info!("Dealer: {}", self.server.player_order[0]);
        fastrand::shuffle(&mut self.server.deck);

        for i in 1..=self.server.curr_round {
            // get random card, give to a player
            for player_id in self.server.player_order.iter() {
                let card = get_random_card(&mut self.server.deck).unwrap();
                let player: &mut GameClient = self.server.players.get_mut(player_id).unwrap();

                let mut new_card = card.clone();
                new_card.played_by = Some(player.id.clone());
                player.hand.push(new_card);
            }
        }
    }

    fn player_status(&self) {
        // tracing::info!("{:?}", self.server.players);
        tracing::info!("Score:\n{:?}", self.server.score);
    }

    fn is_bidding_over(&self) -> bool {
        // check if everyone has a bid
        return self.server.bids.keys().len() == self.server.players.len();
    }

    fn advance_dealer(&mut self) -> Result<String, GameError> {
        for (i, player) in self.server.player_order.iter().enumerate() {
            if player.eq(&self.server.curr_dealer) {
                let nextdealer = match self.server.player_order.get(i + 1) {
                    Some(x) => x,
                    None => &self.server.player_order[0],
                };
                self.server.curr_dealer = nextdealer.clone();
                return Ok(self.server.curr_dealer.clone());
            }
        }
        return Err(GameError::InternalIssue(String::from(
            "Could not advance dealer",
        )));
    }
    // fn process_postround(&mut self, event: GameMessage) -> Option<GameServer> {
    //     self.server.curr_played_cards = vec![];
    //     self.server.curr_winning_card = None;
    //     self.server.update_to_next_state();

    //     return Some(self.server.get_state());
    // }
}

enum GameError {
    InternalIssue(String),
}

fn advance_player_turn(curr: &String, players: &Vec<String>) -> String {
    let mut loc = 0;
    for (i, p) in players.iter().enumerate() {
        if p == curr {
            loc = i;
        }
    }

    // Current player is at the end of the order, loop back around to the start
    if loc + 1 == players.len() {
        return players[0].clone();
    }

    players[loc + 1].clone()
}

fn update_curr_player_from_bids(bid_order: &Vec<(String, i32)>) -> String {
    info(format!(
        "Finding first player based on bid. Bids: {:?}",
        bid_order
    ));
    let mut curr_highest_bid = bid_order[0].clone();
    for (player, bid) in bid_order.iter() {
        if bid > &curr_highest_bid.1 {
            curr_highest_bid = (player.to_string(), *bid);
        }
    }
    curr_highest_bid.0
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
    curr_winning_card
}

fn get_random_card(deck: &mut Vec<Card>) -> Option<Card> {
    fastrand::shuffle(deck);
    deck.pop()
}

// fn create_deck() -> Vec<Card> {
//     let mut cards = vec![];

//     // 14 = Ace
//     let mut cardid = 0;
//     for value in 2..=14 {
//         cards.push(Card {
//             id: cardid,
//             suit: Suit::Heart,
//             value,
//             played_by: None,
//         });
//         cards.push(Card {
//             id: cardid + 1,
//             suit: Suit::Diamond,
//             value,
//             played_by: None,
//         });
//         cards.push(Card {
//             id: cardid + 2,
//             suit: Suit::Club,
//             played_by: None,

//             value,
//         });
//         cards.push(Card {
//             id: cardid + 3,
//             suit: Suit::Spade,
//             value,
//             played_by: None,
//         });
//         cardid += 4;
//     }

//     cards
// }

// #[derive(Debug, Clone, Serialize)]
// pub struct GameServer {
//     pub players: HashMap<String, GameClient>,
//     pub players_secrets: HashMap<String, String>,
//     deck: Vec<Card>,
//     curr_round: i32,
//     trump: Suit,
//     player_order: Vec<String>,
//     curr_played_cards: Vec<Card>,
//     curr_player_turn: Option<String>,
//     curr_winning_card: Option<Card>,
//     curr_dealer: String,
//     // play_order: Vec<String>,
//     // dealer_id: i32,
//     bids: HashMap<String, i32>,
//     bid_order: Vec<(String, i32)>,
//     // bid_order: Vec<
//     wins: HashMap<String, i32>,
//     score: HashMap<String, i32>,
//     state: GameState,
//     // pub tx: broadcast::Sender<FullGameState>,
//     pub event_log: Vec<GameMessage>,
//     pub system_status: Vec<String>,
//     // pub event_queue: Vec<GameEvent>,
//     // rx: broadcast::Receiver<String>,
//     //     tx: broadcast::Sender<String>,
//     //     rx: SplitStream<Message>,
// }

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

    Ok(*bid)
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

    if played_cards.is_empty() {
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

    let led_suit = played_cards.first().unwrap().suit.clone();
    if led_suit != played_card.suit {
        // make sure player does not have that suit
        for c in hand {
            if c.suit == led_suit {
                return Err(PlayedCardError::DidNotFollowSuit);
            }
        }
    }
    Ok(played_card.clone())
}

pub enum EventType {
    PlayCard(Card),
    DealCard(Card),
    WinHand,
    WinRound,
    Bid(i32),
}

// #[derive(Debug, Clone, Copy, Serialize, PartialEq)]
// pub enum GameState {
//     // Deal,
//     Bid,
//     Play,
//     Pregame,
//     // PostRound,
//     // PreRound,
// }

mod tests {
    use common::{Card, Suit};

    use crate::game::{advance_player_turn, find_winning_card, update_curr_player_from_bids};

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

    #[test]
    fn test_get_curr_player_from_bids() {
        let bid_order = vec![
            ("P1".to_string(), 2),
            ("P2".to_string(), 1),
            ("P3".to_string(), 3),
        ];
        let next = update_curr_player_from_bids(&bid_order);

        assert!(next == "P3");
    }

    #[test]
    fn test_get_curr_player_from_bids_multiple_same_bid() {
        let bid_order = vec![
            ("P1".to_string(), 6),
            ("P2".to_string(), 0),
            ("P3".to_string(), 6),
        ];
        let next = update_curr_player_from_bids(&bid_order);

        assert!(next == "P1");
    }

    #[test]
    fn test_get_curr_player_from_bids_no_bids() {
        let bid_order = vec![
            ("P1".to_string(), 0),
            ("P2".to_string(), 0),
            ("P3".to_string(), 0),
        ];
        let next = update_curr_player_from_bids(&bid_order);

        assert!(next == "P1");
    }

    #[test]
    fn test_advance_player_turn() {
        let players = vec![
            "P1".to_string(),
            "P2".to_string(),
            "P3".to_string(),
            "P4".to_string(),
        ];

        let curr = "P1".to_string();
        let res = advance_player_turn(&curr, &players);

        assert!(res == "P2".to_string());
    }
    #[test]
    fn test_advance_player_turn_end_player() {
        let players = vec![
            "P1".to_string(),
            "P2".to_string(),
            "P3".to_string(),
            "P4".to_string(),
        ];

        let curr = "P4".to_string();
        let res = advance_player_turn(&curr, &players);

        assert!(res == "P1".to_string());
    }
}
