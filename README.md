# BlackBall

Website | https://jeremyarde.github.io/blackballgame/

This is a simple card game played with 1 deck and 2-10+ players.

## How to play

The game is played over multiple rounds, starting with dealing 1 card to each player and going up to as many rounds as possible given the amount of players in the game. 
  E.g. Round 1 = deal 1 card each, Round 6 = deal 6 cards to each player. 
Each round there is a designated "trump" suit. The trump rotates through following the order of Hearts, Diamonds, Clubs, Spades, and a special "No Trump" round.
Once the dealer has dealt each player the number of cards equal to the round number, the player after the dealer will bid a number of "hands" they will win during the round. 
A "hand" is one full round of each player playing a single card in order.
The dealer bids last, and is not able to bid a number that will cause the total number of bids to equal the round number. This is so each round at least one player needs to not win what they bid.
The player who bid the highest number gets to play the first card in the first hand, and then the winner of each hand will play the first card for the next hand, until the round is over and all cards have been played.
The first card played each hand will determine the suit that everyone has to play. If a player does not have the suit, they can play another suit.
The winning card of a hand is either:
  1) highest card with the trump suit
  2) the highest card of the first played cards suit.
Trump cards take priority, but not each hand will contain a card with trump suit.

No Trump rounds are special, in that the first card played each round is considered Trump. 
E.g. If the first played card is an Ace, then this player will win the round by default. If They played a King, then only an ace of the same suit an beat the King to win the round.

Once the round is over, players count up each hand they won, and if it equals the bid value they set before the round started, they win 10 points + the bid value.
The dealer moves to the next in line player, and deals out cards equal to the next round number, and the bidding starts.

## Example round

- Round 4
- 3 players
- Trump of Spades

First player to bid has 2 Spade cards, and 2 Hearts, they bid 2.
Second player has 0 Spades, but an Ace, they bid 1.
Dealer has 1 Spade, but they can't bid 1 because bids would equal the round number, so they bid 2.

Hand 1: Player 1 wins hand 1 with a Heart.
  Wins: P1 = 1, P2 = 0, Dealer = 0
Hand 2: Player 1 starts hand 2 with another Heart, Player 2 plays a non-heart card, Dealer has no more hearts, but needs wins so they play their spade, to win the hand.
  Wins: P1 = 1, P2 = 0, Dealer = 1
Hand 3: Dealer plays a King of Diamonds, Player 1 has no diamonds and plays a spade, Player 2 has diamond cards still, so they play the smallest one because they need a win still.
  Wins: P1 = 2, P2 = 0, Dealer = 1
Hand 4: Player 1 plays a small heart card, Player 2 only has their Ace of non hearts to play, dealer has no hearts left and plays a non trump card. Player 1 wins because nobody played trump or a higher heart card.
  Wins: P1 = 3, P2 = 0, Dealer = 1

For this round, every player would get a "blackball", meaning they win 0 points on the round, because nobody bid the correct number of hands they would win during the round.


