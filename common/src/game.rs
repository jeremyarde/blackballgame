use std::collections::HashMap;

use chrono::Utc;
// use common::{Destination, GameClient, GameEventResult, GameState, PlayerRole};
use data_encoding::BASE64;
use nanoid::nanoid_gen;
use serde_json::json;
use tracing::{error, info};

// use crate::{
//     create_deck, Card, Connect, Destination, GameAction, GameClient, GameError, GameEventResult,
//     GameMessage, GameState, GameplayState, PlayState, PlayerDetails, PlayerRole, SetupGameOptions,
//     Suit,
// };

use crate::{
    ai, create_deck, Card, Connect, Destination, GameAction, GameActionResponse, GameClient,
    GameError, GameEventResult, GameMessage, GameState, GameplayState, PlayState, PlayerDetails,
    PlayerRole, SetupGameOptions, Suit,
};

pub fn xor_encrypt_decrypt(data: &str, key: &str) -> Vec<u8> {
    data.as_bytes()
        .iter()
        .zip(key.as_bytes().iter().cycle())
        .map(|(d, k)| d ^ k)
        .collect()
}

impl GameState {
    pub fn get_dealer(&self) -> String {
        self.player_order[self.curr_dealer_idx].clone()
    }

    pub fn update_to_next_state(&mut self) {
        let newstate = match &self.gameplay_state {
            GameplayState::Bid => GameplayState::Play(PlayState::new()),
            GameplayState::Pregame => GameplayState::Bid,
            GameplayState::Play(ps) => {
                // move to new "hand" in the round when each player played a card
                if self.curr_played_cards.len() == self.player_order.len() {
                    GameplayState::PostHand(ps.clone())
                } else {
                    self.gameplay_state.clone()
                }
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
                if self.curr_round > self.setup_game_options.rounds as i32 {
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
        self.gameplay_state = newstate;
    }

    pub fn process_event_pregame(&mut self, event: GameMessage) -> Option<GameEventResult> {
        match event.action {
            GameAction::StartGame(sgo) => {
                let result = self.setup_game(sgo);
                info!("Setup game result: {:?}", result);
            }
            GameAction::Connect(player_details) => {
                let secret = self.add_player(
                    player_details.username.clone(),
                    PlayerRole::Player,
                    player_details.ip.clone().unwrap(),
                );
                return Some(GameEventResult {
                    dest: Destination::User(PlayerDetails {
                        username: event.username.clone(),
                        ip: player_details.ip.clone(),
                        client_secret: self
                            .players
                            .get(&player_details.username.clone())
                            .expect("Failed to get player")
                            .details
                            .client_secret
                            .clone(),
                        lobby: player_details.lobby.clone(),
                    }),
                    msg: crate::GameActionResponse::Connect(Connect {
                        username: event.username.clone(),
                        channel: self.lobby_code.clone(),
                        secret: Some(secret),
                    }),
                });
            }
            GameAction::JoinGame(player) => {
                let secret = self.add_player(
                    player.username.clone(),
                    PlayerRole::Player,
                    player.ip.clone().unwrap(),
                );
                return Some(GameEventResult {
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
                });
            }
            _ => {}
        };

        None
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

    pub fn process_event_bid(&mut self, event: GameMessage) -> Option<GameEventResult> {
        if !self.is_correct_player_turn(&event) {
            return None;
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
                let mut curr_highest_bid = self.player_bids[0].clone();
                for (player, bid) in self.player_bids.iter() {
                    if bid > &curr_highest_bid.1 {
                        curr_highest_bid = (player.to_string(), *bid);
                    }
                }
                self.set_curr_player_turn(&curr_highest_bid.0);
                self.update_to_next_state();
            }
        }

        None
    }

    pub fn process_event_postround(&mut self, event: GameMessage) -> Option<GameEventResult> {
        match event.action {
            GameAction::Deal | GameAction::Ack => {
                self.start_next_round();
            }
            _ => {}
        }
        None
    }

    pub fn process_event_play(&mut self, event: GameMessage) -> Option<GameEventResult> {
        if !self.is_correct_player_turn(&event) {
            return None;
        };
        let player_id = event.username.clone();

        if let GameAction::PlayCard(card) = &event.action {
            match &self.is_played_card_valid(player_id.clone(), card.clone()) {
                Ok(x) => {
                    tracing::info!("card is valid");
                    if x.suit == self.trump {
                        self.trump_played_in_round = true;
                    }
                    // remove the card from the players hand
                    let mut cardloc: Option<usize> = None;
                    let player = self
                        .players
                        .get_mut(&player_id)
                        .expect("Did not find player");
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

        None
    }

    pub fn process_event(&mut self, event: GameMessage) -> GameEventResult {
        self.event_log.push(event.clone());
        self.updated_at = Utc::now();
        self.system_status.clear(); // clear system status on every event, because we only want to show the current player the last error

        info!("Processing event: {:?}", event);
        let has_result = match &self.gameplay_state {
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
                None
            }
            GameplayState::End => {
                if event.action == GameAction::Ack {
                    self.update_to_next_state();
                }
                None
            }
        };

        if self.curr_player_turn.is_some()
            && self
                .players
                .get(self.curr_player_turn.as_ref().unwrap())
                .unwrap()
                .role
                == PlayerRole::Computer
        {
            // self.process_event(event)
            let comp_player = self
                .players
                .get(self.curr_player_turn.as_ref().unwrap())
                .unwrap();
            let action = ai::decide_action(
                self,
                self.curr_player_turn.clone().unwrap().to_string(),
                comp_player.details.client_secret.clone().unwrap(),
            );
            info!("AI chose an action: {:?}", action);
            if action.is_some() {
                self.process_event(GameMessage {
                    username: self.curr_player_turn.clone().unwrap().to_string(),
                    action: action.unwrap().clone(),
                    timestamp: chrono::Utc::now(),
                    lobby: self.lobby_code.clone(),
                });
            }
        }

        if let Some(result) = has_result {
            return result;
        }

        let players = self
            .players
            .values()
            .map(|player| player.details.clone())
            .collect();

        GameEventResult {
            dest: Destination::Lobby(players),
            msg: GameActionResponse::GameState((self.get_state_for_lobby())),
        }
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

    // pub fn decrypt_player_hand(hand: String, player_secret: &String) -> Vec<Card> {
    //     info!("Decrypting hand: {:?}, {:?}", hand, player_secret);
    //     if player_secret.is_empty() {
    //         error!("Player secret is empty");
    //         return vec![];
    //     }

    //     if hand.is_empty() {
    //         info!("Hand is empty");
    //         return vec![];
    //     }
    //     let hand = BASE64
    //         .decode(hand.as_bytes())
    //         .expect("Could not decode hand");
    //     let str_hand = String::from_utf8(hand).expect("Could not convert hand to string");
    //     let secret_data = xor_encrypt_decrypt(&str_hand, player_secret);
    //     let actual_hand: Vec<Card> =
    //         serde_json::from_slice(&secret_data).expect("Could not parse hand");
    //     actual_hand
    // }

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
            GameClient::new(
                player_id,
                role,
                ip,
                client_secret.clone(),
                self.lobby_code.clone(),
            ),
        );
        client_secret
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

            if self.bids.get(&player.id).is_some()
                && self.wins.get(&player.id) == self.bids.get(&player.id).unwrap().as_ref()
            {
                let bidscore = self.bids.get(&player.id).unwrap().unwrap() + 10;
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

        // change dealers to next player
        let (next_turn_idx, next_player) =
            self.advance_turn(self.curr_dealer_idx, &self.player_order);
        self.curr_dealer_idx = next_turn_idx;
        self.curr_dealer = next_player;

        // upcoming player to bid is the player after the dealer
        let (next_turn_idx, next_player) =
            self.advance_turn(self.curr_dealer_idx, &self.player_order);
        self.curr_player_turn_idx = next_turn_idx;
        self.curr_player_turn = Some(next_player);

        self.curr_round += 1;
        self.curr_played_cards = vec![];
        self.curr_winning_card = None;
        self.player_bids = vec![];
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
        // add the computer players
        for i in 0..self.setup_game_options.computer_players {
            self.add_player(
                format!("computer_{}", i),
                PlayerRole::Computer,
                "0.0.0.0:0".to_string(),
            );
        }

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
                self.bids.insert(client.id.clone(), Some(x));
                self.player_bids.push((client.id.clone(), x));
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

    pub fn new(lobby_code: String) -> GameState {
        // let (tx, rx) = broadcast::channel(10);

        GameState {
            lobby_code,
            players: HashMap::new(),
            deck: create_deck(),
            curr_round: 1,
            trump: Suit::Heart,
            player_order: vec![],
            // play_order: vec![],
            bids: HashMap::new(),
            player_bids: Vec::new(),
            wins: HashMap::new(),
            score: HashMap::new(),
            gameplay_state: GameplayState::Pregame,
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
            updated_at: Utc::now(),
            created_at: Utc::now(),
            trump_played_in_round: false,
        }
    }

    fn is_played_card_valid(
        &self,
        // played_cards: &Vec<Card>,
        // hand: Vec<Card>,
        player_id: String,
        played_card: Card,
    ) -> Result<Card, PlayedCardError> {
        // rules for figuring out if you can play a card:
        // 1. must follow suit if available
        // 2. can't play trump to start a round unless that is all the player has
        let playerhand = &self
            .players
            .get(&player_id)
            .expect("Did not find player")
            .hand;

        if self.curr_played_cards.is_empty() {
            if played_card.suit == self.trump && self.trump_played_in_round == false {
                // all cards in hand must be trump
                for c in playerhand {
                    if c.suit != self.trump {
                        return Err(PlayedCardError::CantUseTrump);
                    }
                }
                return Ok(played_card.clone());
            } else {
                return Ok(played_card.clone());
            }
        }

        let led_suit = self
            .curr_played_cards
            .first()
            .expect("Could not get led suit")
            .suit
            .clone();
        if led_suit != played_card.suit {
            // make sure player does not have that suit
            for c in playerhand {
                if c.suit == led_suit {
                    return Err(PlayedCardError::DidNotFollowSuit);
                }
            }
        }
        Ok(played_card.clone())
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
    curr_bids: &HashMap<String, Option<i32>>,
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
    let bid_sum = curr_bids.values().map(|x| x.unwrap()).sum::<i32>();
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

// pub fn xor_encrypt_decrypt(data: &str, key: &str) -> Vec<u8> {
//     data.as_bytes()
//         .iter()
//         .zip(key.as_bytes().iter().cycle())
//         .map(|(d, k)| d ^ k)
//         .collect()
// }

mod tests {
    use std::collections::HashMap;

    use crate::{
        create_deck, game::find_winning_card, Card, GameAction, GameMessage, GameState,
        GameVisibility, GameplayState, PlayState, PlayerRole, SetupGameOptions, Suit,
    };
    use chrono::Utc;

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

        let hand =
            "KBBbNhxKVl4GFEcCAAkUAhc0GyZaUk4bW1kcFx5ZT0tRGAw2DEpWSURIBBYJSkFFBQoVKh1KVloDRTg=";
        let secret = "sky_xhlk78erlhmg";

        // let hand = "KBBbNhxQXF5TTxsKCA8fFhc0GyZaSEQeDQUbVkYdExoHSUN9Gx4TD0lPGwwFAhMWUVFIawUv";
        // let secret = "sky_xrfmkc9zdnfs";

        // let hand = "KBBbNh5LU0NBShMZDw1MFxc0GyZYU0sBDgNDS09ORgcaH1tlWAoFBRtEHUsVDVkHFklDbk4UNA==";
        // let secret = "sky_5d7hjenh3c2k";

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
    fn test_validate_bid() {
        let mut game: GameState = serde_json::from_str(
            "{\"bid_order\":[[\"123\",0],[\"player2\",1]],\"bids\":{\"123\":0,\"player2\":1},\"created_at\":\"2024-11-11T21:40:29.040459Z\",\"curr_dealer\":\"player2\",\"curr_played_cards\":[],\"curr_player_turn\":\"player2\",\"curr_round\":3,\"curr_winning_card\":null,\"event_log\":[{\"action\":{\"joingame\":{\"client_secret\":\"\",\"ip\":\"127.0.0.1:59028\",\"username\":\"123\"}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:39.233Z\",\"username\":\"123\"},{\"action\":{\"joingame\":{\"client_secret\":\"\",\"ip\":\"127.0.0.1:59030\",\"username\":\"player2\"}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:41.063Z\",\"username\":\"player2\"},{\"action\":{\"startgame\":{\"deterministic\":false,\"game_mode\":\"Standard\",\"max_players\":4,\"password\":null,\"rounds\":4,\"start_round\":null,\"visibility\":\"Public\"}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:44.556Z\",\"username\":\"player2\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:49.518Z\",\"username\":\"123\"},{\"action\":{\"bid\":0},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:50.990Z\",\"username\":\"player2\"},{\"action\":{\"bid\":0},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:53.586Z\",\"username\":\"player2\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:55.895Z\",\"username\":\"player2\"},{\"action\":{\"playcard\":{\"id\":11,\"played_by\":\"123\",\"suit\":\"heart\",\"value\":13}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:58.431Z\",\"username\":\"123\"},{\"action\":{\"playcard\":{\"id\":24,\"played_by\":\"player2\",\"suit\":\"diamond\",\"value\":13}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:40:59.599Z\",\"username\":\"player2\"},{\"action\":\"ack\",\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:08.870Z\",\"username\":\"123\"},{\"action\":\"ack\",\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:15.654Z\",\"username\":\"123\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:23.794Z\",\"username\":\"player2\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:25.478Z\",\"username\":\"123\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:25.547Z\",\"username\":\"123\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:41:26.611Z\",\"username\":\"123\"},{\"action\":{\"bid\":0},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:42:59.448Z\",\"username\":\"123\"},{\"action\":{\"playcard\":{\"id\":38,\"played_by\":\"player2\",\"suit\":\"club\",\"value\":14}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:00.923Z\",\"username\":\"player2\"},{\"action\":{\"playcard\":{\"id\":6,\"played_by\":\"123\",\"suit\":\"heart\",\"value\":8}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:02.793Z\",\"username\":\"123\"},{\"action\":{\"playcard\":{\"id\":30,\"played_by\":\"123\",\"suit\":\"club\",\"value\":6}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:03.577Z\",\"username\":\"123\"},{\"action\":\"ack\",\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:05.583Z\",\"username\":\"123\"},{\"action\":{\"playcard\":{\"id\":23,\"played_by\":\"player2\",\"suit\":\"diamond\",\"value\":12}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:06.828Z\",\"username\":\"player2\"},{\"action\":{\"playcard\":{\"id\":6,\"played_by\":\"123\",\"suit\":\"heart\",\"value\":8}},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:08.183Z\",\"username\":\"123\"},{\"action\":\"ack\",\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:10.174Z\",\"username\":\"123\"},{\"action\":\"ack\",\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:11.809Z\",\"username\":\"123\"},{\"action\":{\"bid\":0},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:27.467Z\",\"username\":\"123\"},{\"action\":{\"bid\":3},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:30.617Z\",\"username\":\"player2\"},{\"action\":{\"bid\":1},\"lobby\":\"new\",\"timestamp\":\"2024-11-11T21:43:43.398Z\",\"username\":\"player2\"}],\"gameplay_state\":{\"Play\":{\"hand_num\":1}},\"is_public\":true,\"lobby_code\":\"new\",\"player_order\":[\"player2\",\"123\"],\"players\":{\"123\":{\"details\":{\"client_secret\":\"sky_ptpiiwisgvqw\",\"ip\":\"127.0.0.1:59028\",\"username\":\"123\"},\"encrypted_hand\":\"KBBbNhRWSllFVRkfBg8UEywJAH1KVkFbWlVFURQDGANRUVs3FRUCHUtbSwUGGgQSUVFLIlwPUgANVVNCS1QBGxISHDsvFglLU1VYQVRUXVUAHhArUk5SAQwWGwdFWlMBEgcMOlJOQxRFDEsaA1RLRktHWy8cFQkMDSgLCkVMU0ZBWFtzUgcFAB1VU1EDHxAaHAUdfVxWBggFAgxRXUEMKg==\",\"id\":\"123\",\"num_cards\":0,\"role\":\"Player\"},\"player2\":{\"details\":{\"client_secret\":\"sky_fp8wek3m7l7g\",\"ip\":\"127.0.0.1:59030\",\"username\":\"player2\"},\"encrypted_hand\":\"KBBbNgJSAkFJSUMBVhVSAywJAH1cUkgbBBJWHwVOG0UAHhArREoaHwAKQRkVQBUREgcMOkRKAApJEBEEU04NUF9JCTMHCV0TOglKTw1ORwsSEhwtVFIUVRYeWhkVVhUPFgoLK0RcGgEEB0YIFVYOGl8QWzYCUgJEVUcRHVsNTgIXNBsmREoaBwkKSghFXhVLURgMNhJSAlUGB0YPFUAVERIHDDpESg4KOA==\",\"id\":\"player2\",\"num_cards\":0,\"role\":\"Player\"}},\"score\":{\"123\":21,\"player2\":0},\"secret_key\":\"mysecretkey\",\"setup_game_options\":{\"deterministic\":false,\"game_mode\":\"Standard\",\"max_players\":4,\"password\":null,\"rounds\":4,\"start_round\":null,\"visibility\":\"Public\"},\"system_status\":[\"Error with bid: EqualsRound\",\"Error with bid: EqualsRound\",\"Error with bid: EqualsRound\",\"Error with bid: EqualsRound\",\"Error with bid: EqualsRound\",\"Card is not valid: DidNotFollowSuit\",\"Error with bid: EqualsRound\"],\"trump\":\"club\",\"updated_at\":\"2024-11-11T21:43:43.399415Z\",\"wins\":{\"123\":0,\"player2\":0}}").unwrap();
        game.curr_round = 5;
        game.curr_dealer = "player2".to_string();
        game.bids = HashMap::new();
        game.bids.insert("123".to_string(), Some(4));
        game.gameplay_state = GameplayState::Bid;

        let bid_msg = GameMessage {
            username: "player2".to_string(),
            action: crate::GameAction::Bid(1),
            timestamp: Utc::now(),
            lobby: "new".to_string(),
        };
        let res = game.process_event(bid_msg.clone());

        assert_eq!(game.bids.get("player2"), None);

        let bid_msg2 = GameMessage {
            username: "player2".to_string(),
            action: crate::GameAction::Bid(5),
            timestamp: Utc::now(),
            lobby: "new".to_string(),
        };
        game.process_event(bid_msg2.clone());

        assert_eq!(*game.bids.get("player2").unwrap(), Some(5));
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
        assert_eq!(*game.bids.get(&firstplayer).clone().unwrap(), Some(3));

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
        assert_eq!(*game.bids.get(&secondplayer).clone().unwrap(), Some(1));
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

        assert_eq!(game.bids[&has_first_turn], Some(0));
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
        assert_eq!(game.bids[&has_second_turn], Some(0));

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
        //     ".setup_game_options.*" => "[sgo]",
        //     ".timestamp" => "[utc]",
        //     ".updated_at" => "[utc]",
        //     ".created_at" => "[utc]",
        //     ".players.*.encrypted_hand" => "[encrypted_hand]",
        //     ".players.*.details" => "[details]",
        //     ".event_log.*" => "[events]",
        //     ".wins" => insta::sorted_redaction(),
        //     ".player_bids" => insta::sorted_redaction(),
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
        //     ".setup_game_options.*" => "[sgo]",
        //     ".timestamp" => "[utc]",
        //     ".updated_at" => "[utc]",
        //     ".created_at" => "[utc]",
        //     ".players.*.encrypted_hand" => "[encrypted_hand]",
        //     ".players.*.details" => "[details]",
        //     ".event_log.*" => "[events]",
        //     ".wins" => insta::sorted_redaction(),
        //     ".player_bids" => insta::sorted_redaction(),
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

        let res = GameState::decrypt_player_hand(
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

        let res = GameState::decrypt_player_hand(
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
            ".setup_game_options.*" => "[sgo]",
            ".timestamp" => "[utc]",
            ".updated_at" => "[utc]",
            ".created_at" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".players.*.details" => "[details]",
            ".event_log" => "[events]",
            ".wins" => insta::sorted_redaction(),
            ".player_bids" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        // player one deals, player two bids first
        game.process_event(GameMessage {
            username: player_two.clone(),
            action: crate::GameAction::Bid(3),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });
        game.process_event(GameMessage {
            username: player_one.clone(),
            action: crate::GameAction::Bid(1), // origin: crate::Actioner::Player(has_second_turn.clone()),
            timestamp: Utc::now(),
            lobby: "lobby".to_string(),
        });

        // two players play cards, go into post hand state
        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 38,
                suit: Suit::Club,
                value: 14,
                played_by: Some(player_two.clone()),
            }),
            timestamp: Utc::now(),
        });
        game.process_event(GameMessage {
            username: player_one.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 51,
                suit: Suit::Spade,
                value: 14,
                played_by: Some(player_one.clone()),
            }),
            timestamp: Utc::now(),
        });
        insta::assert_yaml_snapshot!(game, {
            ".setup_game_options.*" => "[sgo]",
            ".timestamp" => "[utc]",
            ".updated_at" => "[utc]",
            ".created_at" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".players.*.details" => "[details]",
            ".event_log.*" => "[events]",
            ".wins" => insta::sorted_redaction(),
            ".player_bids" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        // move to next round
        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Ack,
            timestamp: Utc::now(),
        });

        // should be start of the next round (round 3, hand 2)
        insta::assert_yaml_snapshot!(game, {
            ".setup_game_options.*" => "[sgo]",
            ".timestamp" => "[utc]",
            ".updated_at" => "[utc]",
            ".created_at" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".players.*.details" => "[details]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".player_bids" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });

        // player2 starts again
        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 37,
                suit: Suit::Club,
                value: 13,
                played_by: Some(player_two.clone()),
            }),
            timestamp: Utc::now(),
        });

        game.process_event(GameMessage {
            username: player_one.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 25,
                suit: Suit::Diamond,
                value: 14,
                played_by: Some(player_one.clone()),
            }),
            timestamp: Utc::now(),
        });

        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Ack,
            timestamp: Utc::now(),
        });

        // hand 3/3, player2 starts again
        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 12,
                suit: Suit::Heart,
                value: 14,
                played_by: Some(player_two.clone()),
            }),
            timestamp: Utc::now(),
        });

        game.process_event(GameMessage {
            username: player_one.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::PlayCard(Card {
                id: 50,
                suit: Suit::Spade,
                value: 13,
                played_by: Some(player_one.clone()),
            }),
            timestamp: Utc::now(),
        });

        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Ack,
            timestamp: Utc::now(),
        });

        game.process_event(GameMessage {
            username: player_two.clone(),
            lobby: "lobby".to_string(),
            action: crate::GameAction::Ack,
            timestamp: Utc::now(),
        });

        // End of round 3
        insta::assert_yaml_snapshot!(game, {
            ".setup_game_options.*" => "[sgo]",
            ".timestamp" => "[utc]",
            ".updated_at" => "[utc]",
            ".created_at" => "[utc]",
            ".players.*.encrypted_hand" => "[encrypted_hand]",
            ".players.*.details" => "[details]",
            ".event_log[].timestamp" => "[event_timestamp]",
            ".wins" => insta::sorted_redaction(),
            ".player_bids" => insta::sorted_redaction(),
            ".bids" => insta::sorted_redaction(),
            ".score" => insta::sorted_redaction(),
            ".players" => insta::sorted_redaction(),
        });
    }

    #[test]
    fn test_test() {
        // let secret = "sky_jce98iqm6y7j";
        // let enc_hand = "KBBbNg5BXwgARVMdWhhODxc0GyZIWUdJVAgICERLFUZRGAw2HkFfG1wAEABZF1NIX0kPPgYWABsCXgxBTVteDlFRTWhGQRVVWRAUCWkbTkhJSQkzCxoASwpLXU9FDF4eUVFbLBoCAVwaRVMbVxVCD1FRSG8XTx4bUQ1TVwVVFRofCgA6DjwHQBpTUx1aGE4PAVlbc0gQEFBMS0tPXhxWGAdJVX0cAglMXUtLWEtVTEgaD1tlXlpJG0gFEBRTHWgICklDfRoPBEBdG0NPGltEHxofW2VIEBVYXAxTQRQPVgYGDltlW1EYZA==";

        let secret = "sky_p1zp1271hspm";
        let enc_hand = "KBBbNhQTQEEFHhVBBBIJCBc0GyZSC1hBAwEDA0pfUh4GAg19ShMeGVBfWF8MUVxPBQoVKhUTQENMHkwTARdSV0JdVX0AXRsJVFZoUxFRSk9CWUprQhNWUkJHXkVKSVIJGgoUMB5VWFwTRFZdHRZSV0YWVSRSWB5SCwIbExgfERQWDyY9CRNAUgAABAVaUVxPAB4QK1ILWBhUU0VFSl9SGxIHDDpSC0gNHUkVWAxRSl5HR1svHFADFVVtVUhKSVJcQVhNbVIdWANEW0MTUlETAQYJW3NSRxscRFcVC1lDDTA=";

        let hand = GameState::decrypt_player_hand(enc_hand.to_owned(), &secret.to_owned());
        println!("Decrypted: {:?}", hand);
    }
}
