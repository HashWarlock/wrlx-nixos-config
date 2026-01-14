import { useState, useEffect } from "react";
import { sendMessage, healthCheck, HealthStatus } from "./hooks/useAgent";

function App() {
  const [message, setMessage] = useState("");
  const [response, setResponse] = useState("");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<HealthStatus | null>(null);

  useEffect(() => {
    // Check connection on mount
    healthCheck()
      .then(setStatus)
      .catch(() => setStatus({ connected: false, healthy: false, version: "" }));
  }, []);

  const handleSend = async () => {
    if (!message.trim()) return;

    setLoading(true);
    try {
      const result = await sendMessage(message);
      setResponse(result);
      setMessage("");
    } catch (err) {
      setResponse(`Error: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="container">
      <header>
        <h1>CVM Agent</h1>
        <span className={`status ${status?.connected ? "connected" : "disconnected"}`}>
          {status?.connected ? "Connected" : "Disconnected"}
        </span>
      </header>

      <div className="chat-input">
        <input
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !loading && handleSend()}
          placeholder="Type a message..."
          disabled={loading}
        />
        <button onClick={handleSend} disabled={loading || !message.trim()}>
          {loading ? "..." : "Send"}
        </button>
      </div>

      {response && (
        <div className="response">
          {response}
        </div>
      )}
    </div>
  );
}

export default App;
