import { invoke } from "@tauri-apps/api/core";

export interface HealthStatus {
  connected: boolean;
  healthy: boolean;
  version: string;
}

export async function sendMessage(message: string, conversationId?: string): Promise<string> {
  return invoke<string>("send_message", { message, conversationId });
}

export async function healthCheck(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}
