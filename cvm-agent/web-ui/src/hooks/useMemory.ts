import { useState, useCallback } from "react";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { create } from "@bufbuild/protobuf";
import {
  MemoryService,
  MemoryQuerySchema,
  StoreMemoryRequestSchema,
  GetForgetCandidatesRequestSchema,
  ForgetDecisionSchema,
  PinRequestSchema,
  RefreshFactsRequestSchema,
  MemoryLayer,
} from "../gen/agent_pb";
import type { MemoryEntry, ForgetCandidate } from "../gen/agent_pb";

const transport = createGrpcWebTransport({
  baseUrl: "http://localhost:8080",
});

const memoryClient = createClient(MemoryService, transport);

export interface UseMemoryReturn {
  /** Current list of memories */
  memories: MemoryEntry[];
  /** Current forget candidates */
  forgetCandidates: ForgetCandidate[];
  /** Loading state */
  loading: boolean;
  /** Error message if any */
  error: string | null;
  /** Query memories with optional filters */
  query: (layers?: MemoryLayer[], searchText?: string) => Promise<void>;
  /** Store a new memory */
  store: (
    layer: MemoryLayer,
    content: string,
    metadata?: Record<string, string>
  ) => Promise<string | null>;
  /** Get forget candidates */
  getForgetCandidates: () => Promise<void>;
  /** Confirm forget/keep decisions */
  confirmForget: (forgetIds: string[], keepIds: string[]) => Promise<void>;
  /** Pin or unpin a memory */
  pin: (id: string, pinned: boolean) => Promise<void>;
  /** Refresh system facts */
  refreshFacts: () => Promise<void>;
}

/**
 * Hook for interacting with the MemoryService
 */
export function useMemory(): UseMemoryReturn {
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [forgetCandidates, setForgetCandidates] = useState<ForgetCandidate[]>(
    []
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const query = useCallback(
    async (layers?: MemoryLayer[], searchText?: string) => {
      setLoading(true);
      setError(null);
      try {
        const request = create(MemoryQuerySchema, {
          layers: layers || [],
          searchText: searchText || "",
          limit: 50,
        });
        const response = await memoryClient.query(request);
        setMemories(response.entries);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Query failed");
      } finally {
        setLoading(false);
      }
    },
    []
  );

  const store = useCallback(
    async (
      layer: MemoryLayer,
      content: string,
      metadata?: Record<string, string>
    ) => {
      setError(null);
      try {
        const request = create(StoreMemoryRequestSchema, {
          layer,
          content,
          metadata: metadata || {},
        });
        const response = await memoryClient.store(request);
        return response.id;
      } catch (e) {
        setError(e instanceof Error ? e.message : "Store failed");
        return null;
      }
    },
    []
  );

  const getForgetCandidates = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const request = create(GetForgetCandidatesRequestSchema, {});
      const response = await memoryClient.getForgetCandidates(request);
      setForgetCandidates(response.candidates);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to get candidates");
    } finally {
      setLoading(false);
    }
  }, []);

  const confirmForget = useCallback(
    async (forgetIds: string[], keepIds: string[]) => {
      setError(null);
      try {
        const request = create(ForgetDecisionSchema, {
          forgetIds,
          keepIds,
        });
        await memoryClient.confirmForget(request);
        // Refresh candidates after confirmation
        await getForgetCandidates();
      } catch (e) {
        setError(e instanceof Error ? e.message : "Forget failed");
      }
    },
    [getForgetCandidates]
  );

  const pin = useCallback(
    async (id: string, pinned: boolean) => {
      setError(null);
      try {
        const request = create(PinRequestSchema, { id, pinned });
        await memoryClient.pin(request);
        // Refresh memories after pin change
        await query();
      } catch (e) {
        setError(e instanceof Error ? e.message : "Pin failed");
      }
    },
    [query]
  );

  const refreshFacts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const request = create(RefreshFactsRequestSchema, {});
      const response = await memoryClient.refreshFacts(request);
      // Update memories with refreshed facts
      setMemories((prev) => {
        const nonFacts = prev.filter((m) => m.layer !== MemoryLayer.FACTS);
        return [...nonFacts, ...response.entries];
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Refresh failed");
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    memories,
    forgetCandidates,
    loading,
    error,
    query,
    store,
    getForgetCandidates,
    confirmForget,
    pin,
    refreshFacts,
  };
}

/**
 * Get human-readable label for memory layer
 */
export function getLayerLabel(layer: MemoryLayer): string {
  switch (layer) {
    case MemoryLayer.WORKING:
      return "Working";
    case MemoryLayer.ARCHIVE:
      return "Archive";
    case MemoryLayer.FACTS:
      return "Facts";
    case MemoryLayer.PREFERENCES:
      return "Preferences";
    default:
      return "Unknown";
  }
}

/**
 * Get color class for memory layer
 */
export function getLayerColor(layer: MemoryLayer): string {
  switch (layer) {
    case MemoryLayer.WORKING:
      return "bg-blue-600";
    case MemoryLayer.ARCHIVE:
      return "bg-gray-600";
    case MemoryLayer.FACTS:
      return "bg-green-600";
    case MemoryLayer.PREFERENCES:
      return "bg-purple-600";
    default:
      return "bg-gray-600";
  }
}
