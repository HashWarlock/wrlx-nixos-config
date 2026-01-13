import { useState } from "react";

function App() {
  const [message, setMessage] = useState("");
  const [response, setResponse] = useState("");

  const sendMessage = async () => {
    // TODO: Call Tauri command to send gRPC message
    setResponse(`Echo: ${message}`);
  };

  return (
    <div className="container">
      <h1>CVM Agent</h1>
      <div className="chat-input">
        <input
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && sendMessage()}
          placeholder="Type a message..."
        />
        <button onClick={sendMessage}>Send</button>
      </div>
      {response && <div className="response">{response}</div>}
    </div>
  );
}

export default App;
