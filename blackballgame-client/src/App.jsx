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
      console.log(`Message from server :${message.data}`);
      // const newMessage = JSON.parse(message.data);
      // const newMessage = JSON.parse(message.data);
      setMessages((prevMessages) => [
        ...prevMessages,
        JSON.parse(message.data),
      ]);
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

  const sendMessage = () => {
    if (inputMessage.trim() === "") return;

    const message = {
      username: username,
      message: inputMessage,
      timestamp: new Date().toISOString(),
    };

    console.log("sending message: ", message);

    if (ws) {
      ws.send(JSON.stringify(message));
    }
    setInputMessage("");
  };

  const getHand = () => {
    let message = {
      username: username,
      message: {
        action: "gethand",
        origin: { player: username },
      },
      timestamp: new Date().toISOString(),
    };
    if (ws) {
      ws.send(JSON.stringify(message));
    }
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

    console.log("sending message: ", message);
    if (ws) {
      ws.send(JSON.stringify(message));
    }
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
    if (ws) {
      ws.send(JSON.stringify(message));
    }
  };

  return (
    <>
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
          <input
            type="text"
            value={inputMessage}
            onChange={(e) => setInputMessage(e.target.value)}
          />
          <button onClick={sendMessage}>Send</button>
        </div>
        <div>
          {JSON.stringify(handCards)}
          <button onClick={getHand}>Get hand</button>
          {JSON.stringify(handCards)}
          <button onClick={dealCard}>Deal</button>
          {JSON.stringify(playCard)}
          <button onClick={playCard}>Play Card</button>
        </div>
      </div>
    </>
  );
}

export default App;
