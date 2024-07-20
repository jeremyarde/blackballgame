use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, iter::Cycle};
use tracing::info;

mod client;
mod game;

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub enum GameplayState {
    Bid,
    Play(PlayState),
    Pregame,
    PostRound,           // players played all cards
    PostHand(PlayState), // each player played a card
}

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub struct PlayState {
    hand_num: usize,
}

impl PlayState {
    fn new() -> PlayState {
        return PlayState { hand_num: 1 };
    }
    fn from(new_hand_num: usize) -> PlayState {
        return PlayState {
            hand_num: new_hand_num.try_into().unwrap(),
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerRole {
    Leader,
    Player,
}

enum GameError {
    InternalIssue(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameClient {
    pub id: String,
    #[serde(skip)]
    pub hand: Vec<Card>, // we don't want everyone getting this information
    pub encrypted_hand: String,
    pub num_cards: i32,
    pub role: PlayerRole,
}

impl fmt::Display for GameClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameClient: id={}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub setup_game_options: SetupGameOptions,
    secret_key: String,
    pub players: HashMap<String, GameClient>,
    #[serde(skip)]
    pub players_secrets: HashMap<String, String>,
    #[serde(skip)]
    pub deck: Vec<Card>,
    pub curr_round: i32,
    pub trump: Suit,
    pub player_order: Vec<String>,
    pub curr_played_cards: Vec<Card>,
    pub curr_player_turn: Option<String>,
    #[serde(skip)]
    curr_player_turn_idx: usize,
    pub curr_winning_card: Option<Card>,
    curr_dealer: String,
    #[serde(skip)]
    pub curr_dealer_idx: usize,
    pub bids: HashMap<String, i32>,
    pub bid_order: Vec<(String, i32)>,
    pub wins: HashMap<String, i32>,
    pub score: HashMap<String, i32>,
    pub gameplay_state: GameplayState,
    pub event_log: Vec<GameMessage>,
    // #[serde(skip)]
    pub system_status: Vec<String>, // useful to tell players what is going wrong
    is_public: bool,
    latest_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupGameOptions {
    rounds: usize,
    deterministic: bool,
    start_round: Option<usize>,
}

impl SetupGameOptions {
    pub fn new() -> SetupGameOptions {
        return SetupGameOptions {
            rounds: 99,
            deterministic: false,
            start_round: None,
        };
    }

    pub fn from(
        max_rounds: usize,
        deterministic: bool,
        start_round: Option<usize>,
    ) -> SetupGameOptions {
        return SetupGameOptions {
            rounds: max_rounds,
            deterministic: deterministic,
            start_round: start_round,
        };
    }
}

pub fn create_deck() -> Vec<Card> {
    let mut cards = vec![];

    // 14 = Ace
    let mut cardid = 0;
    for value in 2..=14 {
        cards.push(Card {
            id: cardid,
            suit: Suit::Heart,
            value,
            played_by: None,
        });
        cards.push(Card {
            id: cardid + 13,
            suit: Suit::Diamond,
            value,
            played_by: None,
        });
        cards.push(Card {
            id: cardid + 26,
            suit: Suit::Club,
            played_by: None,

            value,
        });
        cards.push(Card {
            id: cardid + 39,
            suit: Suit::Spade,
            value,
            played_by: None,
        });
        cardid += 1;
    }

    cards
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Connect {
    pub username: String,
    pub channel: String,
    pub secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub struct GameEvent {
    pub action: GameAction,
    // pub origin: Actioner,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameAction {
    // Player actions
    PlayCard(Card),
    Bid(i32),
    Ack,

    // System actions
    StartGame(SetupGameOptions),
    Deal,
    CurrentState,
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
    pub id: usize,
    pub suit: Suit,
    pub value: i32,
    pub played_by: Option<String>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct GameMessage {
    pub username: String,
    pub message: GameEvent,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Actioner {
    System,
    Player(String),
}

#[cfg(test)]
mod tests {
    use super::*;
}
