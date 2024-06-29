export const EXAMPLE = {
  bid_order: [],
  bids: {},
  curr_dealer: "ai",
  curr_played_cards: [],
  curr_player_turn: "a",
  curr_round: 1,
  curr_winning_card: null,
  event_log: [
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T20:43:47.878Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T20:46:56.845Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T20:58:11.144Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T21:14:54.421Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T21:15:53.420Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T21:17:00.301Z",
      username: "a",
    },
    {
      message: { action: "startgame", origin: { player: "a" } },
      timestamp: "2024-06-29T21:18:58.815Z",
      username: "a",
    },
  ],
  gameplay_state: "Bid",
  player_order: ["ai", "a"],
  players: {
    a: {
      encrypted_hand:
        "KBBbNhRHCQdEVhACFVUXCxU8CB9TXk5ZThVbAx4IHlBJSRozBQcRGVAMUx4MUUxUQx43",
      hand: [{ id: 26, played_by: "a", suit: "club", value: 2 }],
      id: "a",
      num_cards: 0,
      role: "Leader",
    },
    ai: {
      encrypted_hand:
        "KBBbNlFKXQUBW0saD1cBFxc0GyYXUkVWWlVFSBBDEQZRUVs7XAkKWF0TS0ZBQBkeBg5bZQRZGmo=",
      hand: [{ id: 22, played_by: "ai", suit: "diamond", value: 11 }],
      id: "ai",
      num_cards: 0,
      role: "Player",
    },
  },
  score: { a: 0, ai: 0 },
  secret_key: "mysecretkey",
  system_status: [],
  trump: "heart",
  wins: { a: 0, ai: 0 },
};

export const LOBBYCODE_KEY = "lobbyCodeKey";
export const USERNAME_KEY = "usernameKey";
export const SECRET_KEY = "secretKey";
