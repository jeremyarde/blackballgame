import { useState, useEffect } from "react";
import "./App.css";

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

function App() {
  const [count, setCount] = useState(0);
  const [resp, setResponse] = useState("");
  const [serverState, setServerState] = useState({});
  const [name, setName] = useState("");
  const [lobbyCode, setLobbyCode] = useState("");
  const [chats, setChats] = useState([]);
  const [messages, setMessages] = useState([]);
  const [inputMessage, setInputMessage] = useState("");

  const [url, setUrl] = useState("ws://127.0.0.1:3000/ws");
  const [ws, setWs] = useState(null);

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
      console.log("Message from server ", event.data);
      const newMessage = JSON.parse(message.data);
      setMessages((prevMessages) => [...prevMessages, newMessage]);
    };

    ws.onclose = () => {
      console.log("WebSocket disconnected");
      // setWs(null);
    };

    return () => {
      ws.close();
    };
  }, [ws]);

  const sendMessage = () => {
    if (inputMessage.trim() === "") return;

    const message = {
      text: inputMessage,
      timestamp: new Date().toISOString(),
    };

    ws.send(JSON.stringify(message));
    setInputMessage("");
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
        <button
          onClick={() => {
            var connectMessage = JSON.stringify({
              username: name,
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
          {messages.map((message, index) => (
            <div key={index}>
              <span>{message.timestamp}: </span>
              <span>{message.text}</span>
            </div>
          ))}
        </div>
        <div>
          <input
            type="text"
            value={inputMessage}
            onChange={(e) => setInputMessage(e.target.value)}
          />
          <button onClick={sendMessage}>Send</button>
        </div>
      </div>
    </>
  );
}

export default App;
