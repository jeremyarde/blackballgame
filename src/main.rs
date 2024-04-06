#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq)]
enum Suit {
    Heart,
    Diamond,
    Club,
    Spade,
    NoTrump,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
struct Card {
    id: usize,
    suit: Suit,
    value: i32,
    played_by: Option<i32>,
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

#[derive(Debug, Clone)]
struct GameServer {
    players: Vec<GameClient>,
    deck: Vec<Card>,
    round: i32,
    trump: Suit,
    player_order: Vec<i32>,
    dealer: i32,
}

#[derive(Debug, Clone)]
struct GameClient {
    id: i32,
    hand: Vec<Card>,
    order: i32,
    trump: Suit,
    round: i32,
    state: PlayerState,
    bids: Vec<i32>,
}

#[derive(Debug, Clone, Copy)]
enum PlayerState {
    Idle,
    RequireInput,
}

use std::io::{self, Read};

impl GameClient {
    fn new(id: i32) -> Self {
        return GameClient {
            id,
            state: PlayerState::Idle,
            hand: vec![],
            order: 0,
            round: 0,
            trump: Suit::Heart,
            bids: vec![],
        };
    }
    fn play_card(&mut self) -> Card {
        let mut input = String::new();
        println!("Select the card you want to play");
        println!("{:#?}", self.hand);
        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");

        while input.parse::<i32>().is_err() {
            println!("Please enter a valid card position");
            io::stdin()
                .read_line(&mut input)
                .expect("error: unable to read user input");
        }
        println!("{}", input);

        return self.hand.pop().unwrap();
    }

    fn get_client_bids(&mut self) -> i32 {
        let mut input = String::new();
        println!("How many tricks do you want?");
        // println!("{:#?}", self.hand);
        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");

        loop {
            if input.parse::<i32>().is_err() {
                continue;
            }
            match validate_bid(input.parse::<i32>().unwrap(), self.round, self.bids) == BidError {
                true => break,
                false => {
                    println!("Please enter a valid number of tricks");
                    io::stdin()
                        .read_line(&mut input)
                        .expect("error: unable to read user input");
                },
            }
        }

        println!("{}", input);
        return input;



    }
}

enum BidError {
    High,
    Low,
    Invalid,
    EqualsRound,
}

fn validate_bid(bid: i32, curr_round: i32, curr_bids: Vec<i32>) -> Result<i32, BidError> {
    // can bid between 0..=round number
    // dealer can't bid a number that will equal the round number
    if bid > curr_round {
        return Err(BidError::High);
    }

    if bid < 0 {
        BidError::Low;
    }

    if (bid + curr_bids.into_iter().sum::<i32>()) == curr_round {
        return Err(BidError::EqualsRound);
    }

    return Ok(bid);
}

impl GameServer {
    fn get_random_card(&mut self) -> Option<Card> {
        fastrand::shuffle(&mut self.deck);
        return self.deck.pop();
    }

    fn play_round(&mut self) {
        // ask for input from each client in specific order (first person after dealer)
        let mut played_cards: Vec<Card> = vec![];

        let mut curr_winning_card: Card;

        for (i, x) in self.player_order.iter().enumerate() {
            

            let player = self.players.get(*x as usize).unwrap();
            let card = player.play_card();
            played_cards.push(card.clone());

            if i==0 {
                curr_winning_card = card;
                continue;
            }

            if card.suit == curr_winning_card.suit && card.value > curr_winning_card.value {
                curr_winning_card = card.clone();
            }
            if card.suit == self.trump
                && curr_winning_card.suit == self.trump
                && &card.value > &curr_winning_card.value
            {
                curr_winning_card = card;
            }
        }

        // match curr_winning_card {
        //     Some(x) => x.played_by,
        //     None => println!("Error finding winning card. This is bad"),
        // }

        println!("Winning card was: {:?}", curr_winning_card);

        // let winning_card = played_cards.sort_by(|card| card);
    }

    fn deal(&mut self) {
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.round {
            // get random card, give to a player
            for playerid in self.player_order.clone() {
                let card = self.get_random_card().unwrap();
                let mut player: &mut GameClient = self.players.get_mut(playerid as usize).unwrap();
                // .get(playerid).unwrap();
                player.hand.push(card.clone().to_owned());
            }
        }
    }

    fn player_status(&self) {
        println!("{:?}", self.players);
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

fn main() {
    let players: Vec<GameClient> = (0..3).into_iter().map(|id| GameClient::new(id)).collect();
    let dealerid = fastrand::usize(..&players.len()) as i32;
    let mut server = GameServer {
        players,
        deck: create_deck(),
        round: 1,
        trump: Suit::Heart,
        player_order: vec![0, 1],
        dealer: dealerid,
    };

    // stages of the game
    // server.deal();
    // server.play_round();

    println!("{:?}", server.deck.sort());

    // println!("{:#?}", server);
    println!("{:#?}", server.player_status());
}
