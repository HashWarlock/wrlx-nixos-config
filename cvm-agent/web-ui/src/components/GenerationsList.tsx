import { useEffect } from "react";
import { useNixOps } from "../hooks/useNixOps";

interface GenerationsListProps {
  onClose: () => void;
}

export function GenerationsList({ onClose }: GenerationsListProps) {
  const {
    generations,
    currentGen,
    output,
    isRunning,
    error,
    fetchGenerations,
    fetchCurrentGeneration,
    rebuild,
    rollback,
  } = useNixOps();

  useEffect(() => {
    fetchGenerations();
    fetchCurrentGeneration();
  }, [fetchGenerations, fetchCurrentGeneration]);

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-gray-900 border border-gray-700 rounded-xl w-[700px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
          <h2 className="text-lg font-semibold text-white">NixOS Generations</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white">
            x
          </button>
        </div>

        {/* Current Generation */}
        {currentGen && (
          <div className="px-4 py-3 bg-blue-500/10 border-b border-gray-700">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-blue-400 font-medium">
                  Current: Generation {currentGen.number}
                </span>
                <span className="text-gray-400 text-sm ml-3">{currentGen.nixosVersion}</span>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => rebuild("test")}
                  disabled={isRunning}
                  className="px-3 py-1 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 rounded text-sm"
                >
                  Test
                </button>
                <button
                  onClick={() => rebuild("switch")}
                  disabled={isRunning}
                  className="px-3 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-800 rounded text-sm"
                >
                  Rebuild
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Generations List */}
        <div className="flex-1 overflow-y-auto p-4">
          {error && (
            <div className="mb-4 p-3 bg-red-500/20 border border-red-500/50 rounded text-red-400 text-sm">
              {error}
            </div>
          )}

          <div className="space-y-2">
            {generations.map((gen) => (
              <div
                key={gen.number}
                className={`flex items-center justify-between p-3 rounded ${
                  gen.current ? "bg-blue-500/20 border border-blue-500/30" : "bg-gray-800"
                }`}
              >
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-white font-medium">Generation {gen.number}</span>
                    {gen.current && (
                      <span className="px-2 py-0.5 bg-blue-500/30 rounded text-xs text-blue-400">
                        current
                      </span>
                    )}
                  </div>
                  <div className="text-gray-400 text-sm mt-1">
                    {gen.date} - {gen.nixosVersion || "unknown version"}
                  </div>
                </div>
                {!gen.current && (
                  <button
                    onClick={() => rollback(gen.number)}
                    disabled={isRunning}
                    className="px-3 py-1 bg-orange-600 hover:bg-orange-700 disabled:bg-gray-800 rounded text-sm"
                  >
                    Rollback
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Output Console */}
        {output.length > 0 && (
          <div className="border-t border-gray-700">
            <div className="px-4 py-2 bg-gray-800 text-sm text-gray-400 flex items-center justify-between">
              <span>Output</span>
              {isRunning && <span className="animate-pulse text-blue-400">Running...</span>}
            </div>
            <pre className="p-4 bg-gray-950 text-xs font-mono max-h-48 overflow-y-auto text-gray-300">
              {output.join("\n")}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
