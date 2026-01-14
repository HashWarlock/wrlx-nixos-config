import { useEffect, useState } from "react";
import { useMemory, getLayerLabel, getLayerColor } from "../hooks/useMemory";
import { ForgetConfirmDialog } from "./ForgetConfirmDialog";
import { MemoryLayer } from "../gen/agent_pb";

/**
 * Panel for viewing and managing memories
 * Supports search, layer filtering, pinning, and cleanup
 */
export function MemoryPanel() {
  const {
    memories,
    forgetCandidates,
    loading,
    error,
    query,
    getForgetCandidates,
    confirmForget,
    pin,
    refreshFacts,
  } = useMemory();

  const [showForgetDialog, setShowForgetDialog] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [selectedLayer, setSelectedLayer] = useState<MemoryLayer | null>(null);

  // Initial load and refresh on filter changes
  useEffect(() => {
    const layers = selectedLayer ? [selectedLayer] : undefined;
    query(layers, searchText || undefined);
  }, [selectedLayer, searchText, query]);

  const handleCheckForget = async () => {
    await getForgetCandidates();
    if (forgetCandidates.length > 0) {
      setShowForgetDialog(true);
    }
  };

  // Re-check after getting candidates
  useEffect(() => {
    if (forgetCandidates.length > 0 && !showForgetDialog) {
      setShowForgetDialog(true);
    }
  }, [forgetCandidates, showForgetDialog]);

  const handleConfirmForget = async (
    forgetIds: string[],
    keepIds: string[]
  ) => {
    await confirmForget(forgetIds, keepIds);
    setShowForgetDialog(false);
    // Refresh the list
    const layers = selectedLayer ? [selectedLayer] : undefined;
    query(layers, searchText || undefined);
  };

  const handlePin = async (id: string, currentlyPinned: boolean) => {
    await pin(id, !currentlyPinned);
  };

  const formatTimestamp = (ms: bigint) => {
    if (!ms || ms === BigInt(0)) return "Never";
    const date = new Date(Number(ms));
    return date.toLocaleDateString() + " " + date.toLocaleTimeString();
  };

  return (
    <div className="p-4 h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-white">Memory</h3>
        <div className="flex gap-2">
          <button
            onClick={refreshFacts}
            className="text-sm text-gray-400 hover:text-white transition-colors"
            title="Refresh system facts"
          >
            Refresh
          </button>
          <button
            onClick={handleCheckForget}
            className="text-sm text-gray-400 hover:text-white transition-colors"
            title="Check for memories to clean up"
          >
            Cleanup
          </button>
        </div>
      </div>

      {/* Search and Filter */}
      <div className="flex gap-2 mb-4">
        <input
          type="text"
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          placeholder="Search memories..."
          className="flex-1 bg-gray-700 text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
        />
        <select
          value={selectedLayer ?? ""}
          onChange={(e) =>
            setSelectedLayer(
              e.target.value ? (Number(e.target.value) as MemoryLayer) : null
            )
          }
          className="bg-gray-700 text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
        >
          <option value="">All layers</option>
          <option value={MemoryLayer.WORKING}>Working</option>
          <option value={MemoryLayer.ARCHIVE}>Archive</option>
          <option value={MemoryLayer.FACTS}>Facts</option>
          <option value={MemoryLayer.PREFERENCES}>Preferences</option>
        </select>
      </div>

      {/* Error display */}
      {error && (
        <div className="text-red-400 text-sm mb-4 p-2 bg-red-900/20 rounded">
          {error}
        </div>
      )}

      {/* Memory list */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="text-gray-400 text-center py-8">Loading...</div>
        ) : memories.length === 0 ? (
          <div className="text-gray-400 text-center py-8">No memories found</div>
        ) : (
          <div className="space-y-2">
            {memories.map((memory) => (
              <div
                key={memory.id}
                className="bg-gray-700 rounded p-3 hover:bg-gray-650 transition-colors"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex-1 min-w-0">
                    {/* Layer badge and pinned indicator */}
                    <div className="flex items-center gap-2 mb-1">
                      <span
                        className={`text-xs text-white px-2 py-0.5 rounded ${getLayerColor(
                          memory.layer
                        )}`}
                      >
                        {getLayerLabel(memory.layer)}
                      </span>
                      {memory.pinned && (
                        <span className="text-xs text-yellow-400 flex items-center gap-1">
                          <svg
                            className="w-3 h-3"
                            fill="currentColor"
                            viewBox="0 0 20 20"
                          >
                            <path d="M5 5a2 2 0 012-2h6a2 2 0 012 2v2h2a1 1 0 011 1v1a1 1 0 01-.293.707L15 12.414V17a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4.586l-2.707-2.707A1 1 0 016 9V8a1 1 0 011-1h2V5z" />
                          </svg>
                          Pinned
                        </span>
                      )}
                    </div>

                    {/* Content */}
                    <p className="text-gray-200 text-sm break-words">
                      {memory.content}
                    </p>

                    {/* Metadata */}
                    <div className="text-gray-500 text-xs mt-2 flex gap-4">
                      <span>
                        Created: {formatTimestamp(memory.createdAtMs)}
                      </span>
                      <span>
                        Accessed: {formatTimestamp(memory.lastAccessedMs)}
                      </span>
                      {memory.accessCount > 0 && (
                        <span>Views: {memory.accessCount}</span>
                      )}
                    </div>
                  </div>

                  {/* Pin button */}
                  <button
                    onClick={() => handlePin(memory.id, memory.pinned)}
                    className={`p-1.5 rounded transition-colors ${
                      memory.pinned
                        ? "text-yellow-400 hover:text-yellow-300"
                        : "text-gray-500 hover:text-gray-300"
                    }`}
                    title={memory.pinned ? "Unpin" : "Pin"}
                  >
                    <svg
                      className="w-4 h-4"
                      fill="currentColor"
                      viewBox="0 0 20 20"
                    >
                      <path d="M5 5a2 2 0 012-2h6a2 2 0 012 2v2h2a1 1 0 011 1v1a1 1 0 01-.293.707L15 12.414V17a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4.586l-2.707-2.707A1 1 0 016 9V8a1 1 0 011-1h2V5z" />
                    </svg>
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Forget confirmation dialog */}
      {showForgetDialog && forgetCandidates.length > 0 && (
        <ForgetConfirmDialog
          candidates={forgetCandidates}
          onConfirm={handleConfirmForget}
          onCancel={() => setShowForgetDialog(false)}
        />
      )}
    </div>
  );
}
