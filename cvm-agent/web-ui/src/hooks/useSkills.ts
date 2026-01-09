import { createConnectTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import { SkillsService, SkillType } from "../gen/agent_pb";
import { useState, useCallback } from "react";

const transport = createConnectTransport({
  baseUrl: "/api",
});

const skillsClient = createClient(SkillsService, transport);

export interface SkillSummary {
  name: string;
  description: string;
  type: "instruction" | "workflow";
  triggers: string[];
}

export function useSkills() {
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchSkills = useCallback(async () => {
    setIsLoading(true);
    try {
      const response = await skillsClient.list({});
      setSkills(
        response.skills.map((s) => ({
          name: s.name,
          description: s.description,
          type: s.type === SkillType.SKILL_INSTRUCTION ? "instruction" : "workflow",
          triggers: [...s.triggers],
        }))
      );
    } catch (error) {
      console.error("Failed to fetch skills:", error);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const matchSkills = useCallback(async (input: string) => {
    try {
      const response = await skillsClient.match({ input });
      return response.matches.map((m) => ({
        name: m.name,
        relevance: m.relevance,
        trigger: m.matchedTrigger,
      }));
    } catch {
      return [];
    }
  }, []);

  const executeSkill = useCallback(async function* (
    name: string,
    params: Record<string, string>
  ) {
    for await (const progress of skillsClient.execute({ name, parameters: params })) {
      yield {
        step: progress.stepNumber,
        total: progress.totalSteps,
        description: progress.stepDescription,
        output: progress.result.case === "output" ? progress.result.value : undefined,
        error: progress.result.case === "error" ? progress.result.value : undefined,
        completed: progress.completed,
      };
    }
  }, []);

  return {
    skills,
    isLoading,
    fetchSkills,
    matchSkills,
    executeSkill,
  };
}
