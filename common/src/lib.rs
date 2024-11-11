use chrono::{DateTime, Utc};
use data_encoding::BASE64;
use game::xor_encrypt_decrypt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fmt, net::SocketAddr};

mod client;
mod game;

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub enum GameplayState {
    Bid,
    Play(PlayState),
    Pregame,
    PostRound,           // players played all cards
    PostHand(PlayState), // each player played a card
    End,
}

// #[derive(Clone, Debug, Deserialize, Serialize)]
// pub enum InternalMessage {
//     ToGame {
//         lobby_code: String,
//         from: Destination,
//         msg: GameMessage,
//     },
//     // Server { dest: Destination, msg: Connect },
//     ToClient {
//         to: Destination,
//         msg: GameEventResult,
//     }, // from game server to client
//        // WsAction(WsAction),
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Destination {
    Lobby(Vec<PlayerDetails>),
    User(PlayerDetails),
}

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub struct PlayState {
    pub hand_num: usize,
}

impl PlayState {
    pub fn new() -> PlayState {
        PlayState { hand_num: 1 }
    }
    pub fn from(new_hand_num: usize) -> PlayState {
        PlayState {
            hand_num: new_hand_num
                .try_into()
                .expect("Failed to convert hand num into usize"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlayerRole {
    Leader,
    Player,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameError {
    InternalIssue(String),
    NotEnoughPlayers,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameClient {
    pub id: String,
    #[serde(skip)]
    pub hand: Vec<Card>, // we don't want everyone getting this information
    // #[serde(skip)]
    // pub secret: String,
    pub encrypted_hand: String,
    pub num_cards: i32,
    pub role: PlayerRole,
    pub details: PlayerDetails,
}

impl GameClient {
    pub fn update_hand(&mut self, new_hand: Vec<Card>) {
        self.hand = new_hand;
        self.encrypt_hand();
    }

    fn encrypt_hand(&mut self) {
        let hand = self.hand.clone();
        let plaintext_hand = json!(hand).to_string();
        let player_secret = &self.details.client_secret;
        let encoded = xor_encrypt_decrypt(&plaintext_hand, player_secret);
        let secret_data = BASE64.encode(&encoded);

        self.encrypted_hand = secret_data;
    }
}

impl fmt::Display for GameClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameClient: id={}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub lobby_code: String,
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
    pub curr_dealer: String,
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
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEventResult {
    pub dest: Destination,
    pub msg: GameActionResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameActionResponse {
    Connect(Connect),
    GameState(GameState),
    Message(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupGameOptions {
    pub rounds: usize,
    pub deterministic: bool,
    pub start_round: Option<usize>,
    pub max_players: usize,
    pub game_mode: String,
    pub visibility: GameVisibility,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameVisibility {
    Public,
    Private,
}

impl Default for SetupGameOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupGameOptions {
    pub fn new() -> SetupGameOptions {
        SetupGameOptions {
            rounds: 9,
            deterministic: false,
            start_round: None,
            max_players: 4,
            game_mode: "Standard".to_string(),
            visibility: GameVisibility::Public,
            password: None,
        }
    }

    pub fn from(
        max_rounds: usize,
        deterministic: bool,
        start_round: Option<usize>,
        max_players: usize,
        game_mode: String,
        visibility: GameVisibility,
        password: Option<String>,
    ) -> SetupGameOptions {
        SetupGameOptions {
            rounds: max_rounds,
            deterministic,
            start_round,
            max_players,
            game_mode,
            visibility,
            password,
        }
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Connect {
    pub username: String,
    pub channel: String,
    pub secret: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlayerSecret {
    pub client_secret: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum AllPossibleMessages {
    Connect(Connect),
    PlayerSecret(PlayerSecret),
    GameState(GameState),
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
    Connect {
        username: String,
        channel: String,
        secret: Option<String>,
    },
    JoinGame(PlayerDetails),
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
pub struct PlayerDetails {
    pub username: String,
    pub ip: String,
    pub client_secret: String,
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

impl Card {
    pub fn new(suit: Suit, value: i32) -> Self {
        Card {
            id: 0,
            suit,
            value: value,
            played_by: None,
        }
    }
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct GameMessage {
    pub username: String,
    pub action: GameAction,
    pub timestamp: DateTime<Utc>,
    pub lobby: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Actioner {
    System,
    Player(String),
}

#[cfg(test)]
mod tests {}
