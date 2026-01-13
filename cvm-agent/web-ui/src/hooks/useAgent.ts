import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { ChatService, ChatRequestSchema } from "../gen/agent_pb";
import { create } from "@bufbuild/protobuf";
import { useState, useCallback } from "react";

const transport = createGrpcWebTransport({
  baseUrl: "/api",
});

const chatClient = createClient(ChatService, transport);

export function useAgent() {
  const [messages, setMessages] = useState<Array<{ role: string; content: string }>>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [currentResponse, setCurrentResponse] = useState("");

  const sendMessage = useCallback(async (message: string) => {
    setIsLoading(true);
    setMessages((prev) => [...prev, { role: "user", content: message }]);
    setCurrentResponse("");

    try {
      let fullResponse = "";
      const request = create(ChatRequestSchema, { message });

      for await (const response of chatClient.sendMessage(request)) {
        if (response.response.case === "text") {
          fullResponse += response.response.value;
          setCurrentResponse(fullResponse);
        } else if (response.response.case === "error") {
          fullResponse = `Error: ${response.response.value}`;
          setCurrentResponse(fullResponse);
          break;
        }
        if (response.done) {
          break;
        }
      }
      setMessages((prev) => [...prev, { role: "assistant", content: fullResponse }]);
      setCurrentResponse("");
    } catch (error) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${error}` },
      ]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { messages, sendMessage, isLoading, currentResponse };
}
