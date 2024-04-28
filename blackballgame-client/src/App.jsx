import { useState } from "react";
import reactLogo from "./assets/react.svg";
import viteLogo from "/vite.svg";
import "./App.css";

function App() {
  const [count, setCount] = useState(0);
  const [resp, setResponse] = useState("");
  const [serverState, setServerState] = useState({});
  const [name, setName] = useState("");

  const socket = new WebSocket("ws://127.0.0.1:3000/ws");
  socket.onmessage = (event) => {
    console.log(event);
    setServerState(event.data);
  };

  return (
    <>
      <div>
        <a href="https://vitejs.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <div>
        <label>Name: </label>
        <input
          type="text"
          onChange={(evt) => setName(evt.target.value)}
        ></input>
      </div>
      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <button
          onClick={() => {
            // const obj = { hello: "world" };
            // const blob = new Blob([JSON.stringify(obj, null, 2)], {
            //   type: "application/json",
            // });
            // console.log("Sending blob over websocket");
            // let res = socket.send(blob);

            let res = socket.send(`join: ${name}`);
            console.log("response from websocket?", res);
            setResponse(res);
          }}
        >
          Send message. Response: {resp}
        </button>
        <p>{JSON.stringify(serverState)}</p>
      </div>
      <div>
        <button
          onClick={() => {
            // const obj = { hello: "world" };
            // const blob = new Blob([JSON.stringify(obj, null, 2)], {
            //   type: "application/json",
            // });
            // console.log("Sending blob over websocket");
            // let res = socket.send(blob);

            let res = socket.send(`show`);
            console.log("response from websocket?", res);
            setResponse(res);
          }}
        >
          Show hand
        </button>
      </div>
    </>
  );
}

export default App;
