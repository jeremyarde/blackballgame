use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, iter::Cycle};

mod client;
mod game;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Deserialize)]
pub enum GameState {
    // Deal,
    Bid,
    Play,
    Pregame,
    // PostRound,
    // PreRound,
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
    pub hand: Vec<Card>,
    pub order: i32,
    pub trump: Suit,
    pub round: i32,
    // pub state: PlayerState,
    pub role: PlayerRole,
    // pub sender: SplitSink<WebSocket, Message>, // don't need if we are using JS
}

impl fmt::Display for GameClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameClient: id={}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServer {
    pub players: HashMap<String, GameClient>,
    pub players_secrets: HashMap<String, String>,
    pub deck: Vec<Card>,
    pub curr_round: i32,
    pub trump: Suit,
    pub player_order: Vec<String>,
    pub curr_played_cards: Vec<Card>,
    pub curr_player_turn: Option<String>,
    #[serde(skip)]
    pub curr_player_cycle: Cycle<String>,
    pub curr_winning_card: Option<Card>,
    pub curr_dealer: String,
    // play_order: Vec<String>,
    // dealer_id: i32,
    pub bids: HashMap<String, i32>,
    pub bid_order: Vec<(String, i32)>,
    // bid_order: Vec<
    pub wins: HashMap<String, i32>,
    pub score: HashMap<String, i32>,
    pub state: GameState,
    // pub tx: broadcast::Sender<FullGameState>,
    pub event_log: Vec<GameMessage>,
    pub system_status: Vec<String>,
    // pub event_queue: Vec<GameEvent>,
    // rx: broadcast::Receiver<String>,
    //     tx: broadcast::Sender<String>,
    //     rx: SplitStream<Message>,
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
            id: cardid + 1,
            suit: Suit::Diamond,
            value,
            played_by: None,
        });
        cards.push(Card {
            id: cardid + 2,
            suit: Suit::Club,
            played_by: None,

            value,
        });
        cards.push(Card {
            id: cardid + 3,
            suit: Suit::Spade,
            value,
            played_by: None,
        });
        cardid += 4;
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
    pub origin: Actioner,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameAction {
    // Player actions
    PlayCard(Card),
    Bid(i32),

    // System actions
    StartGame,
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
