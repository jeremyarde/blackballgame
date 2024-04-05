#[derive(Debug, Clone)]
enum Suit {
    Heart,
    Diamond,
    Club,
    Spade,
    NoTrump,
}

#[derive(Debug, Clone)]
struct Card {
    id: usize,
    suit: Suit,
    value: i32,
}

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
        };
    }
    fn play_card(&mut self) -> Card {
        let mut input = String::new();
        println!("Select the card you want to play");
        println!("{:#?}", self.hand);
        io::stdin()
            .read_line(&mut input)
            .expect("error: unable to read user input");
        println!("{}", input);

        return self.hand.pop().unwrap();
    }
}

impl GameServer {
    fn get_random_card(&mut self) -> Option<Card> {
        fastrand::shuffle(&mut self.deck);
        return self.deck.pop();
    }

    fn play_round(&mut self) {
        // ask for input from each client in specific order (first person after dealer)
        for x in &self.player_order {
            let player = self.players.get(*x as usize).unwrap();
            let card = player.play_card();
        }
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
        });
        cards.push(Card {
            id: cardid + 1,
            suit: Suit::Diamond,
            value: value,
        });
        cards.push(Card {
            id: cardid + 2,
            suit: Suit::Club,
            value: value,
        });
        cards.push(Card {
            id: cardid + 3,
            suit: Suit::Spade,
            value: value,
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

    server.deal();
    server.play_round();

    // println!("{:#?}", server);
    println!("{:#?}", server.player_status());
}
