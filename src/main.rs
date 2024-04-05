#[derive(Debug, Clone)]
struct Player {
    hand: Vec<Card>,
    id: i32,
}

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
    players: Vec<Player>,
    deck: Vec<Card>,
    round: i32,
    trump: Suit,
    player_order: Vec<i32>,
}

#[derive(Debug)]
struct GameClient {
    hand: Vec<Card>,
    order: i32,
    trump: Suit,
    round: i32,
    state: PlayerState,
}
use std::io::{self, Read};

impl GameClient {
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

#[derive(Debug)]
enum PlayerState {
    Idle,
    RequireInput,
}

impl GameServer {
    fn get_random_card(&mut self) -> Option<Card> {
        fastrand::shuffle(&mut self.deck);
        return self.deck.pop();
    }

    fn deal(&mut self) {
        fastrand::shuffle(&mut self.deck);

        for i in 1..=self.round {
            // get random card, give to a player
            for playerid in self.player_order.clone() {
                let card = self.get_random_card().unwrap();
                let mut player: &mut Player = self.players.get_mut(playerid as usize).unwrap();
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
    let players = vec![
        Player {
            hand: vec![],
            id: 0,
        },
        Player {
            hand: vec![],
            id: 1,
        },
    ];

    let mut server = GameServer {
        players,
        deck: create_deck(),
        round: 1,
        trump: Suit::Heart,
        player_order: vec![0, 1],
    };

    server.deal();

    // println!("{:#?}", server);
    println!("{:#?}", server.player_status());
}
