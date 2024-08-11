use std::collections::HashMap;

use chrono::Utc;
use data_encoding::BASE64;

use nanoid::nanoid_gen;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};

use crate::{
    create_deck, Card, Connect, Destination, GameAction, GameClient, GameError, GameEventResult,
    GameMessage, GameState, GameplayState, PlayState, PlayerDetails, PlayerRole, SetupGameOptions,
    Suit,
};

impl GameState {
    pub fn update_to_next_state(&mut self) {
        let newstate = match &self.gameplay_state {
            GameplayState::Bid => GameplayState::Play(PlayState::new()),
            GameplayState::Pregame => GameplayState::Bid,
            GameplayState::Play(ps) => {
                info!(
                    "jere/ update state play -> ?? {}, {}",
                    ps.hand_num, self.curr_round
                );
                // move to new "hand" in the round when each player played a card
                if self.curr_played_cards.len() == self.player_order.len() {
                    GameplayState::PostHand(ps.clone())
                } else {
                    self.gameplay_state.clone()
                }
                // GameplayState::PostHand(ps.clone())
            }
            GameplayState::PostHand(ps) => {
                if ps.hand_num >= self.curr_round.try_into().unwrap() {
                    GameplayState::PostRound
                } else {
                    GameplayState::Play(PlayState::from(ps.hand_num + 1))
                }
            }
            GameplayState::PostRound => {
                self.curr_played_cards = vec![];
                self.curr_winning_card = None;
                GameplayState::Bid
            }
        };

        tracing::info!(
            "=== Transition: {:?} -> {:?} ===",
            self.gameplay_state,
            newstate
        );
        // self.transition_state(newstate.clone());
        self.gameplay_state = newstate;
    }

    pub fn process_event_pregame(&mut self, event: GameMessage) {
        if let GameAction::StartGame(sgo) = event.message.action {
            let result = self.setup_game(sgo);
            info!("Setup game result: {:?}", result);
        }
    }

    fn is_correct_player_turn(&mut self, event: &GameMessage) -> bool {
        if &self.curr_player_turn.clone().unwrap_or("".to_string()) != &event.username {
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
            self.system_status.push(format!(
                "{}'s turn, not {}'s turn.",
                self.curr_player_turn.clone().unwrap(),
                event.username
            ));
            return false;
        }
        true
    }

    pub fn process_event_bid(&mut self, event: GameMessage) {
        if !self.is_correct_player_turn(&event) {
            return;
        };

        if let GameAction::Bid(bid) = event.message.action {
            let res = self.update_bid(event.username.clone(), &bid);
            info!("Bid result: {:?}", res);
            if res.is_ok() {
                let (next_turn_idx, next_turn) =
                    self.advance_turn(self.curr_player_turn_idx, &self.player_order);
                self.curr_player_turn_idx = next_turn_idx;
                self.curr_player_turn = Some(next_turn);
            }

            if self.is_bidding_over() {
                let mut curr_highest_bid = self.bid_order[0].clone();
                for (player, bid) in self.bid_order.iter() {
                    if bid > &curr_highest_bid.1 {
                        curr_highest_bid = (player.to_string(), *bid);
                    }
                }
                self.set_curr_player_turn(&curr_highest_bid.0);
                self.update_to_next_state();
            }
        }
    }

    pub fn process_event_postround(&mut self, event: GameMessage) {
        // if self.is_correct_player_turn(&event) == false {
        //     return None;
        // };

        match event.message.action {
            GameAction::Deal | GameAction::Ack => {
                self.start_next_round();
            }
            _ => {}
        }
    }

