import { useState, useEffect } from "react";
import "./App.css";
import React from "react";

// export const useWs = ({ url }) => {
//   const [isReady, setIsReady] = useState(false);
//   const [val, setVal] = useState(null);

//   const ws = useRef(null);

//   useEffect(() => {
//     const socket = new WebSocket(url);

//     socket.onopen = () => setIsReady(true);
//     socket.onclose = () => setIsReady(false);
//     socket.onmessage = (event) => setVal(event.data);

//     ws.current = socket;

//     return () => {
//       socket.close();
//     };
//   }, []);

//   // bind is needed to make sure `send` references correct `this`
//   return [isReady, val, ws.current?.send.bind(ws.current)];
// };

// export class GameEvent {
//     action: GameAction;
//     origin: Actioner;
// }

// export enum GameAction {
//     PlayCard = 'PlayCard',
//     Bid = 'Bid',
//     Deal = 'Deal',
//     StartGame = 'StartGame'
// }

// export enum Actioner {
//     System = 'System',
//     Player = 'Player'
// }

function App() {
  const [handCards, setHandCards] = useState([]);
  const [count, setCount] = useState(0);
  const [resp, setResponse] = useState("");
  const [serverState, setServerState] = useState({});
  const [username, setUsername] = useState("");
  const [lobbyCode, setLobbyCode] = useState("");
  const [chats, setChats] = useState([]);
  const [messages, setMessages] = useState([]);
  const [inputMessage, setInputMessage] = useState("");

  const [url, setUrl] = useState("ws://127.0.0.1:3000/ws");
  const [ws, setWs] = useState();
  const [gamestate, setGamestate] = useState();

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

      // const filteredGamedata = {
      //   ...JSON.parse(message.data),
      //   deck: undefined,
      // };
      setGamestate(JSON.parse(message.data));

      // setGamestate(message.data);
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
    }
  }

  function displayObject(obj) {
    return <div>{JSON.stringify(obj)}</div>;
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

  const playCard = () => {
    let message = {
      username: username,
      message: {
        action: {
          playcard: {
            id: 1,
            suit: "heart",
            value: 1,
            played_by: username,
          },
        },
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    sendMessage(message);
  };

  return (
    <>
      <div style={{ justifyContent: "left", textAlign: "left" }}>
        {gamestate && (
          <ul>
            <li>
              <b>Bids: </b>
              {displayObject(gamestate.bids)}
            </li>
            <li>
              <div>
                <b>Played cards: </b>
                {gamestate.curr_played_cards}
              </div>
            </li>
            <li>
              <div>
                <b>Turn: </b>
                {gamestate.curr_player_turn}
              </div>
            </li>
            <li>
              <div>
                <b>Round: </b>
                {gamestate.curr_round}
              </div>
            </li>
            <li>
              <div>
                <b>Winning card: </b>
                {gamestate.curr_winning_card}
              </div>
            </li>
            <li>
              <div>
                <b>Deal order: </b>
                {gamestate.dealing_order}
              </div>
            </li>
            <li>
              <div>
                <b>Play order: </b>
                {gamestate.play_order}
              </div>
            </li>
            <li>
              <b>Players: </b>
              {displayObject(gamestate.players)}
            </li>
            <li>
              <b>Score: </b>
              {displayObject(gamestate.score)}
            </li>

            <li>
              <div>
                <b>State: </b>
                {gamestate.state}
              </div>
            </li>
            <li>
              <div>
                <b>Trump: </b>
                {gamestate.trump}
              </div>
            </li>
            <li>
              <b>Wins: </b>
              {displayObject(gamestate.wins)}
            </li>
          </ul>
        )}
      </div>

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
          onClick={() => {
            var connectMessage = JSON.stringify({
              username: username,
              channel: lobbyCode,
            });
            console.log(connectMessage);
            ws.send(connectMessage);
          }}
        >
          Connect
        </button>
      </div>
      <div>
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
        <div>
          {/* {JSON.stringify(handCards)} */}
          <button onClick={startGame}>Start game</button>
          <button onClick={dealCard}>Deal</button>
          {/* {JSON.stringify(playCard)} */}
          <button onClick={playCard}>Play Card</button>
        </div>
      </div>
    </>
  );
}

export default App;
