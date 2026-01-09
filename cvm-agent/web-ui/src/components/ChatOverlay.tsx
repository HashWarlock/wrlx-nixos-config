import { useState, useRef, useEffect } from "react";
import { useAgent } from "../hooks/useAgent";

interface ChatOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function ChatOverlay({ isOpen, onClose, isExpanded, onToggleExpand }: ChatOverlayProps) {
  const [input, setInput] = useState("");
  const { messages, sendMessage, isLoading, currentResponse } = useAgent();
  const inputRef = useRef<HTMLInputElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isOpen]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, currentResponse]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim() && !isLoading) {
      sendMessage(input.trim());
      setInput("");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className={`fixed z-50 transition-all duration-200 ${
        isExpanded
          ? "top-0 right-0 w-96 h-full"
          : "bottom-4 left-1/2 -translate-x-1/2 w-[600px]"
      }`}
      onKeyDown={handleKeyDown}
    >
      <div
        className={`bg-gray-900/95 backdrop-blur-sm border border-gray-700 shadow-2xl ${
          isExpanded ? "h-full rounded-l-lg" : "rounded-xl"
        }`}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-gray-700">
          <span className="text-sm text-gray-400">CVM Agent</span>
          <div className="flex gap-2">
            <button
              onClick={onToggleExpand}
              className="text-gray-400 hover:text-white text-sm"
            >
              {isExpanded ? "▢" : "◱"}
            </button>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-white"
            >
              x
            </button>
          </div>
        </div>

        {/* Messages (only in expanded mode) */}
        {isExpanded && (
          <div className="h-[calc(100%-120px)] overflow-y-auto p-4 space-y-4">
            {messages.map((msg, i) => (
              <div
                key={i}
                className={`${
                  msg.role === "user" ? "text-blue-400" : "text-gray-200"
                }`}
              >
                <span className="text-xs text-gray-500 uppercase">
                  {msg.role}
                </span>
                <p className="mt-1 whitespace-pre-wrap">{msg.content}</p>
              </div>
            ))}
            {currentResponse && (
              <div className="text-gray-200">
                <span className="text-xs text-gray-500 uppercase">assistant</span>
                <p className="mt-1 whitespace-pre-wrap">{currentResponse}</p>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        )}

        {/* Input */}
        <form onSubmit={handleSubmit} className="p-3 border-t border-gray-700">
          <div className="flex gap-2">
            <input
              ref={inputRef}
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Ask the agent..."
              disabled={isLoading}
              className="flex-1 bg-gray-800 border border-gray-600 rounded-lg px-4 py-2 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="submit"
              disabled={isLoading || !input.trim()}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 rounded-lg text-white font-medium"
            >
              {isLoading ? "..." : ">"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
