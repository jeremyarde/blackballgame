import { useState, useEffect } from "react";
import "./App.css";
import React from "react";
import { EXAMPLE, LOBBYCODE_KEY, SECRET_KEY, USERNAME_KEY } from "./constant";

const TEST = false;
const EXAMPLE_USERNAME = "a";

const enum GAME_STATE {
  LOBBY,
  GAME,
  START,
}
import Club from "./assets/club.svg";
import Diamond from "./assets/diamond.svg";
import Heart from "./assets/heart.svg";
import Spade from "./assets/spade.svg";
import NoTrump from "./assets/notrump.svg";

// const suitDisplay = {
//   spade: { src: Spade },
//   diamond: { src: Diamond },
//   club: { src: Club },
//   heart: { src: Heart },
//   notrump: { src: NoTrump },
// };

export function Suit({ cardsuit }) {
  console.log("jere cardsuit: ", cardsuit);
  let suit = {
    notrump: <NoTrump className="size-14"></NoTrump>,
    heart: <Heart className="size-14"></Heart>,
    diamond: <Diamond className="size-14"></Diamond>,
    club: <Club className="size-14"></Club>,
    spade: <Spade className="size-14"></Spade>,
  }[cardsuit];

  console.log("jere/ suit: ", suit);

  return <>{suit}</>;
}

// const suitDisplay = {
//   spade: { src: "./assets/spade.svg" },
//   diamond: { src: "./assets/diamond.svg" },
//   club: { src: "./assets/club.svg" },
//   heart: { src: "./assets/heart.svg" },
//   notrump: { src: "./assets/notrump.svg" },
// };

let mode = import.meta.env.MODE;
// mode = "production";

const ws_url = {
  development: `ws://${window.location.hostname}:8080/ws`,
  // production: `wss://${window.location.host}/ws`,
  production: "wss://blackballgame-blackballgame-server.onrender.com/ws",
};
// const ws_url = `ws://${window.location.hostname}${
//   import.meta.env.MODE === "development" ? ":8080" : ""
// }/ws`;

// const buttonStyle =
//   "w-24 h-10 border border-solid rounded-md  bg-background  ";

