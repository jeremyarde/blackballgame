// fn main() {
//     println!("Hello, world!");
// }

use common::{GameAction, GameState, GameplayState};
use tracing::info;

pub fn decide_action(
    gamestate: &GameState,
    username: String,
    secret_key: String,
) -> Option<GameAction> {
    let action = match &gamestate.gameplay_state {
        common::GameplayState::Bid => get_bid(gamestate),
        common::GameplayState::Pregame => return None,
        common::GameplayState::PostHand(ps) => return None,
        common::GameplayState::Play(ps) => {
            // let player = gamestate.players.get(&self.username).unwrap();
            let cards = GameState::decrypt_player_hand(
                gamestate
                    .players
                    .get(&username)
                    .unwrap()
                    .encrypted_hand
                    .clone(),
                &secret_key,
            );
            info!("Cards: {:?}", cards);
            GameAction::PlayCard(cards.get(0).unwrap().clone())
        }
        GameplayState::PostRound => GameAction::Deal,
        GameplayState::End => GameAction::Ack,
    };
    Some(action)
}

fn get_bid(gamestate: &GameState) -> GameAction {
    let round_num = gamestate.curr_round;
    let bid_total: i32 = gamestate.bids.values().map(|x| x.unwrap()).sum::<i32>();
    let total_players = gamestate.players.len();
    // let bid_order = gamestate.bid_order.clone();

    let mut my_bid = 0;

    if round_num == bid_total {
        my_bid = 1;
    }

    return GameAction::Bid(my_bid);
}
