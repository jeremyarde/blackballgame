import { useState, useEffect } from "react";
import "./App.css";
import React from "react";
import { EXAMPLE, LOBBYCODE_KEY, SECRET_KEY, USERNAME_KEY } from "./constant";

const urlMap = {
  local: "ws://127.0.0.1:3000/ws",
  localNetwork: `ws://${window.location.href.split("http://")[1]}ws`,
};

const TEST = false;
const EXAMPLE_USERNAME = "a";

const enum GAME_STATE {
  LOBBY,
  GAME,
  START,
}

function App() {
  // connection to server state
  const [username, setUsername] = useState(TEST ? EXAMPLE_USERNAME : "");
  const [lobbyCode, setLobbyCode] = useState("");
  const [secret, setSecret] = useState("");
  const [appState, setAppState] = useState(GAME_STATE.START);

  const [messages, setMessages] = useState([]);
  // const [hideState, setHideState] = useState(true);

  const [url, setUrl] = useState(urlMap.local);
  const [ws, setWs] = useState<WebSocket | undefined>(undefined);
  const [gamestate, setGamestate] = useState(TEST ? EXAMPLE : undefined);

  const [connected, setConnected] = useState(false);
  const [debug, setDebug] = useState(false);

  useEffect(() => {
    if (localStorage.getItem(USERNAME_KEY)) {
      setUsername(localStorage.getItem(USERNAME_KEY));
    }
    if (localStorage.getItem(LOBBYCODE_KEY)) {
      setLobbyCode(localStorage.getItem(LOBBYCODE_KEY));
    }
    if (localStorage.getItem(SECRET_KEY)) {
      setSecret(localStorage.getItem(SECRET_KEY));
    }

    const ws = new WebSocket(url);
    setWs(ws);
    return () => {
      ws.close();
    };
  }, [url]);

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

        setConnectionDetails({
          currLobbyCode: lobbyCode,
          currUsername: username,
          currSecret: parseddata.client_secret,
        });
      }

      setMessages((prevMessages) => [...prevMessages, parseddata]);
      // checkIfStateChanged(parseddata);

      // setAppState(GAME_STATE.GAME);
      setGamestate(parseddata);

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
    if (ws) {
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
        action: "startgame",
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
          <div className="flex flex-col items-center justify-center align-middle border rounded-md bg-fuchsia-200 border-input bg-background ring-offset-background">
            <div>
              <label>Lobby code: </label>
              <input
                className="w-24 h-10 border rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                type="text"
                onChange={(evt) => setLobbyCode(evt.target.value)}
                value={lobbyCode}
              ></input>
            </div>
            <div>
              <label>Name: </label>
              <input
                type="text"
                className="w-24 h-10 border rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                onChange={(evt) => setUsername(evt.target.value)}
                value={username}
              ></input>
            </div>
            <button
              className="w-24 h-10 border border-solid rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
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
            <div className="flex flex-col w-full p-4 ">
              <div className="w-1/4 border border-solid bg-cyan-200">
                {gamestate?.players &&
                  Object.entries(gamestate.players).map(([player, details]) => {
                    if (player != username) {
                      return (
                        <div className="flex flex-col w-full">
                          <label className="">
                            <b>{player}</b>
                          </label>
                          <ul>
                            <li>Cards: {details.hand.length}</li>
                            <li>Wins: {gamestate.wins[player]}</li>
                            <li>Bids: {gamestate.bids[player]}</li>
                          </ul>
                        </div>
                      );
                    }
                  })}
              </div>
              <div className="bg-green-500">
                <h3>Played Cards</h3>
                <CardArea
                  cards={gamestate ? gamestate.curr_played_cards : []}
                  playCard={playCard}
                />
              </div>
              <div className="flex">
                <div
                  className={`outline-4 m-2 w-full outline bg-slate-400 flex flex-col ${
                    gamestate?.curr_player_turn === username
                      ? "outline-yellow-300"
                      : ""
                  }`}
                >
                  <h3>Your hand</h3>
                  <CardArea
                    cards={
                      gamestate?.players &&
                      gamestate.players[username] &&
                      gamestate?.players[username].hand
                        ? gamestate?.players[username].hand
                        : []
                    }
                    playCard={playCard}
                  />
                  {gamestate && gamestate.gameplay_state == "Bid" && (
                    <div className="flex justify-center m-4">
                      <>
                        <label>Bid</label>
                        <ol className="flex flex-row">
                          {gamestate?.players &&
                          gamestate.players[username] &&
                          gamestate?.players[username].hand
                            ? bids.map((i) => {
                                return (
                                  <li key={i}>
                                    <button
                                      className="w-24 h-10 border border-solid rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                      onClick={() => sendBid(i)}
                                    >
                                      {i}
                                    </button>
                                  </li>
                                );
                              })
                            : "Failed"}
                        </ol>
                      </>
                    </div>
                  )}
                </div>
              </div>
            </div>
            {/* </div> */}
            {gamestate && gamestate.players && (
              <div className="flex flex-col bg-orange-200 border border-solid rounded-md bg-background">
                <div className="flex flex-col">
                  <h2>Game details</h2>
                  <label>TRUMP: {gamestate.trump}</label>
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

                <div className="flex flex-col bg-cyan-200">
                  <h2>Hand details</h2>
                  <div>{displayObject(gamestate.trump)}</div>
                  <div>{displayObject(gamestate.system_status)}</div>
                  <div>
                    wins:{" "}
                    {gamestate.wins && gamestate.wins[username]
                      ? displayObject(gamestate.wins[username])
                      : "N/A"}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
        <button
          className="bg-red-500"
          onClick={() => {
            console.log(debug);
            debug ? setDebug(false) : setDebug(true);
          }}
        >
          Enable debug mode
        </button>
        {debug && gamestate
          ? Object.entries(gamestate.players).map(
              ([playername, playerdetails]) => {
                console.log(
                  "jere/ playername, details",
                  playername,
                  playerdetails
                );

                return (
                  <>
                    <label>{playername}</label>
                    <div className="flex flex-row p-2">
                      {playerdetails.hand.map((card) => {
                        return (
                          <Card key={card.id} card={card} playCard={playCard} />
                        );
                      })}
                    </div>
                  </>
                );
              }
            )
          : ""}
      </div>
    </>
  );
}

// import Club from "./assets/club.svg";
import Club from "./assets/club.svg";
import Diamond from "./assets/diamond.svg";
import Heart from "./assets/heart.svg";
import Spade from "./assets/spade.svg";

function CardArea({ cards = [], playCard }) {
  let sortedCards = cards.sort((a, b) => a.id - b.id);
  return (
    <div className="flex flex-row justify-center space-x-2">
      {sortedCards
        ? sortedCards.map((card) => {
            return <Card key={card.id} card={card} playCard={playCard} />;
          })
        : ""}
    </div>
  );
}

function Card({ card, playCard }) {
  let suitDisplay = {
    spade: { src: Spade },
    diamond: { src: Diamond },
    club: { src: Club },
    heart: { src: Heart },
  };

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
        className="flex h-[140px] w-[100px] items-center justify-center rounded-lg bg-white shadow-lg "
        onMouseDown={() => playCard(card)}
      >
        <div className="flex flex-col items-center gap-2">
          <span className="text-xl font-bold">{cardValue[card.value]}</span>
          <span className="font-medium text-md">
            <img className={`size-14`} src={suitDisplay[card.suit].src}></img>
          </span>
        </div>
      </div>
    </>
  );
}

export interface GameState {
  bids: Bids;
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
  gameplay_state: string;
  system_status: any[];
  trump: string;
  wins: Wins;
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
