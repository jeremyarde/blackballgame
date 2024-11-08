use std::collections::HashMap;

use chrono::Utc;
use data_encoding::BASE64;

use nanoid::nanoid_gen;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};

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
                if ps.hand_num
                    >= self
                        .curr_round
                        .try_into()
                        .expect("Could not convert round to usize")
                {
                    GameplayState::PostRound
                } else {
                    GameplayState::Play(PlayState::from(ps.hand_num + 1))
                }
            }
            GameplayState::PostRound => {
                info!(
                    "jere/ post round, maybe end game?? {} vs {}",
                    self.curr_round, self.setup_game_options.rounds
                );
                if self.curr_round >= self.setup_game_options.rounds as i32 {
                    GameplayState::End
                } else {
                    self.curr_played_cards = vec![];
                    self.curr_winning_card = None;
                    GameplayState::Bid
                }
            }
            GameplayState::End => {
                self.curr_played_cards = vec![];
                self.curr_winning_card = None;
                GameplayState::Pregame
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
        if let GameAction::StartGame(sgo) = event.action {
            let result = self.setup_game(sgo);
            info!("Setup game result: {:?}", result);
        }
    }

    fn is_correct_player_turn(&mut self, event: &GameMessage) -> bool {
        let curr_turn = &self.curr_player_turn.clone().unwrap_or("".to_string());

        if curr_turn != &event.username {
            info!("{}'s turn, not {}'s turn.", curr_turn, event.username);
            self.broadcast_message(format!(
                "{}'s turn, not {}'s turn.",
                curr_turn, event.username
            ));
            self.system_status.push(format!(
                "{}'s turn, not {}'s turn.",
                curr_turn, event.username
            ));
            return false;
        }
        true
    }

    pub fn process_event_bid(&mut self, event: GameMessage) {
        if !self.is_correct_player_turn(&event) {
            return;
        };

        if let GameAction::Bid(bid) = event.action {
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

        match event.action {
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

        if let GameAction::PlayCard(card) = &event.action {
            let player = self
                .players
                .get_mut(&player_id)
                .expect("Did not find player");

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
                    player
                        .hand
                        .remove(cardloc.expect("Did not find card location in hand"));

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
        match &event.action {
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
                        client_secret: self
                            .players
                            .get(username)
                            .expect("Failed to get player")
                            .details
                            .client_secret
                            .clone(),
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
                let secret = self.add_player(
                    player.username.clone(),
                    PlayerRole::Player,
                    player.ip.clone(),
                );
                return GameEventResult {
                    dest: Destination::User(
                        self.players
                            .get(&event.username)
                            .expect("Failed to get player")
                            .clone()
                            .details,
                    ),
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
                if event.action == GameAction::Ack || event.action == GameAction::Deal {
                    self.start_next_hand();
                    self.update_to_next_state();
                }
            }
            GameplayState::End => {
                if event.action == GameAction::Ack {
                    self.update_to_next_state();
                }
            }
        };

        let players = self
            .players
            .values()
            .map(|player| player.details.clone())
            .collect();

        return GameEventResult {
            dest: Destination::Lobby(players),
            msg: crate::GameActionResponse::GameState(self.get_state_for_lobby()),
        };
    }

    pub fn encrypt_player_hand(&mut self, player_id: &String) {
        let player = self
            .players
            .get_mut(player_id)
            .expect("Did not find player");
        let hand = player.hand.clone();
        let plaintext_hand = json!(hand).to_string();
        let player_secret = self
            .players_secrets
            .get(player_id)
            .expect("Did not find player secret");
        let encoded = xor_encrypt_decrypt(&plaintext_hand, player_secret);
        let secret_data = BASE64.encode(&encoded);

        player.encrypted_hand = secret_data;
    }

    pub fn decrypt_player_hand(hand: String, player_secret: &String) -> Vec<Card> {
        info!("Decrypting hand: {:?}, {:?}", hand, player_secret);
        if player_secret.is_empty() {
            error!("Player secret is empty");
            return vec![];
        }

        if hand.is_empty() {
            info!("Hand is empty");
            return vec![];
        }
        let hand = BASE64
            .decode(hand.as_bytes())
            .expect("Could not decode hand");
        let str_hand = String::from_utf8(hand).expect("Could not convert hand to string");
        let secret_data = xor_encrypt_decrypt(&str_hand, player_secret);
        // let str_hand2 = String::from_utf8(secret_data.clone()).expect();
        // println!("Decrypting hand: {:?}", str_hand2);
        let actual_hand: Vec<Card> =
            serde_json::from_slice(&secret_data).expect("Could not parse hand");
        actual_hand
    }

    pub fn get_state_for_lobby(&mut self) -> Self {
        let mut state_copy = self.clone();
        state_copy.deck = vec![];

        state_copy
    }

    pub fn add_player(&mut self, player_id: String, role: PlayerRole, ip: String) -> String {
        let client_secret = format!("sky_{}", nanoid_gen(12));

        self.players_secrets
            .insert(player_id.clone(), client_secret.clone());

        info!("Adding player: {}, {}", player_id, client_secret);
        self.players.insert(
            player_id.clone(),
            GameClient::new(player_id, role, ip, client_secret.clone()),
        );
        return client_secret;
    }

    pub fn end_hand(&mut self) {
        tracing::info!("End turn, trump={:?}, played cards:", self.trump);
        self.curr_played_cards
            .clone()
            .iter()
            .for_each(|c| tracing::info!("{}", c));

        let winner = self
            .curr_winning_card
            .clone()
            .expect("No winner")
            .played_by
            .expect("No winner");

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
            // let player = self.players.get_mut(player_id).expect();

            if self.wins.get(&player.id) == self.bids.get(&player.id) {
                let bidscore = self.bids.get(&player.id).expect("Did not find bid") + 10;
                let curr_score = self.score.get_mut(&player.id).expect("did not find score");
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

        self.curr_dealer = self
            .player_order
            .get(self.curr_dealer_idx)
            .expect("Did not find dealer")
            .clone();
        // person after dealer
        self.curr_player_turn = Some(
            self.player_order
                .get(self.curr_player_turn_idx)
                .expect("Did not find player turn")
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
                .expect("Could not get start round")
                .try_into()
                .expect("Could not convert start round to usize")
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
        let client = self
            .players
            .get_mut(&player_id)
            .expect("Did not find player");

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
                let card = self.deck.pop().expect("Could not get card");
                let player: &mut GameClient = self
                    .players
                    .get_mut(player_id)
                    .expect("Could not get player");

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

    pub fn get_hand_from_encrypted(encrypted_hand: String, secret_key: &String) -> Vec<Card> {
        let hand = BASE64
            .decode(encrypted_hand.as_bytes())
            .expect("Could not decode hand");
        let str_hand = String::from_utf8(hand).expect("Could not convert hand to string");
        let secret_data = xor_encrypt_decrypt(&str_hand, secret_key);
        let actual_hand: Vec<Card> =
            serde_json::from_slice(&secret_data).expect("Could not parse hand");
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

    let led_suit = played_cards
        .first()
        .expect("Could not get led suit")
        .suit
        .clone();
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

pub fn xor_encrypt_decrypt(data: &str, key: &str) -> Vec<u8> {
    data.as_bytes()
        .iter()
        .zip(key.as_bytes().iter().cycle())
        .map(|(d, k)| d ^ k)
        .collect()
}

mod tests {
    use chrono::Utc;

    use crate::{
        create_deck, game::find_winning_card, Card, GameAction, GameMessage, GameState,
        GameVisibility, GameplayState, PlayState, PlayerRole, SetupGameOptions, Suit,
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
    fn test_decrypt_hand_v2() {
        // INFO bb-admin/src/main.rs:685 Got game state: GameState { lobby_code: "a", setup_game_options:
        // SetupGameOptions { rounds: 4, deterministic: true, start_round: None, max_players: 4, game_mode: "Standard", visibility: Public, password: None },
        // secret_key: "mysecretkey", players: {"e": GameClient { id: "e", hand: [], encrypted_hand: "KBBbNhVHXklWRRAWDgVMUxc0GyZTX0YfTEUQFRcNQRRJSRozBAdGVkwfUwoXARcMQl8EAg==", num_cards: 0, role: Player, details: PlayerDetails { username: "e", ip: "127.0.0.1:49678", client_secret: "sky_qedzni2fbd56" } }, "a": GameClient { id: "a", hand: [], encrypted_hand: "KBBbNgxFD0ICFFVBDlYTFxc0GyZKXRcWERRVQhdeHlBJSQovCQNQVR8aAVAOQg9QSVpNIjU=", num_cards: 0, role: Player, details: PlayerDetails { username: "a", ip: "127.0.0.1:49684", client_secret: "sky_hg5w38w1b7jr" } }}, players_secrets: {}, deck: [], curr_round: 1, trump: Heart, player_order: ["a", "e"], curr_played_cards: [], curr_player_turn: Some("e"), curr_player_turn_idx: 0, curr_winning_card: None, curr_dealer: "a", curr_dealer_idx: 0, bids: {}, bid_order: [], wins: {"e": 0, "a": 0}, score: {"a": 0, "e": 0}, gameplay_state: Bid, event_log: [], system_status: [], is_public: true, latest_update: 2024-11-03T23:55:39.023714Z }

        let hand = "KBBbNgxFD0ICFFVBDlYTFxc0GyZKXRcWERRVQhdeHlBJSQovCQNQVR8aAVAOQg9QSVpNIjU=";
        // let hand = "KBBbNhVHXklWRRAWDgVMUxc0GyZTX0YfTEUQFRcNQRRJSRozBAdGVkwfUwoXARcMQl8EAg==";
        let secret = "sky_hg5w38w1b7jr";

        let decrypted_hand = GameState::decrypt_player_hand(hand.to_string(), &secret.to_string());
        println!("Decrypted: {:?}", decrypted_hand);
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
        let game = GameState::new("lobby".to_string());
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
        let game = GameState::new("lobby".to_string());
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
    fn test_bids_cant_equal_round() {
        let PLAYER_ONE = "p1".to_string();
        let PLAYER_TWO = "p2".to_string();

        let mut game = GameState::new("lobby".to_string());
        game.add_player(PLAYER_ONE.clone(), PlayerRole::Leader, "ip".to_string());
        game.add_player(PLAYER_TWO.clone(), PlayerRole::Player, "ip".to_string());

        game.process_event(GameMessage {
            username: PLAYER_ONE.clone(),
            action: crate::GameAction::StartGame(SetupGameOptions::from(
                5,
                true,
                Some(3),
                4,
                "Standard".to_string(),
                GameVisibility::Public,
                None,
            )),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        let firstplayer = game.curr_player_turn.clone().expect("No player turn");
        game.process_event(GameMessage {
            username: game.curr_player_turn.clone().expect("No player turn"),
            action: GameAction::Bid(4),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        assert!(game
            .bids
            .get(&game.curr_player_turn.clone().expect("No player turn"))
            .is_none());

        // should be able to bid the round number
        game.process_event(GameMessage {
            username: game.curr_player_turn.clone().expect("No player turn"),
            action: GameAction::Bid(3),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        println!("Bids: {:?}", game.bids);
        assert_eq!(game.bids.get(&firstplayer).clone(), Some(&3));

        // curr player turn updates, and we should not be able to bid 0
        let secondplayer = game.curr_player_turn.clone().expect("No player turn");
        game.process_event(GameMessage {
            username: secondplayer.clone(),
            action: GameAction::Bid(0),
            lobby: "lobby".to_string(),
            timestamp: Utc::now(),
        });
        assert!(game.bids.get(&secondplayer).is_none());

        game.process_event(GameMessage {
            username: secondplayer.clone(),
            action: GameAction::Bid(1),
            lobby: "lobby".to_string(),
            timestamp: Utc::now(),
        });
        assert_eq!(game.bids.get(&secondplayer).clone(), Some(&1));
    }

    #[test]
    fn test_game_setup_and_round_end() {
        let PLAYER_ONE = "p1".to_string();
        let PLAYER_TWO = "p2".to_string();

        let mut game = GameState::new("lobby".to_string());
        game.add_player(PLAYER_ONE.clone(), PlayerRole::Leader, "ip".to_string());
        game.add_player(PLAYER_TWO.clone(), PlayerRole::Player, "ip".to_string());

        game.process_event(GameMessage {
            username: PLAYER_ONE.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::StartGame(SetupGameOptions::from(
                5,
                true,
                Some(1),
                4,
                "Standard".to_string(),
                GameVisibility::Public,
                None,
            )),
            // origin: crate::Actioner::Player(PLAYER_ONE.clone()),
            timestamp: Utc::now(),
        });

        let first_dealer = game.curr_dealer.clone();
        let has_first_turn = game.player_order[1].clone(); // person after dealer
        let has_second_turn = game.player_order[0].clone(); // dealer goes second

        assert_ne!(has_first_turn, has_second_turn);
        assert_eq!(first_dealer, has_second_turn); // first dealer goes second
        assert_eq!(
            game.curr_player_turn.clone().expect("No player turn"),
            has_first_turn
        );
        assert_eq!(game.gameplay_state, GameplayState::Bid);

        game.process_event(GameMessage {
            username: has_first_turn.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Bid(0),

            timestamp: Utc::now(),
        });

        assert_eq!(game.bids[&has_first_turn], 0);
        assert_eq!(
            game.curr_player_turn.clone().expect("No player turn"),
            has_second_turn
        );

        game.process_event(GameMessage {
            username: has_second_turn.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Bid(0),
            // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
        });
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
        assert_eq!(
            game.curr_player_turn.clone().expect("No player turn"),
            has_first_turn
        );
        assert_eq!(
            game.gameplay_state,
            GameplayState::Play(crate::PlayState::new())
        );

        // Time to play
        let p1_card = game
            .players
            .get(&has_first_turn)
            .expect("Did not find player")
            .hand
            .first()
            .expect("Could not get first card")
            .clone();
        let p2_card = game
            .players
            .get(&has_second_turn)
            .expect("Did not find player")
            .hand
            .first()
            .expect("Could not get first card")
            .clone();

        assert_eq!(
            game.curr_player_turn.clone().expect("No player turn"),
            has_first_turn
        );
        game.process_event(GameMessage {
            username: has_first_turn.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(p1_card.clone()),
            // origin: crate::Actioner::Player(has_first_turn.clone()),
            timestamp: Utc::now(),
        });

        assert_eq!(
            game.curr_player_turn.clone().expect("No player turn"),
            has_second_turn
        );
        assert_eq!(game.curr_played_cards.len(), 1);
        assert_eq!(
            *game
                .curr_played_cards
                .first()
                .clone()
                .expect("Could not get first card"),
            p1_card.clone()
        );
        assert_eq!(game.gameplay_state, GameplayState::Play(PlayState::from(1)));

        game.process_event(GameMessage {
            username: has_second_turn.clone(),
            action: crate::GameAction::PlayCard(p2_card.clone()),
            // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        assert_eq!(
            game.gameplay_state,
            GameplayState::PostHand(PlayState::from(1))
        );
        assert_eq!(game.curr_played_cards.len(), 2);

        // Send "start next round" message
        game.process_event(GameMessage {
            username: has_first_turn.clone(),
            action: crate::GameAction::Ack,
            // origin: crate::Actioner::Player(has_first_turn.clone()),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        assert_eq!(game.gameplay_state, GameplayState::PostRound);

        game.process_event(GameMessage {
            username: has_first_turn.clone(),
            action: crate::GameAction::Deal,
            // origin: crate::Actioner::Player(has_first_turn.clone()),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

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
        assert_eq!(
            has_second_turn,
            game.curr_player_turn.clone().expect("No player turn")
        ); // round 1 second player is now going first
        assert_eq!(
            first_dealer,
            game.curr_player_turn.clone().expect("No player turn")
        ); // round 1 dealer goes first in round 2

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
        let mut game = GameState::new("lobby".to_string());

        let res = GameState::get_hand_from_encrypted(
            "KBBbNg5WXlVATUQGGgZNVhc0GyZITkYGGUNKVAUSXUdRUVs7AxUJCB4FRFpUEVVfBg5bZVMJOQ=="
                .to_string(),
            &"sky_jtdgpafvvg43".to_string(),
        );

        assert!(res.len() == 1);
        assert!(res.first().is_some());
        assert_eq!(
            *res.first().expect("Could not get first card"),
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

        let mut game = GameState::new("lobby".to_string());
        game.add_player(player_one.clone(), PlayerRole::Leader, "ip".to_string());
        game.add_player(player_two.clone(), PlayerRole::Player, "ip".to_string());

        game.process_event(GameMessage {
            username: player_one.clone(),
            action: crate::GameAction::StartGame(SetupGameOptions::from(
                5,
                true,
                Some(3),
                4,
                "Standard".to_string(),
                GameVisibility::Public,
                None,
            )),
            lobby: "lobby".to_string(),
            // origin: crate::Actioner::Player(PLAYER_ONE.clone()),
            timestamp: Utc::now(),
        });

        insta::assert_yaml_snapshot!(game, {
            ".timestamp" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        game.process_event(GameMessage {
            username: player_two.clone(),
            action: crate::GameAction::Bid(0),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });
        game.process_event(GameMessage {
            username: player_one.clone(),
            action: crate::GameAction::Bid(0), // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        // two players play cards, go into post hand state
        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(
                game.players
                    .get(&player_two)
                    .expect("Did not find player")
                    .hand
                    .first()
                    .clone()
                    .expect("Could not get first card")
                    .clone(),
            ),
            // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
        });
        game.process_event(GameMessage {
            username: player_one.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(
                game.players
                    .get(&player_one)
                    .expect("Did not find player")
                    .hand
                    .first()
                    .clone()
                    .expect("Could not get first card")
                    .clone(),
            ), // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
        });

        insta::assert_yaml_snapshot!(game, {
            ".timestamp" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Ack,
            timestamp: Utc::now(),
        });
    }
}
