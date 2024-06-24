import { useState, useEffect } from "react";
import "./App.css";
import React from "react";
import { EXAMPLE, LOBBYCODE_KEY, SECRET_KEY, USERNAME_KEY } from "./constant";

function App() {
  // connection to server state
  const [username, setUsername] = useState("");
  const [lobbyCode, setLobbyCode] = useState("");
  const [secret, setSecret] = useState("");

  const [messages, setMessages] = useState([]);
  const [hideState, setHideState] = useState(true);

  const [url, setUrl] = useState("ws://127.0.0.1:3000/ws");
  const [ws, setWs] = useState(undefined);
  // const [gamestate, setGamestate] = useState(EXAMPLE);
  const [gamestate, setGamestate] = useState();

  // const [bid, setBid] = useState();
  const [connected, setConnected] = useState(false);
  const [debug, setDebug] = useState(false);

  // const [handCards, setHandCards] = useState([]);
  // const [playAreaCards, setPlayAreaCards] = useState([]);

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
          <button className="p-2 m-1 outline" onClick={startGame}>
            Start game
          </button>
        </div>
        <div className="flex w-full bg-green-300 ">
          {/* <div className="bg-green-300 "> */}
          <div className="flex flex-col w-full p-4">
            <div>
              <h3>Played Cards</h3>
              <div className="flex flex-row justify-center">
                {gamestate?.curr_played_cards
                  ? gamestate.curr_played_cards.map((card) => {
                      return (
                        <div
                          key={card.id}
                          className="flex h-[140px] w-[100px] items-center justify-center rounded-lg bg-white shadow-lg "
                          onMouseDown={() => playCard(card)}
                        >
                          <div className="flex flex-col items-center gap-2">
                            <span className="text-xl font-bold">
                              {card.value}
                            </span>
                            <span className="font-medium text-md">
                              {card.suit}
                            </span>
                          </div>
                        </div>
                      );
                    })
                  : ""}
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
                  <div className="flex flex-row justify-center">
                    {gamestate?.players &&
                    gamestate.players[username] &&
                    gamestate?.players[username].hand
                      ? gamestate.players[username].hand.map((card) => {
                          return (
                            <div
                              key={card.id}
                              className="flex h-[140px] w-[100px] items-center justify-center rounded-lg bg-white shadow-lg "
                              onMouseDown={() => playCard(card)}
                            >
                              <div className="flex flex-col items-center gap-2">
                                <span className="text-xl font-bold">
                                  {card.value}
                                </span>
                                <span className="font-medium text-md">
                                  {card.suit}
                                </span>
                              </div>
                            </div>
                          );
                        })
                      : ""}
                  </div>
                  {gamestate && gamestate.state == "Bid" && (
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
                  {gamestate.wins[username] ? gamestate.wins[username] + 1 : 0}
                </label>
                <label>
                  Player bids:
                  {gamestate.bids ? displayObject(gamestate.bids) : "No bids"}
                </label>
                <label>
                  Player scores:
                  {gamestate.bids ? displayObject(gamestate.score) : "No bids"}
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
        {/* {hideState && (
          <div style={{ justifyContent: "left", textAlign: "left" }}>
            <div>{displayObject(gamestate)}</div>
          </div>
        )} */}
        <button
          className="bg-red-500"
          onClick={() => {
            console.log(debug);
            debug ? setDebug(false) : setDebug(true);
          }}
        >
          Enable debug mode
        </button>
        {debug
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
                    <div className="flex flex-row">
                      {playerdetails.hand.map((card) => {
                        return (
                          <>
                            <div
                              key={card.id}
                              className="flex h-[140px] w-[100px] items-center justify-center rounded-lg bg-white shadow-lg "
                              onMouseDown={() => playCard(card)}
                            >
                              <div className="flex flex-col items-center gap-2">
                                <span className="text-xl font-bold">
                                  {card.value}
                                </span>
                                <span className="font-medium text-md">
                                  {card.suit}
                                </span>
                              </div>
                            </div>
                          </>
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

export default App;
