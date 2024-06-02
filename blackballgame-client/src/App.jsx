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

      <div>
        <label>Lobby code: </label>
        <input
          type="text"
          onChange={(evt) => setLobbyCode(evt.target.value)}
        ></input>
        <label>Name: </label>
        <input
          type="text"
          onChange={(evt) => setUsername(evt.target.value)}
        ></input>
        <button
          className="bg-green-200 border border-solid"
          onClick={connectToLobby}
        >
          Connect
        </button>
      </div>
      <div className="bg-green-300">
        <h2 className="bg-blue-300">Play area</h2>
        {gamestate && gamestate.state == "Bid" && (
          <div className="flex flex-row">
            <label>Enter your bid: </label>
            <input
              type="number"
              onChange={(evt) => setBid(parseInt(evt.target.value))}
            ></input>
            <button onClick={sendBid}>Bid</button>
          </div>
        )}
        <button className="p-2 m-1 outline" onClick={startGame}>
          Start game
        </button>
        <button className="p-2 m-1 outline" onClick={dealCard}>
          Deal
        </button>
        <div className="flex flex-col p-4">
          <div>
            <h3>Play area</h3>
            {gamestate?.curr_played_cards
              ? gamestate.curr_played_cards.map((card) => {
                  return (
                    <div
                      key={card.id}
                      className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800"
                      onMouseDown={() => {}}
                    >
                      <div className="flex flex-col items-center gap-2">
                        <span className="text-4xl font-bold">{card.value}</span>
                        <span className="text-2xl font-medium">
                          {card.suit}
                        </span>
                      </div>
                    </div>
                  );
                })
              : ""}
          </div>
          <div>
            <h3>Your hand</h3>
            <div className="flex flex-row">
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
          </div>
          <input
            className="flex w-24 h-10 px-3 py-2 text-sm border rounded-md border-input bg-background ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            placeholder="Enter bid"
            type="number"
          />
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
    </>
  );
}

export default App;
