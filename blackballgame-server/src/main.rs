use std::borrow::BorrowMut;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::future::IntoFuture;
use std::io;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::str::Bytes;
use std::sync::Arc;

use axum::extract::ws::CloseFrame;
use axum::extract::ConnectInfo;
use axum::extract::Path;
use axum::extract::State;
use axum::http::Method;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use axum_extra::headers;
use axum_extra::TypedHeader;
use client::GameClient;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use futures_util::Stream;
use futures_util::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;
use tracing::info;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

mod client;

#[derive(Debug, Clone, Copy)]
enum ServerError {}

struct AllGames {
    games: Mutex<HashMap<String, GameServer>>,
}

enum EventType {
    PlayCard(Card),
    DealCard(Card),
    WinHand,
    WinRound,
    Bid(i32),
}

#[derive(Debug)]
enum GameState {
    Deal,
    Bid,
    Play,
    Pregame,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Serialize, Deserialize)]
enum Suit {
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

async fn server_process(
    state: Arc<Mutex<GameServer>>,
    mut stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), ServerError> {
    tracing::info!("Starting up server...");

    let mut server = state.lock().await;

    stream.write_all(b"hello, world").await.unwrap();
    info!("success writing some bytes");
    // wait for people to connect
    // start game, ask for input from people, progress game
    let max_rounds = Some(3);

    server.play_game(max_rounds);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
struct Card {
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

// impl PartialOrd for Card {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         // match self.id.partial_cmp(&other.id) {
//         //Some(core::cmp::Ordering::Equal) => {}
//         //ord => return ord,
//         // }
//         match self.suit.partial_cmp(&other.suit) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         match self.value.partial_cmp(&other.value) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         // self.played_by.partial_cmp(&other.played_by)
//     }
// }

#[derive(Debug)]
struct GameServer {
    players: HashMap<String, GameClient>,
    deck: Vec<Card>,
    curr_round: i32,
    trump: Suit,
    dealing_order: Vec<String>,
    play_order: Vec<String>,
    // dealer_id: i32,
    bids: HashMap<String, i32>,
    wins: HashMap<String, i32>,
    score: HashMap<String, i32>,
    state: GameState,
}

#[derive(Debug, Clone, Copy)]
enum PlayerState {
    Idle,
    RequireInput,
}

#[derive(Debug)]
enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
}

fn valid_bids(curr_round: i32, curr_bids: &HashMap<String, i32>, is_dealer: bool) -> Vec<i32> {
    let mut valid_bids = vec![];
    for bid in 0..=curr_round {
        match validate_bid(&bid, curr_round, curr_bids, is_dealer) {
            Ok(x) => valid_bids.push(x),
            Err(err) => {}
        }
    }
    return valid_bids;
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
enum PlayedCardError {
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

fn get_random_card(mut deck: &mut Vec<Card>) -> Option<Card> {
    fastrand::shuffle(&mut deck);
    return deck.pop();
}

impl GameServer {
    // fn handle_event(&mut self, evt: String) {
    //     println!("Game recieved event: {}", evt);

    //     if evt.contains("show") {
    //         sender
    //             .send(Message::Text(
    //                 json!({
    //                     "hand": game.players.get(&0).unwrap().hand
    //                 })
    //                 .to_string(),
    //             ))
    //             .await
    //             .unwrap()
    //     }

    //     if t.contains("join") {}
    // }

    fn new() -> Self {
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
        };
        server
    }

    fn add_player(
        &mut self,
        player_id: String,
        rx: SplitSink<WebSocket, Message>,
        tx: SplitSink<WebSocket, Message>,
    ) {
        self.players
            .insert(player_id.clone(), GameClient::new(player_id, rx, tx));
    }

    fn play_game(&mut self, max_rounds: Option<i32>) {
        // let num_players = 3;
        // let max_rounds = Some(3);
        let mut deal_play_order: Vec<String> =
            self.players.iter().map(|(id, player)| id.clone()).collect();
        fastrand::shuffle(&mut deal_play_order);

        let mut play_order = deal_play_order.clone();
        let first = play_order.remove(0);
        play_order.push(first);

        self.players.iter().for_each(|(id, player)| {
            self.bids.insert(id.clone(), 0);
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

        tracing::info!("Players: {}\nRounds: {}", num_players, max_rounds);

        for round in 1..=max_rounds {
            tracing::info!("\n-- Round {} --", round);

            tracing::info!("\t/debug: deal order: {:#?}", self.dealing_order);
            tracing::info!("\t/debug: play order: {:#?}", self.play_order);

            self.deal();
            self.bids();
            self.play_round();

            // end of round
            // 1. figure out who lost, who won
            // 2. empty player hands, shuffle deck
            // 3. redistribute cards based on the round

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
                self.bids.insert(player.id.clone(), 0);
                player.clear_hand();
            }
            // self.clear_previous_round();
            self.advance_trump();
            self.curr_round += 1;
            let curr_dealer = self.dealing_order.remove(0);
            self.dealing_order.push(curr_dealer);

            let first_player = self.play_order.remove(0);
            self.play_order.push(first_player);

            tracing::info!("Player status: {:#?}", self.player_status());
        }
        // stages of the game
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

    fn bids(&mut self) {
        tracing::info!("=== Bidding ===");
        tracing::info!("Trump is {}", self.trump);

        for player_id in self.play_order.iter() {
            // let curr_index = if self.dealer_idx == self.players.len() as i32 - 1 {
            //     0
            // } else {
            //     self.dealer_idx + 1
            // };
            tracing::info!("Player {} to bid", player_id);
            let mut client = self.players.get_mut(player_id).unwrap();
            let valid_bids = valid_bids(
                self.curr_round,
                &self.bids,
                self.dealing_order[0] == *player_id,
            );
            let mut bid = client.get_client_bids(&valid_bids);

            loop {
                tracing::info!(
                    "\t/debug: bid={}, round={}, bids={:?}, dealer={}",
                    bid,
                    self.curr_round,
                    self.bids,
                    self.dealing_order[0]
                );
                match validate_bid(
                    &bid,
                    self.curr_round,
                    &self.bids,
                    self.dealing_order[0] == client.id,
                ) {
                    Ok(x) => {
                        tracing::info!("bid was: {}", x);
                        self.bids.insert(client.id.clone(), x);
                        break;
                    }
                    Err(e) => {
                        tracing::info!("Error with bid: {:?}", e);
                        bid = client.get_client_bids(&valid_bids);
                    }
                }
            }
        }
        tracing::info!("Biding over, bids are: {:?}", self.bids);
    }

    fn play_round(&mut self) {
        for handnum in 1..=self.curr_round {
            tracing::info!(
                "--- Hand #{}/{} - Trump: {}---",
                handnum,
                self.curr_round,
                self.trump
            );
            // need to use a few things to see who goes first
            // 1. highest bid (at round start)
            // 2. person who won the trick in last round goes first, then obey existing order

            // ask for input from each client in specific order (first person after dealer)
            let mut played_cards: Vec<Card> = vec![];

            let mut curr_winning_card: Option<Card> = None;

            for player_id in self.play_order.iter() {
                let player = self.players.get_mut(player_id).unwrap();

                let valid_cards_to_play = player
                    .hand
                    .iter()
                    .filter_map(|card| {
                        match is_played_card_valid(&played_cards, &player.hand, card, &self.trump) {
                            Ok(x) => Some(x),
                            Err(err) => None,
                        }
                    })
                    .collect::<Vec<Card>>();

                let (loc, mut card) = player.play_card(&valid_cards_to_play);
                loop {
                    match is_played_card_valid(
                        &played_cards.clone(),
                        &mut player.hand,
                        &card.clone(),
                        &self.trump,
                    ) {
                        Ok(x) => {
                            tracing::info!("card is valid");
                            card = x;
                            // remove the card from the players hand
                            player.hand.remove(loc);
                            break;
                        }
                        Err(e) => {
                            tracing::info!("card is NOT valid: {:?}", e);
                            (_, card) = player.play_card(&valid_cards_to_play);
                        }
                    }
                }
                played_cards.push(card.clone());

                // logic for finding the winning card
                if curr_winning_card.is_none() {
                    curr_winning_card = Some(card);
                } else {
                    let curr = curr_winning_card.clone().unwrap();
                    if card.suit == curr.suit && card.value > curr.value {
                        curr_winning_card = Some(card.clone());
                    }
                    if card.suit == self.trump
                        && curr.suit == self.trump
                        && card.clone().value > curr.value
                    {
                        curr_winning_card = Some(card);
                    }
                }

                tracing::info!(
                    "Curr winning card: {:?}",
                    curr_winning_card.clone().unwrap()
                );
            }

            tracing::info!("End turn, trump={:?}, played cards:", self.trump);
            played_cards
                .clone()
                .iter()
                .for_each(|c| tracing::info!("{}", c));

            let win_card = curr_winning_card.unwrap();
            tracing::info!(
                "Player {:?} won. Winning card: {:?}",
                win_card.played_by,
                win_card
            );
            let winner = win_card.played_by;
            if let Some(x) = self.wins.get_mut(&winner.unwrap()) {
                *x = *x + 1;
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

/// Shorthand for the transmit half of the message channel.
type Tx = SplitSink<WebSocket, Message>;

/// Shorthand for the receive half of the message channel.
type Rx = SplitSink<WebSocket, Message>;

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

/// helper to print contents of messages to stdout. Has special treatment for Close.
fn process_message(
    msg: Message,
    who: SocketAddr,
    mut game: Arc<Mutex<HashMap<String, GameServer>>>,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {who} somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>> {who} sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {who} sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(())
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr, mut state: Arc<AllGames>) {
    // send a ping (unsupported by some browsers) just to kick things off and get a response
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged {who}...");
    } else {
        println!("Could not send ping {who}!");
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    let (mut sender, mut receiver) = socket.split();

    // receive single message from a client (we can either receive or send with socket).
    // this will likely be the Pong for our Ping or a hello message from client.
    // waiting for message from a client will block this task, but will not block other client's
    // connections.
    while let Some(Ok(message)) = receiver.next().await {
        match message {
            Message::Text(name) => {
                println!(">>> {who} sent str: {name:?}");
                #[derive(Deserialize)]
                struct Connect {
                    username: String,
                    channel: String,
                }

                let connect: Connect = match serde_json::from_str(&name) {
                    Ok(connect) => connect,
                    Err(error) => {
                        tracing::error!(%error);
                        let _ = sender
                            .send(Message::Text(String::from(
                                "Failed to parse connect message",
                            )))
                            .await;
                        break;
                    }
                };

                // Scope to drop the mutex guard before the next await
                {
                    // If username that is sent by client is not taken, fill username string.
                    let mut games = state.games.lock().await;

                    let channel = connect.channel.clone();
                    let room = games
                        .entry(connect.channel)
                        .or_insert_with(|| GameServer::new);

                    tx = Some(room.tx.clone());

                    if !room.user_set.contains(&connect.username) {
                        room.user_set.insert(connect.username.to_owned());
                        username = connect.username.clone();
                    }
                }

                // If not empty we want to quit the loop else we want to quit function.
                if tx.is_some() && !username.is_empty() {
                    break;
                } else {
                    // Only send our client that username is taken.
                    let _ = sender
                        .send(Message::Text(String::from("Username already taken.")))
                        .await;

                    return;
                }
            }
            Message::Binary(d) => {
                println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
            }
            Message::Close(c) => {
                if let Some(cf) = c {
                    println!(
                        ">>> {} sent close with code {} and reason `{}`",
                        who, cf.code, cf.reason
                    );
                } else {
                    println!(">>> {who} somehow sent close message without CloseFrame");
                }
            }

            Message::Pong(v) => {
                println!(">>> {who} sent pong with {v:?}");
            }
            // You should never need to manually handle Message::Ping, as axum's websocket library
            // will do so for you automagically by replying with Pong and copying the v according to
            // spec. But if you need the contents of the pings you can see them here.
            Message::Ping(v) => {
                println!(">>> {who} sent ping with {v:?}");
            }
        }
    }

    // let sender: SplitSink<WebSocket, Message>;
    // let receiver: SplitSink<WebSocket, Message>;

    // state
    //     .get_mut()
    //     .add_player("testplayer".to_string(), receiver, sender);

    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).

    // Spawn a task that will push several messages to the client (does not matter what client does)
    let mut send_task = tokio::spawn(async move {
        println!("Sending close to {who}...");
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: Cow::from("Goodbye"),
            })))
            .await
        {
            println!("Could not send Close due to {e}, probably it is ok?");
        }
        // n_msg
    });

    // This second task will receive messages from client and print them on server console
    let mut recv_task = tokio::spawn(async move {
        let mut cnt = 0;
        while let Some(Ok(msg)) = receiver.next().await {
            cnt += 1;
            // print message and break if instructed to do so
            if process_message(msg.clone(), who, state.clone()).is_break() {
                break;
            }

            match msg {
                Message::Text(t) => {
                    if t.contains("new_game") {
                        let mut game = state.borrow_mut().lock().await;
                        // game.handle_event(t);
                    }
                }
                Message::Binary(b) => todo!(),
                _ => {}
            }
        }
        cnt
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        // rv_a = (&mut send_task) => {
        //     match rv_a {
        //         Ok(a) => println!("{a} messages sent to {who}"),
        //         Err(a) => println!("Error sending messages {a:?}")
        //     }
        //     recv_task.abort();
        // },
        rv_b = (&mut recv_task) => {
            match rv_b {
                Ok(b) => println!("Received {b} messages"),
                Err(b) => println!("Error receiving messages {b:?}")
            }
            // send_task.abort();
        }
    }

    // returning from the handler closes the websocket connection
    println!("Websocket context {who} destroyed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AllGames>>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn root() -> &'static str {
    "Hello, World"
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("blackballgame-server=info".parse().unwrap()),
        )
        .with_span_events(FmtSpan::FULL)
        .init();

    let allgames = AllGames {
        games: Mutex::new(HashMap::new()),
    };

    let serverstate: Arc<AllGames> = Arc::new(allgames);

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);

    // let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let assets_dir =
        PathBuf::from("/Users/jarde/Documents/code/blackballgame/blackballgame-client/dist");

    let app = Router::new()
        // `GET /` goes to `root`
        // .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        // .route("/", get(root))
        .route("/ws", get(ws_handler))
        .nest_service(
            "/",
            ServeDir::new(assets_dir)
                .fallback(ServeFile::new("blackballgame-server/assets/index.html")),
        )
        .layer(cors)
        // .route("/ui".get(ServeDir::new(assets_dir).append_index_html_on_directories(true)))
        // .route("/game", get(Game))
        .with_state(serverstate);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    // loop {
    //     tracing::info!("Waiting for inbound TcpStream");
    //     // Asynchronously wait for an inbound TcpStream.
    //     let (stream, addr) = listener.accept().await.unwrap();

    //     tracing::info!("Got message, beginning game...");

    //     // Clone a handle to the `Shared` state for the new connection.
    //     let state = Arc::clone(&serverstate);

    //     // Spawn our handler to be run asynchronously.
    //     tokio::spawn(async move {
    //         tracing::debug!("accepted connection");
    //         if let Err(e) = server_process(state, stream, addr).await {
    //             tracing::info!("an error occurred; error = {:?}", e);
    //         }
    //     });
    // }
}
