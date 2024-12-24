use chrono::{DateTime, Utc};
use data_encoding::BASE64;
// use game::xor_encrypt_decrypt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, fmt};

mod ai;
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Destination {
    Lobby(Vec<PlayerDetails>),
    User(PlayerDetails),
}

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub struct PlayState {
    pub hand_num: i32,
    pub hands: i32,
}

// impl Default for PlayState {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl PlayState {
    // pub fn new() -> PlayState {
    //     PlayState {
    //         hand_num: 1,
    //         hands: 1,
    //     }
    // }
    pub fn from(new_hand_num: i32, hands: i32) -> PlayState {
        PlayState {
            hand_num: new_hand_num
                .try_into()
                .expect("Failed to convert hand num into usize"),
            hands: hands,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlayerRole {
    Leader,
    Player,
    Computer,
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
pub fn xor_encrypt_decrypt(data: &str, key: &str) -> Vec<u8> {
    data.as_bytes()
        .iter()
        .zip(key.as_bytes().iter().cycle())
        .map(|(d, k)| d ^ k)
        .collect()
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
        let encoded = xor_encrypt_decrypt(
            &plaintext_hand,
            player_secret.as_ref().unwrap_or(&"".to_string()),
        );
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
    setup_game_options: SetupGameOptions,
    secret_key: String,
    pub players: HashMap<String, GameClient>,
    #[serde(skip)]
    pub players_secrets: HashMap<String, String>,
    #[serde(skip)]
    pub deck: Vec<Card>,
    pub curr_round: i32,
    pub max_rounds: i32,
    pub cards_to_deal: i32,
    pub trump: Suit,
    pub player_order: Vec<String>,
    pub curr_played_cards: Vec<Card>,
    pub curr_player_turn: Option<String>,
    curr_player_turn_idx: usize,
    pub curr_winning_card: Option<Card>,
    pub curr_dealer: String,
    curr_dealer_idx: usize,
    pub bids: HashMap<String, Option<i32>>,
    pub player_bids: Vec<(String, i32)>,
    pub wins: HashMap<String, i32>,
    pub score: HashMap<String, i32>,
    pub gameplay_state: GameplayState,
    pub event_log: Vec<GameMessage>,
    pub system_status: Vec<String>, // useful to tell players what is going wrong
    is_public: bool,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub trump_played_in_round: bool,
}

impl GameState {
    pub fn get_game_mode(&self) -> String {
        return self.setup_game_options.game_mode.clone();
    }
    pub fn get_max_players(&self) -> usize {
        return self.setup_game_options.max_players;
    }

    pub fn decrypt_player_hand(hand: String, player_secret: &String) -> Vec<Card> {
        // info!("Decrypting hand: {:?}, {:?}", hand, player_secret);
        if player_secret.is_empty() {
            // error!("Player secret is empty");
            return vec![];
        }

        if hand.is_empty() {
            // info!("Hand is empty");
            return vec![];
        }
        let hand = BASE64
            .decode(hand.as_bytes())
            .expect("Could not decode hand");
        let str_hand = String::from_utf8(hand).expect("Could not convert hand to string");
        let secret_data = xor_encrypt_decrypt(&str_hand, player_secret);
        let actual_hand: Vec<Card> =
            serde_json::from_slice(&secret_data).expect("Could not parse hand");
        actual_hand
    }
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
    pub computer_players: usize,
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
            max_players: 8,
            game_mode: "Standard".to_string(),
            visibility: GameVisibility::Public,
            password: None,
            computer_players: 0,
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
            computer_players: 0,
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
    Connect(PlayerDetails),
    JoinGame(PlayerDetails),
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
pub struct PlayerDetails {
    pub username: String,
    pub ip: Option<String>,
    pub client_secret: Option<String>,
    pub lobby: String,
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
            value,
            played_by: None,
        }
    }
    pub fn with_played_by(suit: Suit, value: i32, played_by: String) -> Self {
        Card {
            id: 0,
            suit,
            value,
            played_by: Some(played_by),
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