    pub fn process_event_play(&mut self, event: GameMessage) {
        if !self.is_correct_player_turn(&event) {
            return;
        };
        let player_id = event.username.clone();

        if let GameAction::PlayCard(card) = &event.message.action {
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
                    player.hand.remove(cardloc.unwrap());

                    // encrypt player hand again
                    self.encrypt_player_hand(&player_id);

                    // add card to curr_played_cards
                    self.curr_played_cards.push(x.clone());

                    self.curr_winning_card = Some(find_winning_card(
                        self.curr_played_cards.clone(),
                        self.trump.clone(),
                    ));

                    let (next_turn_idx, next_turn) =
                        self.advance_turn(self.curr_player_turn_idx, &self.player_order);
                    self.curr_player_turn_idx = next_turn_idx;
                    self.curr_player_turn = Some(next_turn);
                }
                Err(e) => {
                    info!("card is NOT valid: {:?}", e);
                    self.broadcast_message(format!("Card is not valid: {:?}", e));
                }
            }
        }

        // in theory everyone played a card
        if self.curr_played_cards.len() == self.players.len() {
            self.end_hand();
        }

        self.update_to_next_state();
    }

    pub fn process_event(
        &mut self,
        event: GameMessage,
        // sender: &Sender<GameServer>,
        // player_id: String,
    ) -> GameEventResult {
        // info!("[TODO] Processing an event");
        // self.event_log.extend(events.clone());

        info!("Processing event: {:?}", event);
        match &event.message.action {
            // GameAction::PlayCard(_) => todo!(),
            // GameAction::Bid(_) => todo!(),
            // GameAction::Ack => todo!(),
            // GameAction::StartGame(_) => todo!(),
            // GameAction::Deal => todo!(),
            // GameAction::CurrentState => todo!(),
            GameAction::Connect {
                username,
                channel,
                secret,
            } => {
                let secret = self.add_player(
                    username.clone(),
                    PlayerRole::Player,
                    "Connect action".to_string(),
                );
                return GameEventResult {
                    dest: Destination::User(PlayerDetails {
                        username: event.username.clone(),
                        ip: String::new(),
                    }),
                    msg: crate::GameActionResponse::Connect(Connect {
                        username: event.username.clone(),
                        channel: self.lobby_code.clone(),
                        secret: Some(secret),
                    }),
                };
            }
            GameAction::PlayCard(_) => {}
            GameAction::Bid(_) => {}
            GameAction::Ack => {}
            GameAction::StartGame(_) => {}
            GameAction::Deal => {}
            GameAction::CurrentState => {}
            GameAction::JoinGame(player) => {
                // let secret = self.add_player(
                //     username.clone(),
                //     PlayerRole::Player,
                //     "Connect action".to_string(),
                // );
                let secret = self.add_player(
                    player.username.clone(),
                    PlayerRole::Player,
                    player.ip.clone(),
                );
                return GameEventResult {
                    dest: Destination::User(PlayerDetails {
                        username: event.username.clone(),
                        ip: String::new(),
                    }),
                    msg: crate::GameActionResponse::Connect(Connect {
                        username: event.username.clone(),
                        channel: self.lobby_code.clone(),
                        secret: Some(secret),
                    }),
                };
            }
        }

        match &self.gameplay_state {
            // Allow new players to join
            GameplayState::Pregame => self.process_event_pregame(event),
            // Get bids from all players
            GameplayState::Bid => self.process_event_bid(event),
            // Play cards starting with after dealer
            // Get winner once everyones played and start again with winner of round
            GameplayState::Play(ps) => self.process_event_play(event),
            GameplayState::PostRound => self.process_event_postround(event),
            GameplayState::PostHand(ps) => {
                if event.message.action == GameAction::Ack
                    || event.message.action == GameAction::Deal
                {
                    self.start_next_hand();
                    self.update_to_next_state();
                }
            }
        };

        return GameEventResult {
            dest: Destination::Lobby(self.lobby_code.clone()),
            msg: crate::GameActionResponse::GameState(self.get_state()),
        };
    }

    pub fn encrypt_player_hand(&mut self, player_id: &String) {
        let player = self.players.get_mut(player_id).unwrap();
        let hand = player.hand.clone();
        let plaintext_hand = json!(hand).to_string();
        let player_secret = self.players_secrets.get(player_id).unwrap();
        let encoded = xor_encrypt_decrypt(&plaintext_hand, player_secret);
        let secret_data = BASE64.encode(&encoded);

        player.encrypted_hand = secret_data;
    }

    pub fn decrypt_player_hand(hand: String, player_secret: &String) -> Vec<Card> {
        if player_secret.is_empty() {
            error!("Player secret is empty");
            return vec![];
        }

        if hand.is_empty() {
            info!("Hand is empty");
            return vec![];
        }
        let hand = BASE64.decode(hand.as_bytes()).unwrap();
        let str_hand = String::from_utf8(hand).unwrap();
        // println!("Decrypting hand: {:?}", str_hand);
        let secret_data = xor_encrypt_decrypt(&str_hand, player_secret);
        // let str_hand2 = String::from_utf8(secret_data.clone()).unwrap();
        // println!("Decrypting hand: {:?}", str_hand2);
        let actual_hand: Vec<Card> = serde_json::from_slice(&secret_data).unwrap();
        actual_hand
    }

    pub fn get_state(&mut self) -> Self {
        // for player in self.player_order.iter() {
        //     // let hand = player.hand.clone();
        //     // let plaintext_hand = json!(hand).to_string();
        //     // let player_secret = self.players_secrets.get(key).unwrap();
        //     // let encoded = xor_encrypt_decrypt(&plaintext_hand, player_secret);
        //     // let secret_data = BASE64.encode(&encoded);

        //     // player.encrypted_hand = secret_data;
        //     // // player.nonce = nonce.to_vec();
        //     // self.encrypt_hand(&key);
        //     self.encrypt_hand(player);
        // }

        self.clone()
    }

    pub fn add_player(&mut self, player_id: String, role: PlayerRole, ip: String) -> String {
        info!("Adding player: {}", player_id);
        let client_secret = format!("sky_{}", nanoid_gen(12));

        self.players_secrets
            .insert(player_id.clone(), client_secret.clone());

        self.players
            .insert(player_id.clone(), GameClient::new(player_id, role, ip));
        return client_secret;
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
        // person who won the hand plays first next hand
        self.set_curr_player_turn(&winner);
    }

    pub fn set_curr_player_turn(&mut self, next_player: &String) {
        let next_idx = 0;
        for (i, player) in self.player_order.iter().enumerate() {
            if player == next_player {
                self.curr_player_turn_idx = i;
                self.curr_player_turn = Some(next_player.clone());
                return;
            }
        }
    }

    pub fn start_next_hand(&mut self) {
        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
    }

    pub fn start_next_round(&mut self) {
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
        // self.advance_dealer();

        let (next_turn_idx, next_player) =
            self.advance_turn(self.curr_player_turn_idx, &self.player_order);
        self.curr_player_turn_idx = next_turn_idx;
        self.curr_player_turn = Some(next_player);

        let (next_turn_idx, next_player) =
            self.advance_turn(self.curr_dealer_idx, &self.player_order);
        self.curr_dealer_idx = next_turn_idx;
        self.curr_dealer = next_player;

        self.curr_round += 1;

        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
        self.bid_order = vec![];
        self.deal();
        self.update_to_next_state();

        tracing::info!("Player status: {:#?}", self.player_status());
    }

    fn advance_turn(&self, curr_turn_idx: usize, player_order: &Vec<String>) -> (usize, String) {
        let mut next_player_idx = 0;
        if curr_turn_idx == player_order.len() - 1 {
            next_player_idx = 0;
        } else {
            next_player_idx += 1;
        }

        (next_player_idx, player_order[next_player_idx].clone())
    }

    pub fn broadcast_message(&mut self, message: String) {
        self.system_status.push(message);
    }

    pub fn setup_game(&mut self, sgo: SetupGameOptions) -> Result<(), GameError> {
        self.setup_game_options = sgo;
        if self.players.len() <= 1 {
            // Should maybe send a better message
            // self.system_status.push("Not enough players".into());
            self.broadcast_message("Not enough players".to_string());
            return Err(GameError::NotEnoughPlayers);
        }

        let player_ids: Vec<String> = self.players.keys().cloned().collect::<Vec<String>>();

        self.player_order = player_ids;

        if self.setup_game_options.deterministic {
            self.player_order.sort();
        }

        let mut deal_play_order: Vec<String> = self.player_order.to_vec();

        if !self.setup_game_options.deterministic {
            fastrand::shuffle(&mut deal_play_order);
        }

        self.player_order = deal_play_order;
        self.curr_dealer_idx = 0;
        self.curr_player_turn_idx = 1;

        self.curr_dealer = self.player_order.get(self.curr_dealer_idx).unwrap().clone();
        // person after dealer
        self.curr_player_turn = Some(
            self.player_order
                .get(self.curr_player_turn_idx)
                .unwrap()
                .clone(),
        );

        self.player_order.iter().for_each(|id| {
            // self.bids.insert(id.clone(), 0);
            self.wins.insert(id.clone(), 0);
            self.score.insert(id.clone(), 0);
        });

        let num_players = self.players.len() as i32;

        self.curr_round = if self.setup_game_options.start_round.is_some() {
            self.setup_game_options
                .start_round
                .unwrap()
                .try_into()
                .unwrap()
        } else {
            1
        };

        self.deal();
        self.update_to_next_state();

        tracing::info!(
            "Players: {}\nSettings: {:?}",
            num_players,
            self.setup_game_options
        );
        Ok(())
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

        if !self.setup_game_options.deterministic {
            fastrand::shuffle(&mut self.deck);
        }

        for i in 1..=self.curr_round {
            // get random card, give to a player
            for player_id in self.player_order.iter() {
                let card = self.deck.pop().unwrap();
                let player: &mut GameClient = self.players.get_mut(player_id).unwrap();

                let mut new_card = card.clone();
                new_card.played_by = Some(player.id.clone());
                player.hand.push(new_card);
            }
        }

        let players = self.player_order.clone();
        players.into_iter().for_each(|player_id| {
            self.encrypt_player_hand(&player_id);
        });
    }

    fn player_status(&self) {
        // tracing::info!("{:?}", self.players);
        tracing::info!("Score:\n{:?}", self.score);
    }

    fn is_bidding_over(&self) -> bool {
        // check if everyone has a bid
        return self.bids.keys().len() == self.players.len();
    }

    // fn advance_dealer(&mut self) -> Result<String, GameError> {
    //     for (i, player) in self.player_order.iter().enumerate() {
    //         if player.eq(&self.curr_dealer) {
    //             let nextdealer = match self.player_order.get(i + 1) {
    //                 Some(x) => x,
    //                 None => &self.player_order[0],
    //             };
    //             self.curr_dealer = nextdealer.clone();
    //             return Ok(self.curr_dealer.clone());
    //         }
    //     }
    //     return Err(GameError::InternalIssue(String::from(
    //         "Could not advance dealer",
    //     )));
    // }

    pub fn get_hand_from_encrypted(encrypted_hand: String, secret_key: &String) -> Vec<Card> {
        let hand = BASE64.decode(encrypted_hand.as_bytes()).unwrap();
        let str_hand = String::from_utf8(hand).unwrap();
        let secret_data = xor_encrypt_decrypt(&str_hand, secret_key);
        let actual_hand: Vec<Card> = serde_json::from_slice(&secret_data).unwrap();
        actual_hand
    }

    pub fn new(lobby_code: String) -> GameState {
        // let (tx, rx) = broadcast::channel(10);

        GameState {
            lobby_code: lobby_code,
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
            gameplay_state: GameplayState::Pregame,

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
            curr_player_turn_idx: 0,
            curr_dealer_idx: 0,
            secret_key: "mysecretkey".to_string(),
            setup_game_options: SetupGameOptions::new(),
            is_public: true,
            latest_update: Utc::now(), // tx,
                                       // rx,
        }
    }
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

