import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { NixOpsService } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createGrpcWebTransport({
  baseUrl: "/api",
});

const nixClient = createClient(NixOpsService, transport);

export interface Generation {
  number: number;
  date: string;
  nixosVersion: string;
  kernelVersion: string;
  configurationRevision: string;
  current: boolean;
}

export function useNixOps() {
  const [generations, setGenerations] = useState<Generation[]>([]);
  const [currentGen, setCurrentGen] = useState<Generation | null>(null);
  const [output, setOutput] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchGenerations = useCallback(async (limit: number = 10) => {
    try {
      const response = await nixClient.listGenerations({ limit });
      setGenerations(
        response.generations.map((g) => ({
          number: g.number,
          date: g.date,
          nixosVersion: g.nixosVersion,
          kernelVersion: g.kernelVersion,
          configurationRevision: g.configurationRevision,
          current: g.current,
        }))
      );
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const fetchCurrentGeneration = useCallback(async () => {
    try {
      const response = await nixClient.currentGeneration({});
      setCurrentGen({
        number: response.number,
        date: response.date,
        nixosVersion: response.nixosVersion,
        kernelVersion: response.kernelVersion,
        configurationRevision: response.configurationRevision,
        current: true,
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const rebuild = useCallback(
    async (action: "switch" | "boot" | "test" | "build" = "switch") => {
      setIsRunning(true);
      setOutput([]);
      setError(null);

      try {
        for await (const chunk of nixClient.rebuild({ action })) {
          if (chunk.output.case === "stdout" && chunk.output.value) {
            const line = chunk.output.value;
            setOutput((prev) => [...prev, line]);
          } else if (chunk.output.case === "stderr" && chunk.output.value) {
            const line = `[stderr] ${chunk.output.value}`;
            setOutput((prev) => [...prev, line]);
          }
          if (chunk.phase) {
            const phaseLine = `--- Phase: ${chunk.phase} ---`;
            setOutput((prev) => [...prev, phaseLine]);
          }
          if (chunk.exitCode !== undefined && chunk.exitCode !== null) {
            if (chunk.exitCode !== 0) {
              setError(`Rebuild failed with exit code ${chunk.exitCode}`);
            }
          }
        }
        await fetchCurrentGeneration();
        await fetchGenerations();
      } catch (e) {
        setError(String(e));
      } finally {
        setIsRunning(false);
      }
    },
    [fetchCurrentGeneration, fetchGenerations]
  );

  const rollback = useCallback(
    async (generation?: number) => {
      setIsRunning(true);
      setOutput([]);
      setError(null);

      try {
        for await (const chunk of nixClient.rollback({ generation })) {
          if (chunk.output.case === "stdout" && chunk.output.value) {
            const line = chunk.output.value;
            setOutput((prev) => [...prev, line]);
          } else if (chunk.output.case === "stderr" && chunk.output.value) {
            const line = `[stderr] ${chunk.output.value}`;
            setOutput((prev) => [...prev, line]);
          }
          if (chunk.exitCode !== undefined && chunk.exitCode !== null) {
            if (chunk.exitCode !== 0) {
              setError(`Rollback failed with exit code ${chunk.exitCode}`);
            }
          }
        }
        await fetchCurrentGeneration();
        await fetchGenerations();
      } catch (e) {
        setError(String(e));
      } finally {
        setIsRunning(false);
      }
    },
    [fetchCurrentGeneration, fetchGenerations]
  );

  return {
    generations,
    currentGen,
    output,
    isRunning,
    error,
    fetchGenerations,
    fetchCurrentGeneration,
    rebuild,
    rollback,
  };
}
