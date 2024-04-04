

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
    player_order: Vec<i32>
}

impl GameServer {
    fn get_random_card(&self) -> &Card {
        let random_pos = 0;
        return self.deck.get(random_pos).unwrap();
    }
    fn deal(&mut self) {
        for i in 1..self.round {
            // get random card, give to a player
            for playerid in &self.player_order {
                let card = &self.get_random_card();
                let mut player: &mut Player = self.players.get_mut(*playerid as usize).unwrap();
                // .get(playerid).unwrap();
                player.hand.push(card.clone().to_owned());
            }
        }
    }
}

fn create_deck() -> Vec<Card> {

    let mut cards = vec![];
    
    // 14 = Ace
    let mut cardid = 0;
    for value in 2..=14 {

        cards.push(Card {id: cardid, suit: Suit::Heart, value: value});
        cards.push(Card {id: cardid + 1, suit: Suit::Diamond, value: value});
        cards.push(Card {id: cardid + 2, suit: Suit::Club, value: value});
        cards.push(Card {id: cardid + 3, suit: Suit::Spade, value: value});
        cardid += 4;

    }
    
    return cards;
}

fn main() {
    let players = vec![Player {hand: vec![], id: 0}];

    let server = GameServer {
        players,
        deck: create_deck(),
        round: 0,
        trump: Suit::Heart,
        player_order: vec![],
    };

    println!("{:#?}", server);
}
