import { useState } from "react";
import reactLogo from "./assets/react.svg";
import viteLogo from "/vite.svg";
import "./App.css";

function App() {
  const [count, setCount] = useState(0);
  const [resp, setResponse] = useState("");
  const [serverState, setServerState] = useState({});
  const [name, setName] = useState("");
  const [lobbyCode, setLobbyCode] = useState("");
  const [chats, setChats] = useState([]);
  const [message, setMessage] = useState("");

  const socket = new WebSocket("ws://127.0.0.1:3000/ws");
  socket.onmessage = (event) => {
    console.log(event);
    setServerState(event.data);
    setChats((curr) => curr.push(event.data));
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
          onChange={(evt) => setName(evt.target.value)}
        ></input>
      </div>
      <div className="card">
        <button
          onClick={() => {
            let res = socket.send(
              JSON.stringify({ username: name, channel: lobbyCode })
            );
            console.log("response from websocket?", res);
            setResponse(res);
          }}
        >
          Connect
        </button>
        <p>{JSON.stringify(serverState)}</p>
      </div>
      <div>
        <button
          onClick={() => {
            let res = socket.send(`show`);
            console.log("response from websocket?", res);
            setResponse(res);
          }}
        >
          Show hand
        </button>
      </div>

      <div>
        <input
          type="text"
          onChange={(evt) => setMessage(evt.target.value)}
        ></input>
        <button onClick={() => socket.send(message)}>send message</button>
        chats: {JSON.stringify(chats)}
      </div>
    </>
  );
}

export default App;