#[derive(Debug)]
pub enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
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

fn xor_encrypt_decrypt(data: &str, key: &str) -> Vec<u8> {
    data.as_bytes()
        .iter()
        .zip(key.as_bytes().iter().cycle())
        .map(|(d, k)| d ^ k)
        .collect()
}

mod tests {
    use chrono::Utc;

    use crate::{
        game::find_winning_card, Card, GameMessage, GameState, GameplayState, PlayState,
        PlayerRole, SetupGameOptions, Suit,
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
    fn test_decrypt_hand() {
        let hand =
            "KBBbNhdRSlwFT0MDGVQDEhc0GyZRSVIHURQ+AxlUAxIBSVV9AAYZHRZZQwAFVB4SUUdbKRIfBQwWWVBHCBkBVRoPW2VBRlxLRA8AChBRJRUKSUN9HRYHNkQPAAoQR1hbURgMNgdRSktQCgAeGlseVV9JDz4fBhVLDlJVDllOWB4XSUNqQ19SGVgCGBYRahgOUVFbMRYELxlYAhgWBxdWVQAeECtRSVIaRAIFFlcZWAESBww6UUlBWklPGlEcUVhNQV9VfQMfERBRBz4RDBdAVR0ODgADHxEQURFDX1dGDx4HSUN9FxoRBFsNBVFZFwwWHx4cfUlCQxQYGEMaERdAQ0pHWy8fEgkMUDwDClcPWBkWHCYvHxIJDEZBTVEGQBMDUVFbLAMSFAwWT0MFFFkPElFRSG0OLg==".to_string();
        let secret = "sky_sspi4casu5zw";

        let decrypted_hand = GameState::decrypt_player_hand(hand, &secret.to_string());
        println!("Decrypted: {:?}", decrypted_hand);
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
    fn test_advance_player_turn() {
        let game = GameState::new();
        let players = vec![
            "P1".to_string(),
            "P2".to_string(),
            "P3".to_string(),
            "P4".to_string(),
        ];

        let curr = 0;
        let (i, res) = game.advance_turn(curr, &players);

        assert!(res == "P2".to_string());
        assert!(i == 1);
    }
    #[test]
    fn test_advance_player_turn_end_player() {
        let game = GameState::new();
        let players = vec![
            "P1".to_string(),
            "P2".to_string(),
            "P3".to_string(),
            "P4".to_string(),
        ];

        let curr = 3;
        let (i, res) = game.advance_turn(curr, &players);

        assert!(res == "P1".to_string());
        assert!(i == 0);
    }

    #[test]
    fn test_game_setup_and_round_end() {
        let PLAYER_ONE = "p1".to_string();
        let PLAYER_TWO = "p2".to_string();

        let mut game = GameState::new();
        game.add_player(PLAYER_ONE.clone(), PlayerRole::Leader);
        game.add_player(PLAYER_TWO.clone(), PlayerRole::Player);

        game.process_event(vec![GameMessage {
            username: PLAYER_ONE.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::StartGame(SetupGameOptions::from(5, true, Some(1))),
                // origin: crate::Actioner::Player(PLAYER_ONE.clone()),
            },
            timestamp: Utc::now(),
        }]);

        let first_dealer = game.curr_dealer.clone();
        let has_first_turn = game.player_order[1].clone(); // person after dealer
        let has_second_turn = game.player_order[0].clone(); // dealer goes second

        assert_ne!(has_first_turn, has_second_turn);
        assert_eq!(first_dealer, has_second_turn); // first dealer goes second
        assert_eq!(game.curr_player_turn.clone().unwrap(), has_first_turn);
        assert_eq!(game.gameplay_state, GameplayState::Bid);

        game.process_event(vec![GameMessage {
            username: has_first_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::Bid(0),
            },
            timestamp: Utc::now(),
        }]);

        assert_eq!(game.bids[&has_first_turn], 0);
        assert_eq!(game.curr_player_turn.clone().unwrap(), has_second_turn);

        game.process_event(vec![GameMessage {
            username: has_second_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::Bid(0),
                // origin: crate::Actioner::Player(has_second_turn.clone()),
            },
            timestamp: Utc::now(),
        }]);
        assert_eq!(game.bids[&has_second_turn], 0);

