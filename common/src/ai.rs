// fn main() {
//     println!("Hello, world!");
// }

use std::{cmp, collections::HashMap};

use once_cell::sync::Lazy;
use tracing::info;

use crate::{Card, GameAction, GameState, GameplayState, Suit};

static CARD_VALUE_MATRIX: Lazy<HashMap<i32, i32>> = Lazy::new(|| {
    return serde_json::from_str(include_str!("../card_value_matrix.json")).unwrap();
});

fn get_bidding_strength(hand: &Vec<Card>, deck: &Vec<Card>, trump: &Suit) -> i32 {
    let min_card_value = CARD_VALUE_MATRIX[&1] as f32;
    let max_card_value = CARD_VALUE_MATRIX[&14] as f32;

    let suit_bonus = 1.5;

    let hand_value = hand
        .iter()
        .map(|x| {
            if x.suit == *trump {
                return CARD_VALUE_MATRIX[&x.value] as f32 * suit_bonus;
            }
            return CARD_VALUE_MATRIX[&x.value] as f32;
        })
        .sum::<f32>();

    let highest_value = (max_card_value * hand.len() as f32);
    let lowest_value = (min_card_value * hand.len() as f32);

    let normalized_value =
        (hand_value - (min_card_value * hand.len() as f32)) / (highest_value - lowest_value);

    let sugg_bid = (normalized_value * hand.len() as f32) as i32;

    return cmp::min(hand.len() as i32, sugg_bid);
}

pub fn get_bid(gamestate: &GameState) -> GameAction {
    let mut sugg_bid = get_bidding_strength(
        &gamestate
            .players
            .get(&gamestate.curr_player_turn.clone().unwrap())
            .unwrap()
            .hand,
        &gamestate.deck,
        &gamestate.trump,
    );
    info!(
        "Suggested bid: {sugg_bid}, hand: {:?}",
        &gamestate
            .players
            .get(&gamestate.curr_player_turn.clone().unwrap())
            .unwrap()
            .hand
    );

    let round_num = gamestate.curr_round;
    let bid_total: i32 = gamestate.bids.values().map(|x| x.unwrap()).sum::<i32>();

    if gamestate
        .curr_dealer
        .eq(&gamestate.curr_player_turn.clone().unwrap())
        && (bid_total + sugg_bid == round_num)
    {
        if sugg_bid >= 1 {
            sugg_bid -= 1;
        } else {
            sugg_bid += 1;
        }
    }

    // can't bid more than the number of rounds
    return GameAction::Bid(sugg_bid);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{create_deck, game::deal_hand};

    use super::*;

    #[test]
    fn test_get_deck_value() {
        let card_value_matrix: HashMap<i32, i32> =
            serde_json::from_str(include_str!("../card_value_matrix.json")).unwrap();

        let players = vec![
            "player1".to_string(),
            "player2".to_string(),
            "player3".to_string(),
            "player4".to_string(),
        ];
        let hand = deal_hand(5, &create_deck(), &players);

        let trump = Suit::Heart;

        let card_total = card_value_matrix.values().sum::<i32>();
        let per_card_value = card_total as f32 / 52.0;
        let min_card_value = card_value_matrix[&1] as f32;
        let max_card_value = card_value_matrix[&13] as f32;
        // let twentyfifth_card_value = card_value_matrix[&5];
        // let seventyfifth_card_value = card_value_matrix[&10];
        let suit_bonus = 1.5;

        for (player, cards) in hand {
            let mut card_strength = 0.0;
            let distribution_bonus = 0.0;
            // let mut sugg_bid = 0;
            // let average_hand_value = cards.len() as f32 * per_card_value;

            for card in &cards {
                if card.suit == trump {
                    card_strength += 1.0 * card_value_matrix[&card.value] as f32;
                } else {
                    card_strength += 0.1 * card_value_matrix[&card.value] as f32;
                }
            }

            // let mut value = ((card_strength + 0.75 * distribution_bonus) / 5.0);

            let hand_value = cards
                .iter()
                .map(|x| {
                    if x.suit == trump {
                        return card_value_matrix[&x.value] as f32 * suit_bonus;
                    }
                    return card_value_matrix[&x.value] as f32;
                })
                .sum::<f32>();

            let highest_value = (max_card_value * cards.len() as f32);
            let lowest_value = (min_card_value * cards.len() as f32);

            // println!("Hand value: {hand_value}, highest: {highest_value}, lowest: {lowest_value}");
            let normalized_value = (hand_value - (min_card_value * cards.len() as f32))
                / (highest_value - lowest_value);

            let sugg_bid = (normalized_value * cards.len() as f32) as i32;

            // let cardstext = cards
            //     .iter()
            //     .map(|x| x.to_string())
            //     .collect::<Vec<String>>()
            //     .join(" ");
            // println!(
            //     "Player: {player}, Value: {value}, Norm: {normalized_value} -> {sugg_bid}, cards: {cardstext}"
            // );
            // println!("Player: {player}, Strength: {card_strength}, cards: {cards:?}");
        }
    }
}
