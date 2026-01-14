import { useState } from "react";
import type { ForgetCandidate } from "../gen/agent_pb";
import { getLayerLabel, getLayerColor } from "../hooks/useMemory";

interface ForgetConfirmDialogProps {
  candidates: ForgetCandidate[];
  onConfirm: (forgetIds: string[], keepIds: string[]) => void;
  onCancel: () => void;
}

/**
 * Dialog for confirming memory cleanup decisions
 * Shows candidates for deletion with toggle to keep/forget each
 */
export function ForgetConfirmDialog({
  candidates,
  onConfirm,
  onCancel,
}: ForgetConfirmDialogProps) {
  // Initialize decisions based on auto_forget flag
  const [decisions, setDecisions] = useState<Record<string, "forget" | "keep">>(
    () => {
      const initial: Record<string, "forget" | "keep"> = {};
      candidates.forEach((c) => {
        if (c.entry) {
          initial[c.entry.id] = c.autoForget ? "forget" : "keep";
        }
      });
      return initial;
    }
  );

  const handleConfirm = () => {
    const forgetIds: string[] = [];
    const keepIds: string[] = [];
    Object.entries(decisions).forEach(([id, decision]) => {
      if (decision === "forget") {
        forgetIds.push(id);
      } else {
        keepIds.push(id);
      }
    });
    onConfirm(forgetIds, keepIds);
  };

  const toggleDecision = (id: string) => {
    setDecisions((prev) => ({
      ...prev,
      [id]: prev[id] === "forget" ? "keep" : "forget",
    }));
  };

  const needsConfirmation = candidates.filter((c) => !c.autoForget);
  const autoForgetCount = candidates.filter((c) => c.autoForget).length;
  const forgetCount = Object.values(decisions).filter(
    (d) => d === "forget"
  ).length;
  const keepCount = Object.values(decisions).filter((d) => d === "keep").length;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-gray-800 rounded-lg p-6 max-w-lg w-full max-h-[80vh] overflow-y-auto mx-4">
        <h2 className="text-xl font-semibold text-white mb-4">
          Memory Cleanup
        </h2>

        {autoForgetCount > 0 && (
          <p className="text-gray-400 mb-4">
            {autoForgetCount} expired item{autoForgetCount > 1 ? "s" : ""} will
            be automatically removed.
          </p>
        )}

        {needsConfirmation.length > 0 ? (
          <>
            <p className="text-gray-300 mb-4">
              The following memories haven't been accessed recently. Choose
              which to keep or forget:
            </p>

            <div className="space-y-3 mb-6">
              {needsConfirmation.map((candidate) => {
                const entry = candidate.entry;
                if (!entry) return null;

                const isForget = decisions[entry.id] === "forget";

                return (
                  <div
                    key={entry.id}
                    className={`p-3 rounded border transition-colors ${
                      isForget
                        ? "border-red-500/50 bg-red-900/20"
                        : "border-green-500/50 bg-green-900/20"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <span
                            className={`text-xs text-white px-2 py-0.5 rounded ${getLayerColor(
                              entry.layer
                            )}`}
                          >
                            {getLayerLabel(entry.layer)}
                          </span>
                          {entry.pinned && (
                            <span className="text-xs text-yellow-400">
                              Pinned
                            </span>
                          )}
                        </div>
                        <p className="text-gray-200 text-sm line-clamp-2">
                          {entry.content}
                        </p>
                        <p className="text-gray-500 text-xs mt-1">
                          {candidate.reason}
                        </p>
                      </div>
                      <button
                        onClick={() => toggleDecision(entry.id)}
                        className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
                          isForget
                            ? "bg-red-600 hover:bg-red-500 text-white"
                            : "bg-green-600 hover:bg-green-500 text-white"
                        }`}
                      >
                        {isForget ? "Forget" : "Keep"}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        ) : (
          <p className="text-gray-300 mb-6">
            All candidates can be automatically cleaned up.
          </p>
        )}

        {/* Summary */}
        <div className="text-sm text-gray-400 mb-4">
          Summary: {forgetCount} to forget, {keepCount} to keep
        </div>

        {/* Actions */}
        <div className="flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-500 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            className="px-4 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
          >
            Confirm
          </button>
        </div>
      </div>
    </div>
  );
}
