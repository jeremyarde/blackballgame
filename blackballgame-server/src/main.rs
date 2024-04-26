use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::str::Bytes;
use std::sync::Arc;

use axum::extract::ws::CloseFrame;
use axum::extract::ConnectInfo;
use axum::extract::Path;
use axum::http::Method;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use axum_extra::headers;
use axum_extra::TypedHeader;
use client::GameClient;
use futures_util::SinkExt;
use futures_util::StreamExt;
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

#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq)]
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct Card {
    id: usize,
    suit: Suit,
    value: i32,
    played_by: Option<i32>,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let played_by = if self.played_by.is_some() {
            format!(" (Player {})", self.played_by.unwrap())
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
    players: HashMap<i32, GameClient>,
    deck: Vec<Card>,
    curr_round: i32,
    trump: Suit,
    dealing_order: Vec<i32>,
    play_order: Vec<i32>,
    // dealer_id: i32,
    bids: HashMap<i32, i32>,
    wins: HashMap<i32, i32>,
    score: HashMap<i32, i32>,
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

fn valid_bids(curr_round: i32, curr_bids: &HashMap<i32, i32>, is_dealer: bool) -> Vec<i32> {
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
    curr_bids: &HashMap<i32, i32>,
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
    fn play_game(&mut self, max_rounds: Option<i32>) {
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
                self.wins.insert(player.id, 0);
                self.bids.insert(player.id, 0);
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
                        self.bids.insert(client.id, x);
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
            let winner = win_card.played_by;
            tracing::info!("Player {:?} won. Winning card: {}", winner, win_card);

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
type Tx = mpsc::UnboundedSender<String>;

/// Shorthand for the receive half of the message channel.
type Rx = mpsc::UnboundedReceiver<String>;

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

/// helper to print contents of messages to stdout. Has special treatment for Close.
fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
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
async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    // send a ping (unsupported by some browsers) just to kick things off and get a response
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged {who}...");
    } else {
        println!("Could not send ping {who}!");
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    // receive single message from a client (we can either receive or send with socket).
    // this will likely be the Pong for our Ping or a hello message from client.
    // waiting for message from a client will block this task, but will not block other client's
    // connections.
    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if process_message(msg, who).is_break() {
                return;
            }
        } else {
            println!("client {who} abruptly disconnected");
            return;
        }
    }

    // Since each client gets individual statemachine, we can pause handling
    // when necessary to wait for some external event (in this case illustrated by sleeping).
    // Waiting for this client to finish getting its greetings does not prevent other clients from
    // connecting to server and receiving their greetings.
    // for i in 1..5 {
    //     if socket
    //         .send(Message::Text(format!("Hi {i} times!")))
    //         .await
    //         .is_err()
    //     {
    //         println!("client {who} abruptly disconnected");
    //         return;
    //     }
    //     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    // }

    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task that will push several messages to the client (does not matter what client does)
    // let mut send_task = tokio::spawn(async move {
    //     // println!("Sending close to {who}...");
    //     // if let Err(e) = sender
    //     //     .send(Message::Close(Some(CloseFrame {
    //     //         code: axum::extract::ws::close_code::NORMAL,
    //     //         reason: Cow::from("Goodbye"),
    //     //     })))
    //     //     .await
    //     // {
    //     //     println!("Could not send Close due to {e}, probably it is ok?");
    //     // }
    //     // n_msg
    // });

    // This second task will receive messages from client and print them on server console
    let mut recv_task = tokio::spawn(async move {
        let mut cnt = 0;
        while let Some(Ok(msg)) = receiver.next().await {
            cnt += 1;
            // print message and break if instructed to do so
            if process_message(msg, who).is_break() {
                break;
            }
            sender
                .send(Message::Text("Sending from server :)".to_string()))
                .await
                .unwrap()
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
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn root() -> &'static str {
    "Hello, World"
}

#[tokio::main]
async fn main() {
    let num_players = 3;
    let max_rounds = Some(3);

    let players: HashMap<i32, GameClient> = (0..num_players)
        .into_iter()
        .map(|id| {
            let (tx, rx) = mpsc::unbounded_channel();

            return (id, GameClient::new(id, rx, tx));
        })
        .collect();
    let mut deal_play_order: Vec<i32> = players.iter().map(|(id, player)| id.clone()).collect();
    fastrand::shuffle(&mut deal_play_order);

    let mut play_order = deal_play_order.clone();
    let first = play_order.remove(0);
    play_order.push(first);

    let mut server = GameServer {
        players: players,
        deck: create_deck(),
        curr_round: 1,
        trump: Suit::Heart,
        dealing_order: deal_play_order.clone(),
        play_order: play_order,
        // dealer_id: deal_play_order[0],
        bids: HashMap::new(),
        wins: HashMap::new(),
        score: HashMap::new(),
    };

    server.players.iter().for_each(|(&id, player)| {
        server.bids.insert(id, 0);
        server.wins.insert(id, 0);
        server.score.insert(id, 0);
    });

    // Configure a `tracing` subscriber that logs traces emitted by the chat
    // server.
    tracing_subscriber::fmt()
        // Filter what traces are displayed based on the RUST_LOG environment
        // variable.
        //
        // Traces emitted by the example code will always be displayed. You
        // can set `RUST_LOG=tokio=trace` to enable additional traces emitted by
        // Tokio itself.
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("blackballgame-server=info".parse().unwrap()),
        )
        // Log events when `tracing` spans are created, entered, exited, or
        // closed. When Tokio's internal tracing support is enabled (as
        // described above), this can be used to track the lifecycle of spawned
        // tasks on the Tokio runtime.
        .with_span_events(FmtSpan::FULL)
        // Set this subscriber as the default, to collect all traces emitted by
        // the program.
        .init();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:6142".to_string());

    let listener = TcpListener::bind(&addr).await.unwrap();

    let serverstate = Arc::new(Mutex::new(server));

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
