export const EXAMPLE = {
  bid_order: [
    ["b", 0],
    ["asdf", 0],
  ],
  bids: { asdf: 0, b: 0 },
  curr_dealer: "asdf",
  curr_played_cards: [],
  curr_player_turn: "b",
  curr_round: 1,
  curr_winning_card: null,
  event_log: [
    {
      message: { action: "startgame" },
      timestamp: "2024-07-08T01:09:10.620Z",
      username: "b",
    },
    {
      message: { action: "startgame" },
      timestamp: "2024-07-08T01:09:11.874Z",
      username: "asdf",
    },
    {
      message: { action: { bid: 0 } },
      timestamp: "2024-07-08T01:11:21.053Z",
      username: "b",
    },
    {
      message: { action: { bid: 0 } },
      timestamp: "2024-07-08T01:11:22.153Z",
      username: "asdf",
    },
  ],
  gameplay_state: { Play: { hand_num: 1 } },
  player_order: ["asdf", "b"],
  players: {
    asdf: {
      encrypted_hand:
        "KBBbNghFX1JEVEEXVVUaFhc0GyZOXUcABBwFRRUWEAYaH1tlTgQJFBVaT0VPVQ8GFklDZhE6",
      id: "asdf",
      num_cards: 0,
      role: "Leader",
    },
    b: {
      encrypted_hand:
        "KBBbNhVODkVdXFsAAQUKARc0GyZTVhYVSlxbAxgNB0ZJSR02EAFbGQxSVVIbBR8RFklDbkURaQ==",
      id: "b",
      num_cards: 0,
      role: "Player",
    },
  },
  score: { asdf: 0, b: 0 },
  secret_key: "mysecretkey",
  system_status: ["b's turn, not asdf's turn."],
  trump: "heart",
  wins: { asdf: 0, b: 0 },
};

export const LOBBYCODE_KEY = "lobbyCodeKey";
export const USERNAME_KEY = "usernameKey";
export const SECRET_KEY = "secretKey";