        // insta::assert_yaml_snapshot!(game, {
        //     ".timestamp" => "[utc]",
        //     ".players.*.encrypted_hand" => "[encrypted_hand]",
        //     ".event_log[].timestamp" => "[event_timestamp]",
        //     ".wins" => insta::sorted_redaction(),
        //     ".bids" => insta::sorted_redaction(),
        //     ".score" => insta::sorted_redaction(),
        //     ".players" => insta::sorted_redaction(),
        // });

        // first player that bid 0 goes first because both bid 0
        assert_eq!(game.curr_player_turn.clone().unwrap(), has_first_turn);
        assert_eq!(
            game.gameplay_state,
            GameplayState::Play(crate::PlayState::new())
        );

        // Time to play
        let p1_card = game
            .players
            .get(&has_first_turn)
            .unwrap()
            .hand
            .first()
            .unwrap()
            .clone();
        let p2_card = game
            .players
            .get(&has_second_turn)
            .unwrap()
            .hand
            .first()
            .unwrap()
            .clone();

        assert_eq!(game.curr_player_turn.clone().unwrap(), has_first_turn);
        game.process_event(vec![GameMessage {
            username: has_first_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::PlayCard(p1_card.clone()),
                // origin: crate::Actioner::Player(has_first_turn.clone()),
            },
            timestamp: Utc::now(),
        }]);

        assert_eq!(game.curr_player_turn.clone().unwrap(), has_second_turn);
        assert_eq!(game.curr_played_cards.len(), 1);
        assert_eq!(
            *game.curr_played_cards.first().clone().unwrap(),
            p1_card.clone()
        );
        assert_eq!(game.gameplay_state, GameplayState::Play(PlayState::from(1)));

        game.process_event(vec![GameMessage {
            username: has_second_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::PlayCard(p2_card.clone()),
                // origin: crate::Actioner::Player(has_second_turn.clone()),
            },
            timestamp: Utc::now(),
        }]);

        assert_eq!(
            game.gameplay_state,
            GameplayState::PostHand(PlayState::from(1))
        );
        assert_eq!(game.curr_played_cards.len(), 2);

        // Send "start next round" message
        game.process_event(vec![GameMessage {
            username: has_first_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::Ack,
                // origin: crate::Actioner::Player(has_first_turn.clone()),
            },
            timestamp: Utc::now(),
        }]);

        assert_eq!(game.gameplay_state, GameplayState::PostRound);

        game.process_event(vec![GameMessage {
            username: has_first_turn.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::Deal,
                // origin: crate::Actioner::Player(has_first_turn.clone()),
            },
            timestamp: Utc::now(),
        }]);

        // insta::assert_yaml_snapshot!(game, {
        //     ".timestamp" => "[utc]",
        //     ".players.*.encrypted_hand" => "[encrypted_hand]",
        //     ".event_log[].timestamp" => "[event_timestamp]",
        //     ".wins" => insta::sorted_redaction(),
        //     ".bids" => insta::sorted_redaction(),
        //     ".score" => insta::sorted_redaction(),
        //     ".players" => insta::sorted_redaction(),
        // });

        assert_eq!(game.gameplay_state, GameplayState::Bid);
        assert_eq!(game.curr_played_cards.len(), 0);
        assert_eq!(has_first_turn, game.curr_dealer); // round 1 first player is now dealer
        assert_eq!(has_second_turn, game.curr_player_turn.clone().unwrap()); // round 1 second player is now going first
        assert_eq!(first_dealer, game.curr_player_turn.clone().unwrap()); // round 1 dealer goes first in round 2

        // insta::assert_yaml_snapshot!(game, {
        //     ".timestamp" => "[utc]",
        //     ".players.*.encrypted_hand" => "[encrypted_hand]",
        //     ".event_log[].timestamp" => "[event_timestamp]",
        //     ".wins" => insta::sorted_redaction(),
        //     ".bids" => insta::sorted_redaction(),
        //     ".score" => insta::sorted_redaction(),
        //     ".players" => insta::sorted_redaction(),
        // });
    }

    #[test]
    fn test_deck_creation() {
        println!("{}", serde_json::json!(create_deck()));
    }

    #[test]
    fn test_get_hand_from_encrypted() {
        let mut game = GameState::new();

        let res = GameState::get_hand_from_encrypted(
            "KBBbNg5WXlVATUQGGgZNVhc0GyZITkYGGUNKVAUSXUdRUVs7AxUJCB4FRFpUEVVfBg5bZVMJOQ=="
                .to_string(),
            &"sky_jtdgpafvvg43".to_string(),
        );

        assert!(res.len() == 1);
        assert!(res.first().is_some());
        assert_eq!(
            *res.first().unwrap(),
            Card {
                id: 20,
                suit: Suit::Diamond,
                value: 9,
                played_by: Some("ai".to_string())
            }
        );
        println!("Cards: {:?}", res);

        let res = GameState::get_hand_from_encrypted(
            "KBBbNg5WXlVATUQGGgZNVhc0GyZITkYGGUNKVAUSXUdRUVs7AxUJCB4FRFpUEVVfBg5bZVMJOQ=="
                .to_string(),
            &"sky_jtdgpafvvg43".to_string(),
        );
        println!("Cards: {:?}", res);
    }

    #[test]
    fn test_start_round3() {
        let player_one = "p1".to_string();
        let player_two = "p2".to_string();

        let mut game = GameState::new();
        game.add_player(player_one.clone(), PlayerRole::Leader);
        game.add_player(player_two.clone(), PlayerRole::Player);

        game.process_event(vec![GameMessage {
            username: player_one.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::StartGame(SetupGameOptions::from(5, true, Some(3))),
                // origin: crate::Actioner::Player(PLAYER_ONE.clone()),
            },
            timestamp: Utc::now(),
        }]);

        insta::assert_yaml_snapshot!(game, {
            ".timestamp" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        game.process_event(vec![
            GameMessage {
                username: player_two.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::Bid(0),
                },
                timestamp: Utc::now(),
            },
            GameMessage {
                username: player_one.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::Bid(0), // origin: crate::Actioner::Player(has_second_turn.clone()),
                },
                timestamp: Utc::now(),
            },
        ]);

        // two players play cards, go into post hand state
        game.process_event(vec![
            GameMessage {
                username: player_two.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::PlayCard(
                        game.players
                            .get(&player_two)
                            .unwrap()
                            .hand
                            .first()
                            .clone()
                            .unwrap()
                            .clone(),
                    ),
                    // origin: crate::Actioner::Player(has_second_turn.clone()),
                },
                timestamp: Utc::now(),
            },
            GameMessage {
                username: player_one.clone(),
                message: crate::GameEvent {
                    action: crate::GameAction::PlayCard(
                        game.players
                            .get(&player_one)
                            .unwrap()
                            .hand
                            .first()
                            .clone()
                            .unwrap()
                            .clone(),
                    ), // origin: crate::Actioner::Player(has_second_turn.clone()),
                },
                timestamp: Utc::now(),
            },
        ]);

        insta::assert_yaml_snapshot!(game, {
            ".timestamp" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        game.process_event(vec![GameMessage {
            username: player_two.clone(),
            message: crate::GameEvent {
                action: crate::GameAction::Ack,
            },
            timestamp: Utc::now(),
        }]);
    }
}
