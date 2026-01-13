import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient } from "@connectrpc/connect";
import {
  GUIService,
  UIElement,
  MouseButton,
  CoordinatesSchema,
} from "../gen/agent_pb";
import { create } from "@bufbuild/protobuf";
import { useState, useCallback } from "react";

const transport = createGrpcWebTransport({
  baseUrl: "/api",
});

const guiClient = createClient(GUIService, transport);

export interface GUIElement {
  id: string;
  application: string;
  role: string;
  name: string;
  description: string;
  bounds?: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  states: string[];
  actions: string[];
}

export interface VisionResult {
  found: boolean;
  center?: { x: number; y: number };
  bounds?: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  confidence: number;
  elementDescription: string;
}

function uiElementToGUIElement(element: UIElement): GUIElement {
  return {
    id: element.id,
    application: element.application,
    role: element.role,
    name: element.name,
    description: element.description,
    bounds: element.bounds
      ? {
          x: element.bounds.x,
          y: element.bounds.y,
          width: element.bounds.width,
          height: element.bounds.height,
        }
      : undefined,
    states: [...element.states],
    actions: [...element.actions],
  };
}

function uint8ArrayToBase64DataUrl(data: Uint8Array, format: string): string {
  const base64 = btoa(
    Array.from(data)
      .map((b) => String.fromCharCode(b))
      .join("")
  );
  const mimeType = format === "jpeg" ? "image/jpeg" : "image/png";
  return `data:${mimeType};base64,${base64}`;
}

export function useGUI() {
  const [screenshot, setScreenshot] = useState<string | null>(null);
  const [screenshotDimensions, setScreenshotDimensions] = useState<{
    width: number;
    height: number;
  } | null>(null);
  const [elements, setElements] = useState<GUIElement[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const captureScreenshot = useCallback(
    async (windowId?: string, includeCursor: boolean = true, quality: number = 0) => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await guiClient.screenshot({
          windowId,
          includeCursor,
          quality,
        });
        const dataUrl = uint8ArrayToBase64DataUrl(response.imageData, response.format);
        setScreenshot(dataUrl);
        setScreenshotDimensions({ width: response.width, height: response.height });
        return dataUrl;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const queryElements = useCallback(
    async (application?: string, role?: string, name?: string, maxDepth: number = 0) => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await guiClient.getElements({
          application,
          role,
          name,
          maxDepth,
        });
        const mapped = response.elements.map(uiElementToGUIElement);
        setElements(mapped);
        return mapped;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return [];
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const click = useCallback(
    async (x: number, y: number, button: "left" | "right" | "middle" = "left", clicks: number = 1) => {
      setIsLoading(true);
      setError(null);
      try {
        const mouseButton =
          button === "right"
            ? MouseButton.MOUSE_RIGHT
            : button === "middle"
              ? MouseButton.MOUSE_MIDDLE
              : MouseButton.MOUSE_LEFT;

        const response = await guiClient.click({
          target: {
            case: "position",
            value: create(CoordinatesSchema, { x, y }),
          },
          button: mouseButton,
          clicks,
        });

        if (!response.success) {
          setError(response.error);
          return false;
        }
        return true;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return false;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const clickElement = useCallback(
    async (elementId: string, button: "left" | "right" | "middle" = "left", clicks: number = 1) => {
      setIsLoading(true);
      setError(null);
      try {
        const mouseButton =
          button === "right"
            ? MouseButton.MOUSE_RIGHT
            : button === "middle"
              ? MouseButton.MOUSE_MIDDLE
              : MouseButton.MOUSE_LEFT;

        const response = await guiClient.click({
          target: {
            case: "elementId",
            value: elementId,
          },
          button: mouseButton,
          clicks,
        });

        if (!response.success) {
          setError(response.error);
          return false;
        }
        return true;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return false;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const typeText = useCallback(
    async (text: string, elementId?: string, delayMs: number = 0, clearFirst: boolean = false) => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await guiClient.type({
          text,
          elementId,
          delayMs,
          clearFirst,
        });

        if (!response.success) {
          setError(response.error);
          return false;
        }
        return true;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return false;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const keyPress = useCallback(async (keys: string[], elementId?: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await guiClient.keyPress({
        keys,
        elementId,
      });

      if (!response.success) {
        setError(response.error);
        return false;
      }
      return true;
    } catch (e) {
      const errorMsg = String(e);
      setError(errorMsg);
      return false;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const moveMouse = useCallback(async (x: number, y: number, smooth: boolean = false) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await guiClient.moveMouse({
        x,
        y,
        smooth,
      });

      if (!response.success) {
        setError(response.error);
        return false;
      }
      return true;
    } catch (e) {
      const errorMsg = String(e);
      setError(errorMsg);
      return false;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const findByVision = useCallback(
    async (description: string, screenshotData?: Uint8Array): Promise<VisionResult | null> => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await guiClient.findByVision({
          description,
          screenshot: screenshotData,
        });

        const result: VisionResult = {
          found: response.found,
          center: response.center
            ? { x: response.center.x, y: response.center.y }
            : undefined,
          bounds: response.bounds
            ? {
                x: response.bounds.x,
                y: response.bounds.y,
                width: response.bounds.width,
                height: response.bounds.height,
              }
            : undefined,
          confidence: response.confidence,
          elementDescription: response.elementDescription,
        };

        return result;
      } catch (e) {
        const errorMsg = String(e);
        setError(errorMsg);
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const refresh = useCallback(async () => {
    await captureScreenshot();
    await queryElements();
  }, [captureScreenshot, queryElements]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    // State
    screenshot,
    screenshotDimensions,
    elements,
    isLoading,
    error,

    // Methods
    captureScreenshot,
    queryElements,
    click,
    clickElement,
    typeText,
    keyPress,
    moveMouse,
    findByVision,
    refresh,
    clearError,
  };
}
