use std::{collections::HashMap, fmt};

// use axum::extract::ws::{Message, WebSocket};
// use bevy::utils::info;

// use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
// use tokio::sync::broadcast::Sender;
use tracing::info;

use crate::{
    create_deck, Card, GameAction, GameClient, GameError, GameMessage, GameServer, GameState,
    PlayerRole, Suit,
};

/*
THINGS TO FIX
1. fix dealer not updating nicely
2. Player who bid most not needing to go first
3. Errors for game related things should be an enum
*/

struct MyGameServer {
    server: GameServer,
}

impl MyGameServer {}

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

        tracing::info!("=== Transition: {:?} -> {:?} ===", self.state, newstate);
        self.state = newstate;
        self.state
    }

    pub fn process_event_pregame(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            // GameAction::PlayCard(_) => todo!(),
            // GameAction::Bid(_) => todo!(),
            GameAction::StartGame => self.setup_game(None),
            // GameAction::Deal => todo!(),
            _ => return None,
        }

        Some(self.get_state())
    }

    pub fn process_event_bid(&mut self, event: GameMessage) -> Option<GameServer> {
        match event.message.action {
            GameAction::Bid(bid) => {
                let res = self.update_bid(event.username.clone(), &bid);

                if res.is_ok() {
                    self.curr_player_turn = Some(advance_player_turn(
                        &self.curr_player_turn.clone().unwrap(),
                        &self.player_order,
                    ));
                }

                if self.is_bidding_over() {
                    // need to update who plays next based on the bids
                    self.curr_player_turn = Some(update_curr_player_from_bids(&self.bid_order));
                    self.update_to_next_state();
                }
            }
            _ => {
                // None;
            }
        }

        Some(self.get_state())
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
                                cardloc = Some(i)
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

                        self.curr_player_turn = Some(advance_player_turn(
                            &self.curr_player_turn.clone().unwrap(),
                            &self.player_order,
                        ));
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

        Some(self.get_state())
    }

    pub fn process_event(
        &mut self,
        events: Vec<GameMessage>,
        // sender: &Sender<GameServer>,
        // player_id: String,
    ) -> GameServer {
        info!("[TODO] Processing an event");
        self.event_log.extend(events.clone());

        for event in events {
            if event.message.action == GameAction::CurrentState {
                // let _ = sender.send(self.get_state());

                continue;
            }

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
        }

        return self.get_state();
    }

    pub fn get_state(&self) -> Self {
        let mut cloned = self.clone();
        cloned.deck = vec![];
        // cloned.players = HashMap::new();
        cloned
    }

    fn add_player(
        &mut self,
        player_id: String,
        // rx: SplitStream<WebSocket>,
        // sender: SplitSink<WebSocket, Message>,
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
            *x += 1;
        }

        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
        // self.bid_order = vec![];
        self.curr_player_turn = Some(winner); // person who won the hand plays first next hand
    }

    pub fn end_round(&mut self) {
        tracing::info!("Bids won: {:#?}\nBids wanted: {:#?}", self.wins, self.bids);
        for (player_id, player) in self.players.iter_mut() {
            // let player = self.players.get_mut(player_id).unwrap();

            if self.wins.get(&player.id) == self.bids.get(&player.id) {
                let bidscore = self.bids.get(&player.id).unwrap() + 10;
                let curr_score = self.score.get_mut(&player.id).unwrap();
                *curr_score += bidscore;
            }

            // resetting the data structures for a round before round start
            self.wins.insert(player.id.clone(), 0);
            player.clear_hand();
        }
        self.bids.clear();
        self.deck = create_deck();
        self.advance_trump();
        self.advance_dealer();
        self.curr_player_turn = Some(advance_player_turn(
            &self.curr_player_turn.clone().unwrap(),
            &self.player_order,
        ));
        self.curr_round += 1;

        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
        self.bid_order = vec![];
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

        self.player_order = deal_play_order;
        self.curr_dealer = self.player_order[0].clone();
        // person after dealer
        self.curr_player_turn = Some(self.player_order.get(1).unwrap().clone());

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
        self.deck.pop()
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

    fn update_bid(&mut self, player_id: String, bid: &i32) -> Result<i32, String> {
        tracing::info!("Player {} to bid", player_id);

        if self.curr_player_turn.clone().unwrap() != player_id {
            self.system_status
                .push(format!("Not player {}'s turn.", player_id));
            return Err("Not player {}'s turn.".to_string());
        }
        let client = self.players.get_mut(&player_id).unwrap();

        match validate_bid(
            bid,
            self.curr_round,
            &self.bids,
            self.curr_dealer == client.id,
        ) {
            Ok(x) => {
                tracing::info!("bid was: {}", x);
                self.bids.insert(client.id.clone(), x);
                self.bid_order.push((client.id.clone(), x));
                Ok(x)
            }
            Err(e) => {
                tracing::info!("Error with bid: {:?}", e);
                self.broadcast_message(format!("Error with bid: {:?}", e));
                Err("Bid not valid".to_string())
            }
        }
    }

    fn deal(&mut self) {
        tracing::info!("=== Dealing ===");
        tracing::info!("Dealer: {}", self.player_order[0]);
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.curr_round {
            // get random card, give to a player
            for player_id in self.player_order.iter() {
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

    fn advance_dealer(&mut self) -> Result<String, GameError> {
        for (i, player) in self.player_order.iter().enumerate() {
            if player.eq(&self.curr_dealer) {
                let nextdealer = match self.player_order.get(i + 1) {
                    Some(x) => x,
                    None => &self.player_order[0],
                };
                self.curr_dealer = nextdealer.clone();
                return Ok(self.curr_dealer.clone());
            }
        }
        return Err(GameError::InternalIssue(String::from(
            "Could not advance dealer",
        )));
    }
    // fn process_postround(&mut self, event: GameMessage) -> Option<GameServer> {
    //     self.curr_played_cards = vec![];
    //     self.curr_winning_card = None;
    //     self.update_to_next_state();

    //     return Some(self.get_state());
    // }

    pub fn new() -> Self {
        // let (tx, rx) = broadcast::channel(10);

        GameServer {
            players: HashMap::new(),
            deck: create_deck(),
            curr_round: 1,
            trump: Suit::Heart,
            player_order: vec![],
            // play_order: vec![],
            bids: HashMap::new(),
            bid_order: Vec::new(),
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
            curr_dealer: String::new(),
            system_status: vec![],
            players_secrets: HashMap::new(),
            // tx,
            // rx,
        }
    }
}

fn update_curr_player_from_bids(bid_order: &Vec<(String, i32)>) -> String {
    info!(
        "{}",
        format!("Finding first player based on bid. Bids: {:?}", bid_order)
    );
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
    use chrono::Utc;

    use crate::{
        game::{advance_player_turn, find_winning_card, update_curr_player_from_bids},
        Card, GameMessage, GameServer, GameState, PlayerRole, Suit,
    };

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

    #[test]
    fn test_game_setup_and_round_end() {
        let PLAYER_ONE = "p1".to_string();
        let PLAYER_TWO = "p2".to_string();

        let mut game = GameServer::new();
        game.add_player(PLAYER_ONE.clone(), PlayerRole::Leader);
        game.add_player(PLAYER_TWO.clone(), PlayerRole::Player);

        game.process_event(vec![GameMessage {
            username: PLAYER_ONE.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::StartGame,
                origin: crate::Actioner::Player(PLAYER_ONE.clone()),
            },
            timestamp: Utc::now(),
        }]);

        let first_dealer = game.curr_dealer.clone();
        let has_first_turn = game.curr_player_turn.clone().unwrap();
        let has_second_turn = game.curr_dealer.clone();

        println!("Game details @StartGame: {:#?}", game.get_state());

        assert_ne!(has_first_turn, has_second_turn);
        assert_eq!(first_dealer, has_second_turn); // first dealer goes second
        assert_eq!(game.curr_player_turn.clone().unwrap(), has_first_turn);

        game.process_event(vec![
            GameMessage {
                username: has_first_turn.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::Bid(0),
                    origin: crate::Actioner::Player(has_first_turn.clone()),
                },
                timestamp: Utc::now(),
            },
            GameMessage {
                username: has_second_turn.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::Bid(0),
                    origin: crate::Actioner::Player(has_second_turn.clone()),
                },
                timestamp: Utc::now(),
            },
        ]);

        // Time to play
        let p1_card = game
            .players
            .get(&has_first_turn)
            .unwrap()
            .hand
            .first()
            .unwrap();
        let p2_card = game
            .players
            .get(&has_second_turn)
            .unwrap()
            .hand
            .first()
            .unwrap();

        game.process_event(vec![
            GameMessage {
                username: has_first_turn.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::PlayCard(p1_card.clone()),
                    origin: crate::Actioner::Player(has_first_turn.clone()),
                },
                timestamp: Utc::now(),
            },
            GameMessage {
                username: has_second_turn.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::PlayCard(p2_card.clone()),
                    origin: crate::Actioner::Player(has_second_turn.clone()),
                },
                timestamp: Utc::now(),
            },
        ]);

        println!("Game details @Bid - Round 2: {:#?}", game.get_state());

        assert_eq!(game.state, GameState::Bid);
        assert_eq!(first_dealer, game.curr_player_turn.clone().unwrap()); // round 1 dealer goes first in round 2
        assert_eq!(has_second_turn, game.curr_dealer.clone()); // round 1 first player is now dealer
    }
}
