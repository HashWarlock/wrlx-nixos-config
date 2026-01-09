import { useEffect, useState } from "react";
import { useGitOps, FileDiffItem } from "../hooks/useGitOps";

interface DiffPreviewProps {
  onClose: () => void;
  onApprove: () => void;
}

const riskColors = {
  low: "bg-green-500/20 text-green-400 border-green-500/50",
  medium: "bg-yellow-500/20 text-yellow-400 border-yellow-500/50",
  high: "bg-orange-500/20 text-orange-400 border-orange-500/50",
  critical: "bg-red-500/20 text-red-400 border-red-500/50",
};

const riskLabels = {
  low: "Low Risk",
  medium: "Medium Risk",
  high: "High Risk",
  critical: "Critical - Review Carefully",
};

export function DiffPreview({ onClose, onApprove }: DiffPreviewProps) {
  const { status, diffs, isLoading, error, fetchStatus, fetchDiff, stageFiles, commit } =
    useGitOps();
  const [commitMessage, setCommitMessage] = useState("");
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [expandedDiff, setExpandedDiff] = useState<string | null>(null);

  useEffect(() => {
    fetchStatus();
    fetchDiff();
  }, [fetchStatus, fetchDiff]);

  const handleStage = async () => {
    if (selectedFiles.size > 0) {
      await stageFiles(Array.from(selectedFiles));
      setSelectedFiles(new Set());
      fetchDiff(true);
    }
  };

  const handleCommit = async () => {
    if (commitMessage.trim()) {
      const hash = await commit(commitMessage);
      if (hash) {
        setCommitMessage("");
        onApprove();
      }
    }
  };

  const toggleFile = (path: string) => {
    const newSelected = new Set(selectedFiles);
    if (newSelected.has(path)) {
      newSelected.delete(path);
    } else {
      newSelected.add(path);
    }
    setSelectedFiles(newSelected);
  };

  const maxRisk = diffs.reduce((max, d) => {
    const order = ["low", "medium", "high", "critical"];
    return order.indexOf(d.riskLevel) > order.indexOf(max) ? d.riskLevel : max;
  }, "low" as FileDiffItem["riskLevel"]);

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-gray-900 border border-gray-700 rounded-xl w-[800px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-white">Review Changes</h2>
            {status && (
              <span className="text-sm text-gray-400">
                {status.branch} - {diffs.length} file(s) changed
              </span>
            )}
          </div>
          <button onClick={onClose} className="text-gray-400 hover:text-white">
            x
          </button>
        </div>

        {/* Risk Banner */}
        {maxRisk !== "low" && (
          <div className={`px-4 py-2 border-b ${riskColors[maxRisk]}`}>
            <span className="font-medium">{riskLabels[maxRisk]}</span>
            {maxRisk === "critical" && (
              <span className="ml-2 text-sm">
                These changes affect core system configuration
              </span>
            )}
          </div>
        )}

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {isLoading && <p className="text-gray-400">Loading...</p>}
          {error && <p className="text-red-400">{error}</p>}

          {/* File List */}
          {diffs.map((diff) => (
            <div key={diff.path} className="mb-4">
              <div
                className="flex items-center justify-between p-2 bg-gray-800 rounded-t cursor-pointer"
                onClick={() => setExpandedDiff(expandedDiff === diff.path ? null : diff.path)}
              >
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={selectedFiles.has(diff.path)}
                    onChange={() => toggleFile(diff.path)}
                    onClick={(e) => e.stopPropagation()}
                    className="rounded"
                  />
                  <span className="text-gray-200 font-mono text-sm">{diff.path}</span>
                </div>
                <span
                  className={`px-2 py-0.5 rounded text-xs border ${riskColors[diff.riskLevel]}`}
                >
                  {diff.riskLevel}
                </span>
              </div>

              {expandedDiff === diff.path && (
                <pre className="p-3 bg-gray-950 rounded-b text-xs font-mono overflow-x-auto max-h-64 overflow-y-auto">
                  {diff.diff.split("\n").map((line, i) => (
                    <div
                      key={i}
                      className={
                        line.startsWith("+")
                          ? "text-green-400"
                          : line.startsWith("-")
                            ? "text-red-400"
                            : line.startsWith("@")
                              ? "text-blue-400"
                              : "text-gray-400"
                      }
                    >
                      {line}
                    </div>
                  ))}
                </pre>
              )}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="border-t border-gray-700 p-4">
          <div className="flex gap-2 mb-3">
            <input
              type="text"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Commit message..."
              className="flex-1 bg-gray-800 border border-gray-600 rounded px-3 py-2 text-white text-sm"
            />
          </div>
          <div className="flex justify-between">
            <button
              onClick={handleStage}
              disabled={selectedFiles.size === 0}
              className="px-4 py-2 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-500 rounded text-sm"
            >
              Stage Selected ({selectedFiles.size})
            </button>
            <div className="flex gap-2">
              <button
                onClick={onClose}
                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm"
              >
                Cancel
              </button>
              <button
                onClick={handleCommit}
                disabled={!commitMessage.trim() || (status?.staged.length === 0)}
                className={`px-4 py-2 rounded text-sm font-medium ${
                  maxRisk === "critical"
                    ? "bg-red-600 hover:bg-red-700"
                    : "bg-blue-600 hover:bg-blue-700"
                } disabled:bg-gray-800 disabled:text-gray-500`}
              >
                {maxRisk === "critical" ? "Commit (Critical)" : "Commit"}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
