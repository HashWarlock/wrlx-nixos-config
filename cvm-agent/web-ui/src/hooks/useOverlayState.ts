import { useReducer, useCallback, useEffect } from "react";

/**
 * Overlay states for the agent chat interface
 * - hidden: Only floating action button visible
 * - quick-input: Minimal input bar for quick commands
 * - expanded: Full side panel with conversation history
 */
export type OverlayState = "hidden" | "quick-input" | "expanded";

/**
 * Events that trigger state transitions
 */
export type OverlayEvent =
  | "TOGGLE" // cmd+shift+space or FAB click
  | "ESCAPE" // Escape key or click outside
  | "EXPAND" // Manual expand
  | "COLLAPSE" // Manual collapse back to quick-input
  | "APPROVAL_NEEDED" // Risky operation needs approval
  | "ERROR" // Error occurred
  | "LONG_CONVERSATION" // Conversation exceeds threshold
  | "STREAMING_START" // Long streaming output started
  | "HIDE"; // Force hide

interface StateConfig {
  on: Partial<Record<OverlayEvent, OverlayState>>;
}

/**
 * State machine configuration
 * Defines valid transitions from each state
 */
const stateConfig: Record<OverlayState, StateConfig> = {
  hidden: {
    on: {
      TOGGLE: "quick-input",
    },
  },
  "quick-input": {
    on: {
      TOGGLE: "hidden",
      ESCAPE: "hidden",
      HIDE: "hidden",
      EXPAND: "expanded",
      APPROVAL_NEEDED: "expanded",
      ERROR: "expanded",
      LONG_CONVERSATION: "expanded",
      STREAMING_START: "expanded",
    },
  },
  expanded: {
    on: {
      TOGGLE: "hidden",
      ESCAPE: "quick-input",
      HIDE: "hidden",
      COLLAPSE: "quick-input",
    },
  },
};

function reducer(state: OverlayState, event: OverlayEvent): OverlayState {
  const config = stateConfig[state];
  const nextState = config.on[event];
  return nextState ?? state;
}

export interface UseOverlayStateReturn {
  /** Current overlay state */
  state: OverlayState;
  /** Send an event to the state machine */
  send: (event: OverlayEvent) => void;
  /** Whether the overlay is visible (not hidden) */
  isVisible: boolean;
  /** Whether the overlay is in expanded mode */
  isExpanded: boolean;
}

/**
 * Hook for managing the adaptive overlay state machine
 *
 * States:
 * - hidden: Only FAB visible, desktop at full view
 * - quick-input: Minimal input bar for quick commands
 * - expanded: Full side panel with history, diffs, approvals
 *
 * Auto-expands for:
 * - Approval requests (risky operations)
 * - Errors
 * - Long conversations
 * - Streaming output
 */
export function useOverlayState(): UseOverlayStateReturn {
  const [state, dispatch] = useReducer(reducer, "hidden");

  const send = useCallback((event: OverlayEvent) => {
    dispatch(event);
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // cmd+shift+space (Mac) or ctrl+shift+space (Windows/Linux)
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.code === "Space") {
        e.preventDefault();
        send("TOGGLE");
      }
      // Escape to dismiss/collapse
      if (e.key === "Escape") {
        send("ESCAPE");
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [send]);

  return {
    state,
    send,
    isVisible: state !== "hidden",
    isExpanded: state === "expanded",
  };
}

/**
 * Helper to determine if conversation length warrants expansion
 */
export function shouldExpandForConversation(messageCount: number): boolean {
  return messageCount > 3;
}

/**
 * Helper to determine if streaming output warrants expansion
 */
export function shouldExpandForStreaming(outputLines: number): boolean {
  return outputLines > 10;
}
