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

  const sendBid = (evt) => {
    console.log("sendBid: ", bid);
    let message = {
      username: username,
      message: {
        action: {
          bid: bid,
        },
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };

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
      <div className="">
        <div className="flex flex-col items-center justify-center w-1/2 align-middle border rounded-md border-input bg-background ring-offset-background">
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
            className="w-20 bg-green-500 border border-solid"
            onClick={connectToLobby}
          >
            Connect
          </button>
          <button className="p-2 m-1 outline" onClick={startGame}>
            Start game
          </button>
        </div>
        <div className="bg-green-300">
          <div className="flex flex-col p-4">
            <div>
              <h3>Played Cards</h3>
              <div className="flex flex-row justify-center">
                {gamestate?.curr_played_cards
                  ? gamestate.curr_played_cards.map((card) => {
                      return (
                        <div
                          key={card.id}
                          className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800"
                          onMouseDown={() => {}}
                        >
                          <div className="flex flex-col items-center gap-2">
                            <span className="text-4xl font-bold">
                              {card.value}
                            </span>
                            <span className="text-2xl font-medium">
                              {card.suit}
                            </span>
                          </div>
                        </div>
                      );
                    })
                  : ""}
              </div>
              <div className="m-2 outline outline-1 bg-slate-400">
                <h3>Your hand</h3>
                <div className="flex flex-row justify-center">
                  {gamestate?.players &&
                  gamestate.players[username] &&
                  gamestate?.players[username].hand
                    ? gamestate.players[username].hand.map((card) => {
                        return (
                          <div
                            key={card.id}
                            className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800"
                            onMouseDown={() => playCard(card)}
                          >
                            <div className="flex flex-col items-center gap-2">
                              <span className="text-4xl font-bold">
                                {card.value}
                              </span>
                              <span className="text-2xl font-medium">
                                {card.suit}
                              </span>
                            </div>
                          </div>
                        );
                      })
                    : ""}
                </div>
                <div className="flex justify-center m-4">
                  {gamestate && gamestate.state == "Bid" && (
                    <>
                      <input
                        className="flex w-24 h-10 border rounded-md border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                        placeholder="Enter bid"
                        onChange={(evt) => setBid(parseInt(evt.target.value))}
                        type="number"
                        onKeyDown={(evt) => {
                          if (evt.key === "Enter") {
                            sendBid();
                          }
                        }}
                      />
                      <button className="" onClick={sendBid}>
                        Submit
                      </button>
                    </>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
        <div>
          <ul>
            {messages.map((message, index) => (
              <div key={index}>
                <li>
                  {/* <span>{message.timestamp}: </span>
              <span>{message.text}</span> */}
                  <span>{JSON.stringify(message)}</span>
                </li>
              </div>
            ))}
          </ul>
        </div>
      </div>
    </>
  );
}

export default App;
