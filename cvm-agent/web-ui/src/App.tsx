import { useState, useEffect } from "react";
import { ChatOverlay } from "./components/ChatOverlay";

function App() {
  const [isOpen, setIsOpen] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // cmd+shift+space (Mac) or ctrl+shift+space (Windows/Linux)
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.code === "Space") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <div className="min-h-screen bg-gray-950">
      {/* Placeholder for noVNC - in production this would be the noVNC canvas */}
      <div className="flex items-center justify-center h-screen text-gray-600">
        <div className="text-center">
          <p className="text-sm">Press <kbd className="px-2 py-1 bg-gray-800 rounded">Cmd</kbd> + <kbd className="px-2 py-1 bg-gray-800 rounded">Shift</kbd> + <kbd className="px-2 py-1 bg-gray-800 rounded">Space</kbd> to open agent</p>
          <p className="text-xs mt-2 text-gray-700">Or click the button below</p>
          <button
            onClick={() => setIsOpen(true)}
            className="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-white"
          >
            Open Agent
          </button>
        </div>
      </div>

      {/* Floating action button */}
      {!isOpen && (
        <button
          onClick={() => setIsOpen(true)}
          className="fixed bottom-4 right-4 w-12 h-12 bg-blue-600 hover:bg-blue-700 rounded-full shadow-lg flex items-center justify-center text-white text-xl"
          title="Open CVM Agent"
        >
          ?
        </button>
      )}

      <ChatOverlay
        isOpen={isOpen}
        onClose={() => {
          setIsOpen(false);
          setIsExpanded(false);
        }}
        isExpanded={isExpanded}
        onToggleExpand={() => setIsExpanded(!isExpanded)}
      />
    </div>
  );
}

export default App;
