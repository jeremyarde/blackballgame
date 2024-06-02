import { useState, useEffect } from "react";
import "./App.css";
import React from "react";
import { EXAMPLE } from "./constant";

function App() {
  const [username, setUsername] = useState("a");
  const [lobbyCode, setLobbyCode] = useState("");
  const [messages, setMessages] = useState([]);
  const [hideState, setHideState] = useState(false);

  const [url, setUrl] = useState("ws://127.0.0.1:3000/ws");
  const [ws, setWs] = useState();
  const [gamestate, setGamestate] = useState(EXAMPLE);
  const [bid, setBid] = useState();

  // const [handCards, setHandCards] = useState([]);
  // const [playAreaCards, setPlayAreaCards] = useState([]);

  useEffect(() => {
    const ws = new WebSocket(url);
    setWs(ws);
    return () => {
      ws.close();
    };
  }, [url]);

  useEffect(() => {
    if (!ws) return;

    ws.onopen = () => {
      console.log("WebSocket connected");
    };

    ws.onmessage = (message) => {
      console.log(`Message from server: ${message.data}`);
      setMessages((prevMessages) => [
        ...prevMessages,
        JSON.parse(message.data),
      ]);

      setGamestate(JSON.parse(message.data));

      console.log("all messages");
      console.log(messages);
      // setMessages((prevMessages) => [...prevMessages, message.data]);
    };

    ws.onclose = () => {
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

  function displayObject(obj) {
    return <div>{JSON.stringify(obj)}</div>;
  }

  function connectToLobby() {
    var connectMessage = JSON.stringify({
      username: username,
      channel: lobbyCode,
    });
    console.log(connectMessage);
    ws.send(connectMessage);
    // sendMessage(connectMessage);
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
  for (let i = 0; i < gamestate.curr_round + 1; i++) {
    bids.push(i);
  }

  return (
    <>
      {hideState && (
        <div style={{ justifyContent: "left", textAlign: "left" }}>
          {gamestate && (
            <ul>
              <li>
                <b>Bids: </b>
                {displayObject(gamestate.bids)}
              </li>
              <li>
                <b>Player Turn: </b>
                {gamestate.curr_player_turn}
              </li>
              <li>
                <b>Round: </b>
                {gamestate.curr_round}
              </li>
              <li>
                <b>Winning card: </b>
                {displayObject(gamestate.curr_winning_card)}
              </li>
              <li>
                <b>Deal order: </b>
                {gamestate.dealing_order}
              </li>
              <li>
                <div>
                  <b>Play order: </b>
                  {gamestate.play_order}
                </div>
              </li>
              {/* <li>
              <b>Players: </b>
              {displayObject(gamestate.players)}
            </li> */}
              <li>
                <b>Score: </b>
                {displayObject(gamestate.score)}
              </li>
              <li>
                <b>State: </b>
                {gamestate.state}
              </li>
              <li>
                <b>Trump: </b>
                {gamestate.trump}
              </li>
              <li>
                <b>Wins: </b>
                {displayObject(gamestate.wins)}
              </li>
              <li>
                <b>system status: </b>
                {displayObject(gamestate.system_status)}
              </li>
            </ul>
          )}
        </div>
      )}
      <div className="flex flex-col">
        <div className="flex flex-col items-center justify-center w-1/2 align-middle border rounded-md bg-fuchsia-200 border-input bg-background ring-offset-background">
          <div>
            <label>Lobby code: </label>
            <input
              className="w-24 h-10 border rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              type="text"
              onChange={(evt) => setLobbyCode(evt.target.value)}
            ></input>
          </div>
          <div>
            <label>Name: </label>
            <input
              type="text"
              className="w-24 h-10 border rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              onChange={(evt) => setUsername(evt.target.value)}
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
        <div className="flex">
          <div className="w-3/4 bg-green-300">
            <div className="flex flex-col p-4">
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
                    className={`outline-4 w-3/4 m-2 outline bg-slate-400 flex flex-col ${
                      gamestate.curr_player_turn === username
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
          </div>
          {gamestate && gamestate.players && (
            <div className="flex flex-col w-1/4 bg-orange-200 border border-solid rounded-md bg-background">
              <div>
                <h2>Game details</h2>
                <label>Round: {gamestate.curr_round}</label>
                <label>Player Turn: {gamestate.curr_player_turn}</label>
                <ul className="flex space-x-2">
                  <label>Play order:</label>
                  {gamestate &&
                    gamestate.play_order &&
                    gamestate.play_order.map((playername) => {
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
              </div>

              <div className="flex flex-col bg-cyan-200">
                <h2>Hand details</h2>
                <div>{displayObject(gamestate.trump)}</div>
                <div>{displayObject(gamestate.system_status)}</div>
                {/* <div>{displayObject(gamestate.wins[username])}</div> */}
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

export default App;