function App() {
  console.log(`Mode: ${mode}`);
  console.log(
    `Host: ${window.location.host}, hostname: ${window.location.hostname}`
  );
  console.log(`WS urls: ${JSON.stringify(ws_url)}`);

  // connection to server state
  const [username, setUsername] = useState(TEST ? EXAMPLE_USERNAME : "");
  const [lobbyCode, setLobbyCode] = useState("");
  const [secret, setSecret] = useState("");
  const [appState, setAppState] = useState(GAME_STATE.START);
  const [playerHand, setPlayerHand] = useState([]);

  const [messages, setMessages] = useState([]);
  // const [hideState, setHideState] = useState(true);

  const [url, setUrl] = useState(ws_url[mode]);
  const [ws, setWs] = useState<WebSocket | undefined>(undefined);
  const [gamestate, setGamestate] = useState<GameState | undefined>(
    TEST ? EXAMPLE : undefined
  );

  const [connected, setConnected] = useState(false);
  const [debug, setDebug] = useState(false);

  useEffect(() => {
    if (localStorage.getItem(USERNAME_KEY)) {
      setUsername(localStorage.getItem(USERNAME_KEY) || "");
    }
    if (localStorage.getItem(LOBBYCODE_KEY)) {
      setLobbyCode(localStorage.getItem(LOBBYCODE_KEY) || "");
    }
    if (localStorage.getItem(SECRET_KEY)) {
      setSecret(localStorage.getItem(SECRET_KEY) || "");
    }

    console.log("Connecting to WS at ", url);
    const ws = new WebSocket(url);
    setWs(ws);
    return () => {
      ws.close();
    };
  }, [url]);

  function xorEncryptDecrypt(data, key) {
    console.log("xor function: ", data, key);
    return data.map((byte, index) => byte ^ key.charCodeAt(index % key.length));
  }

  // when gamestate updates
  useEffect(() => {
    function decryptHand(ciphertext) {
      console.log("Attempting to decrypt: ", ciphertext);
      let encoder = new TextEncoder();

      let base64_decoded = atob(ciphertext);

      let secret_data = xorEncryptDecrypt(
        encoder.encode(base64_decoded),
        secret
      );
      const secretDataString = new TextDecoder().decode(secret_data);

      return JSON.parse(secretDataString);
    }

    if (
      gamestate?.players &&
      gamestate.players[username] &&
      gamestate.players[username].encrypted_hand
    ) {
      let playerdetails = gamestate.players[username];
      console.log("jere/ playerdetails: ", playerdetails);
      let hand = decryptHand(playerdetails.encrypted_hand);
      console.log("set playing hand");
      setPlayerHand(hand);
      console.log("result of decrypt: ", hand);
    }
  }, [gamestate]);

  useEffect(() => {
    if (!ws) {
      console.log("Already connected");
      return;
    }

    ws.onopen = () => {
      setConnected(true);
      console.log("WebSocket connected");
    };

    ws.onmessage = (message) => {
      console.log(`Message from server: ${message.data}`);
      let parseddata = JSON.parse(message.data);

      if (parseddata.client_secret) {
        console.log("setting secret value", parseddata.client_secret);
        setSecret(parseddata.client_secret);

        console.log("setting connection details in onmessage: ", {
          currLobbyCode: lobbyCode,
          currUsername: username,
          currSecret: parseddata.client_secret,
        });
        setConnectionDetails({
          currLobbyCode: lobbyCode,
          currUsername: username,
          currSecret: parseddata.client_secret,
        });
      }

      setMessages((prevMessages) => [...prevMessages, parseddata]);

      if (parseddata.players) {
        setGamestate(parseddata);
      }

      console.log("all messages");
      console.log(messages);
    };

    ws.onclose = () => {
      setConnected(false);
      console.log("WebSocket disconnected");
      setMessages([]);

      // setWs(null);
    };

    return () => {
      ws.close();
    };
  }, [ws]); // adding messages causes the ws to close

  function sendMessage(message) {
    if (ws && ws.readyState === 1) {
      ws.send(JSON.stringify(message));
      // ws.send(message);
    }
  }

  function setConnectionDetails({ currUsername, currLobbyCode, currSecret }) {
    console.log("Setting connection details in local storage: ", {
      currUsername,
      currLobbyCode,
      secret,
    });
    if (currUsername && currUsername !== "") {
      localStorage.setItem(USERNAME_KEY, currUsername);
    }
    if (currLobbyCode && currLobbyCode !== "") {
      localStorage.setItem(LOBBYCODE_KEY, currLobbyCode);
    }
    if (currSecret && currSecret !== "") {
      localStorage.setItem(SECRET_KEY, currSecret);
    }
  }

  function displayObject(obj) {
    return (
      <code>
        <pre>{JSON.stringify(obj, null, 2)}</pre>
      </code>
    );
  }

  function connectToLobby() {
    var connectMessage = {
      username: username,
      channel: lobbyCode,
      secret: secret,
    };
    console.log("Sending connection request:", connectMessage);
    sendMessage(connectMessage);

    console.log(
      "Setting lobbycode and username:",
      JSON.stringify({
        username: username,
        channel: lobbyCode,
        secret: secret,
      })
    );

    setConnectionDetails({
      currUsername: username,
      currLobbyCode: lobbyCode,
      currSecret: secret,
    });

    setAppState(GAME_STATE.LOBBY);
  }

  const startGame = () => {
    let message = {
      username: username,
      message: {
        action: {
          startgame: {
            rounds: 9,
            deterministic: false,
          },
        },
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    setAppState(GAME_STATE.GAME);
    sendMessage(message);
  };

  const dealCard = () => {
    let message = {
      username: username,
      message: {
        action: "deal",
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };

  const playCard = (card) => {
    let message = {
      username: username,
      message: {
        action: {
          playcard: {
            id: card.id,
            suit: card.suit,
            value: card.value,
            played_by: username,
          },
        },
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };

  const sendBid = (bidValue) => {
    console.log("sendBid: ", bidValue);
    let message = {
      username: username,
      message: {
        action: {
          bid: bidValue,
        },
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };

  const sendAck = () => {
    console.log("sendAck");
    let message = {
      username: username,
      message: {
        action: "ack",
        // origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };
  let bids = [];
  if (gamestate?.curr_round) {
    for (let i = 0; i < gamestate.curr_round + 1; i++) {
      bids.push(i);
    }
  }

  return (
    <>
      <div className="flex flex-col w-full h-full">
        {appState === GAME_STATE.START && (
          <div className="flex flex-col items-center justify-center align-middle border rounded-md bg-fuchsia-200 bg-background ring-offset-background">
            <div>
              <label>Lobby code: </label>
              <input
                className="w-24 h-10 border rounded-md bg-background "
                type="text"
                onChange={(evt) => setLobbyCode(evt.target.value)}
                value={lobbyCode}
              ></input>
            </div>
            <div>
              <label>Name: </label>
              <input
                type="text"
                className="w-24 h-10 border rounded-md bg-background "
                onChange={(evt) => setUsername(evt.target.value)}
                value={username}
              ></input>
            </div>
            <button
              className="w-24 h-10 border border-solid rounded-md bg-background "
              onClick={connectToLobby}
            >
              Connect
            </button>
          </div>
        )}
        {appState === GAME_STATE.LOBBY && (
          <div>
            {"current players listed here"}
            <button className="p-2 m-1 outline" onClick={startGame}>
              Start game
            </button>
          </div>
        )}
        {appState === GAME_STATE.GAME && (
          <div className="flex w-full bg-green-300">
            <div className="flex flex-col w-full p-4">
              <div className="w-1/4 bg-cyan-200">
                {gamestate?.players &&
                  Object.entries(gamestate.players).map(([player, details]) => {
                    if (player != username) {
                      return (
                        <div className="flex flex-col w-full ">
                          <label className="w-full text-center bg-cyan-400">
                            <b>{player}</b>
                          </label>
                          <ul className="flex flex-col">
                            <li className="flex flex-row justify-between">
                              <div>Cards left</div>
                              <div>{details.num_cards}</div>
                            </li>
                            <li className="flex flex-row justify-between">
                              <div>Hands won</div>
                              <div>{gamestate.wins[player]}</div>
                            </li>
                            <li className="flex flex-row justify-between">
                              <div>Bid</div>
                              <div>
                                {gamestate.bids[player] ??
                                  `${gamestate.curr_player_turn}'s turn to bid`}
                              </div>
                            </li>
                            <li className="flex flex-row justify-between">
                              <div>Score</div>
                              <div>{gamestate.score[player]}</div>
                            </li>
                          </ul>
                        </div>
                      );
                    }
                  })}
              </div>
              {gamestate &&
                (gamestate.gameplay_state["Play"] ||
                  gamestate.gameplay_state["PostRound"] ||
                  gamestate.gameplay_state["PostHand"]) && (
                  <div className="bg-green-500 h-1/4">
                    <h3>Played Cards</h3>
                    <CardArea
                      cards={gamestate ? gamestate.curr_played_cards : []}
                      playCard={playCard}
                      gamestate={gamestate}
                    />
                  </div>
                )}
              <div className="flex">
                <div
                  className={`outline-4 m-2 w-full outline flex flex-col ${
                    gamestate?.curr_player_turn === username
                      ? "outline-red-500 bg-red-300"
                      : ""
                  }`}
                >
                  {gamestate && (
                    <CardArea
                      cards={
                        // gamestate?.players &&
                        // gamestate.players[username] &&
                        // gamestate?.players[username].hand
                        //   ? gamestate?.players[username].hand
                        playerHand ? playerHand : []
                      }
                      playCard={playCard}
                      gamestate={gamestate}
                    />
                  )}
                  {gamestate && gamestate.curr_player_turn == username && (
                    <h3 className="flex self-center mt-3">Your turn</h3>
                  )}
                  {gamestate && gamestate.gameplay_state == "Bid" && (
                    <div className="flex justify-center m-4">
                      <>
                        <label>Bid</label>
                        <ol className="flex flex-row">
                          {bids.map((i) => {
                            return (
                              <li key={i}>
                                <button
                                  className="w-24 h-10 border border-solid rounded-md bg-slate-100"
                                  onClick={() => sendBid(i)}
                                >
                                  {i}
                                </button>
                              </li>
                            );
                          })}
                        </ol>
                      </>
                    </div>
                  )}
                  {gamestate && gamestate.gameplay_state === "PostRound" && (
                    <div className="w-full p-4 text-center bg-red-300">
                      {gamestate.curr_player_turn === username ? (
                        <button
                          className="p-2 bg-red-400 border border-red-500 border-solid rounded-md"
                          onClick={dealCard}
                        >
                          Start next round
                        </button>
                      ) : (
                        <div className="p-2">
                          Waiting for dealer to start next round
                        </div>
                      )}
                    </div>
                  )}
                  {gamestate && gamestate.gameplay_state["PostHand"] && (
                    <div className="w-full p-4 text-center bg-red-300">
                      {
                        <>
                          <button
                            className="p-2 bg-red-400 border border-red-500 border-solid rounded-md"
                            onClick={sendAck}
                          >
                            Go to next hand
                          </button>
                          <button
                            className="p-2 bg-red-400 border border-red-500 border-solid rounded-md"
                            onClick={dealCard}
                          >
                            Start next round
                          </button>
                        </>
                      }
                    </div>
                  )}
                </div>
              </div>
              <div className="w-full bg-cyan-200">
                {gamestate?.players && username && (
                  <div className="flex flex-row w-full ">
                    <label className="text-center bg-cyan-400">
                      <b>{`${username} (you)`}</b>
                    </label>
                    <ul className="flex flex-row items-stretch content-between justify-between">
                      <li className="">
                        <div>Cards left</div>
                        <div>{gamestate.players[username].num_cards}</div>
                      </li>
                      <li className="">
                        <div>Hands won</div>
                        <div>{gamestate.wins[username]}</div>
                      </li>
                      <li className="">
                        <div>Bid</div>
                        <div>
                          {gamestate.bids[username] ??
                            `${gamestate.curr_player_turn}'s turn to bid`}
                        </div>
                      </li>
                      <li className="">
                        <div>Score</div>
                        <div>{gamestate.score[username]}</div>
                      </li>
                    </ul>
                  </div>
                )}
              </div>
            </div>
            {gamestate && gamestate.players && (
              <div className="flex flex-col bg-orange-200 border border-solid rounded-md shadow-lg drop-shadow-xlbg-background">
                <div className="flex flex-col">
                  <h2 className="outline">Game details</h2>
                  <label>
                    <b>State: {displayObject(gamestate.gameplay_state)}</b>
                  </label>
                  <label>
                    Trump suit:
                    <Suit cardsuit={gamestate.trump}></Suit>
                  </label>
                  <label>Round: {gamestate.curr_round}</label>
                  <label>Player Turn: {gamestate.curr_player_turn}</label>
                  <ul className="flex flex-row space-x-2">
                    <label>Player order:</label>
                    {gamestate &&
                      gamestate.player_order &&
                      gamestate.player_order.map((playername) => {
                        return (
                          <li className="flex flex-row" key={playername}>
                            <label
                              className={
                                playername === gamestate.curr_player_turn
                                  ? "bg-green-400"
                                  : ""
                              }
                            >
                              {playername}
                              {playername === gamestate.curr_player_turn
                                ? "<-- "
                                : ""}
                            </label>
                          </li>
                        );
                      })}
                  </ul>
                  <div>Dealer: {gamestate.curr_dealer}</div>
                  <div>
                    Hands won: {gamestate.wins[username]}/
                    {gamestate.bids[username] ?? "0"}
                  </div>
                  <label>
                    Current hand #:{" "}
                    {gamestate.wins[username]
                      ? gamestate.wins[username] + 1
                      : 0}
                  </label>
                  <label>
                    Player bids:
                    {gamestate.bids ? displayObject(gamestate.bids) : "No bids"}
                  </label>
                  <label>
                    Player scores:
                    {gamestate.bids
                      ? displayObject(gamestate.score)
                      : "No bids"}
                  </label>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </>
  );
}

function CardArea({ cards = [], playCard, gamestate }) {
  console.log("jere/ cards", cards);
  let sortedCards = cards.sort((a, b) => a.id - b.id);
  let curr_winning_card = gamestate?.curr_winning_card;
  return (
    <div className="flex flex-row justify-center space-x-2">
      {sortedCards
        ? sortedCards.map((card) => {
            return (
              <Card
                key={card.id}
                card={card}
                playCard={playCard}
                isWinning={curr_winning_card?.id === card.id}
              />
            );
          })
        : ""}
    </div>
  );
}

function Card({ card, playCard, isWinning }) {
  let cardValue = {
    14: "A",
    13: "K",
    12: "Q",
    11: "J",
    10: 10,
    9: 9,
    8: 8,
    7: 7,
    6: 6,
    5: 5,
    4: 4,
    3: 3,
    2: 2,
  };

  return (
    <>
      <div
        key={card.id}
        className={`flex h-[140px] w-[100px] items-center justify-center rounded-lg bg-white shadow-lg ${
          isWinning ? "bg-yellow-200" : ""
        }`}
        onMouseDown={() => playCard(card)}
      >
        <div className={`flex flex-col items-center gap-2`}>
          <span className="text-xl font-bold">{cardValue[card.value]}</span>
          <span className="font-medium text-md">
            <Suit cardsuit={card.suit}></Suit>
          </span>
        </div>
      </div>
    </>
  );
}

export interface GameState {
  bids: Bids;
  player_order: string[];
  curr_played_cards: CurrPlayedCard[];
  curr_player_turn: string;
  curr_round: number;
  curr_winning_card: any;
  dealing_order: string[];
  deck: Deck[];
  event_log: any[];
  play_order: string[];
  players: Players;
  score: Score;
  gameplay_state: object | string;
  system_status: any[];
  trump: string;
  wins: Wins;
}

interface PlayState {
  hand_num: number;
}

export interface Bids {}

export interface CurrPlayedCard {
  id: number;
  played_by: any;
  suit: string;
  value: number;
}

export interface Deck {
  id: number;
  played_by: any;
  suit: string;
  value: number;
}

export interface Players {
  a: A;
  q: Q;
}

export interface A {
  hand: Hand[];
  id: string;
  order: number;
  role: string;
  round: number;
  state: string;
  trump: string;
}

export interface Hand {
  id: number;
  played_by: any;
  suit: string;
  value: number;
}

export interface Q {
  hand: Hand2[];
  id: string;
  order: number;
  role: string;
  round: number;
  state: string;
  trump: string;
}

export interface Hand2 {
  id: number;
  played_by?: string;
  suit: string;
  value: number;
}

export interface Score {
  a: number;
  q: number;
}

export interface Wins {
  a: number;
  q: number;
}

export default App;
