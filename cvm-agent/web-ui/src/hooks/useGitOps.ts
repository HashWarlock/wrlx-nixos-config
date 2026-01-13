import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { GitOpsService, RiskLevel } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createGrpcWebTransport({
  baseUrl: "http://localhost:8080",
});

const gitClient = createClient(GitOpsService, transport);

export interface FileChange {
  path: string;
  status: string;
}

export interface FileDiffItem {
  path: string;
  diff: string;
  riskLevel: "low" | "medium" | "high" | "critical";
}

export interface GitStatus {
  branch: string;
  modified: FileChange[];
  staged: FileChange[];
  untracked: FileChange[];
  hasUpstream: boolean;
  ahead: number;
  behind: number;
}

function riskLevelToString(level: RiskLevel): FileDiffItem["riskLevel"] {
  switch (level) {
    case RiskLevel.RISK_CRITICAL:
      return "critical";
    case RiskLevel.RISK_HIGH:
      return "high";
    case RiskLevel.RISK_MEDIUM:
      return "medium";
    default:
      return "low";
  }
}

export function useGitOps() {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [diffs, setDiffs] = useState<FileDiffItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await gitClient.status({});
      setStatus({
        branch: response.branch,
        modified: response.modified.map((f) => ({ path: f.path, status: f.status })),
        staged: response.staged.map((f) => ({ path: f.path, status: f.status })),
        untracked: response.untracked.map((f) => ({ path: f.path, status: f.status })),
        hasUpstream: response.hasUpstream,
        ahead: response.ahead,
        behind: response.behind,
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const fetchDiff = useCallback(async (staged: boolean = false) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await gitClient.diff({ staged });
      setDiffs(
        response.diffs.map((d) => ({
          path: d.path,
          diff: d.diff,
          riskLevel: riskLevelToString(d.riskLevel),
        }))
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const stageFiles = useCallback(
    async (paths: string[]) => {
      setIsLoading(true);
      try {
        await gitClient.add({ paths });
        await fetchStatus();
      } catch (e) {
        setError(String(e));
      } finally {
        setIsLoading(false);
      }
    },
    [fetchStatus]
  );

  const commit = useCallback(
    async (message: string) => {
      setIsLoading(true);
      try {
        const response = await gitClient.commit({ message });
        if (!response.success) {
          setError(response.error);
          return null;
        }
        await fetchStatus();
        return response.commitHash;
      } catch (e) {
        setError(String(e));
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    [fetchStatus]
  );

  return {
    status,
    diffs,
    isLoading,
    error,
    fetchStatus,
    fetchDiff,
    stageFiles,
    commit,
  };
}
