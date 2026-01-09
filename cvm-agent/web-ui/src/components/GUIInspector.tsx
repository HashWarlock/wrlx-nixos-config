import { useEffect, useState, useRef, useCallback } from "react";
import { useGUI, GUIElement } from "../hooks/useGUI";

interface GUIInspectorProps {
  onClose: () => void;
}

export function GUIInspector({ onClose }: GUIInspectorProps) {
  const {
    screenshot,
    screenshotDimensions,
    elements,
    isLoading,
    error,
    captureScreenshot,
    queryElements,
    click,
    findByVision,
    refresh,
    clearError,
  } = useGUI();

  const [filterApp, setFilterApp] = useState("");
  const [filterRole, setFilterRole] = useState("");
  const [visionQuery, setVisionQuery] = useState("");
  const [visionResult, setVisionResult] = useState<{
    found: boolean;
    center?: { x: number; y: number };
    bounds?: { x: number; y: number; width: number; height: number };
    confidence: number;
    description: string;
  } | null>(null);
  const [selectedElement, setSelectedElement] = useState<GUIElement | null>(null);
  const [clickPosition, setClickPosition] = useState<{ x: number; y: number } | null>(null);
  const [imageScale, setImageScale] = useState(1);

  const imageContainerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);

  // Load screenshot and elements on mount
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Calculate image scale when screenshot loads
  useEffect(() => {
    if (imageRef.current && screenshotDimensions && imageContainerRef.current) {
      const containerWidth = imageContainerRef.current.clientWidth;
      const containerHeight = imageContainerRef.current.clientHeight;
      const scaleX = containerWidth / screenshotDimensions.width;
      const scaleY = containerHeight / screenshotDimensions.height;
      setImageScale(Math.min(scaleX, scaleY, 1));
    }
  }, [screenshot, screenshotDimensions]);

  // Get unique applications and roles for filter dropdowns
  const applications = [...new Set(elements.map((e) => e.application).filter(Boolean))];
  const roles = [...new Set(elements.map((e) => e.role).filter(Boolean))];

  // Filter elements
  const filteredElements = elements.filter((el) => {
    if (filterApp && el.application !== filterApp) return false;
    if (filterRole && el.role !== filterRole) return false;
    return true;
  });

  const handleImageClick = useCallback(
    async (e: React.MouseEvent<HTMLImageElement>) => {
      if (!imageRef.current || !screenshotDimensions) return;

      const rect = imageRef.current.getBoundingClientRect();
      const clickX = Math.round(((e.clientX - rect.left) / rect.width) * screenshotDimensions.width);
      const clickY = Math.round(((e.clientY - rect.top) / rect.height) * screenshotDimensions.height);

      setClickPosition({ x: clickX, y: clickY });

      // Perform the click
      const success = await click(clickX, clickY);

      if (success) {
        // Refresh screenshot after click
        setTimeout(() => {
          captureScreenshot();
        }, 500);
      }
    },
    [click, captureScreenshot, screenshotDimensions]
  );

  const handleVisionSearch = async () => {
    if (!visionQuery.trim()) return;

    const result = await findByVision(visionQuery.trim());
    if (result) {
      setVisionResult({
        found: result.found,
        center: result.center,
        bounds: result.bounds,
        confidence: result.confidence,
        description: result.elementDescription,
      });
    }
  };

  const handleRefresh = () => {
    setVisionResult(null);
    setSelectedElement(null);
    setClickPosition(null);
    refresh();
  };

  const handleRequery = () => {
    queryElements(filterApp || undefined, filterRole || undefined);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
  };

  // Convert real coordinates to display coordinates
  const toDisplayCoords = (x: number, y: number) => ({
    x: x * imageScale,
    y: y * imageScale,
  });

  return (
    <div
      className="fixed inset-0 bg-black/90 z-50 flex flex-col"
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700 bg-gray-900">
        <div className="flex items-center gap-4">
          <h2 className="text-lg font-semibold text-white">GUI Inspector</h2>
          {isLoading && (
            <span className="text-sm text-blue-400 animate-pulse">Loading...</span>
          )}
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleRefresh}
            disabled={isLoading}
            className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-500 rounded text-sm text-white"
          >
            Refresh
          </button>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white text-xl px-2"
          >
            x
          </button>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="px-4 py-2 bg-red-900/50 border-b border-red-700 flex items-center justify-between">
          <span className="text-red-400 text-sm">{error}</span>
          <button
            onClick={clearError}
            className="text-red-400 hover:text-red-300 text-sm"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Screenshot Panel */}
        <div className="flex-1 flex flex-col overflow-hidden bg-gray-950">
          {/* Screenshot Container */}
          <div
            ref={imageContainerRef}
            className="flex-1 relative overflow-auto flex items-center justify-center p-4"
          >
            {screenshot ? (
              <div className="relative">
                <img
                  ref={imageRef}
                  src={screenshot}
                  alt="Screen capture"
                  className="max-w-full max-h-full cursor-crosshair"
                  onClick={handleImageClick}
                  style={{ imageRendering: "auto" }}
                />

                {/* Overlay for selected element bounds */}
                {selectedElement?.bounds && (
                  <div
                    className="absolute border-2 border-blue-500 bg-blue-500/20 pointer-events-none"
                    style={{
                      left: toDisplayCoords(selectedElement.bounds.x, 0).x,
                      top: toDisplayCoords(0, selectedElement.bounds.y).y,
                      width: selectedElement.bounds.width * imageScale,
                      height: selectedElement.bounds.height * imageScale,
                    }}
                  />
                )}

                {/* Overlay for vision result bounds */}
                {visionResult?.found && visionResult.bounds && (
                  <div
                    className="absolute border-2 border-green-500 bg-green-500/20 pointer-events-none"
                    style={{
                      left: toDisplayCoords(visionResult.bounds.x, 0).x,
                      top: toDisplayCoords(0, visionResult.bounds.y).y,
                      width: visionResult.bounds.width * imageScale,
                      height: visionResult.bounds.height * imageScale,
                    }}
                  />
                )}

                {/* Click position indicator */}
                {clickPosition && (
                  <div
                    className="absolute w-4 h-4 -ml-2 -mt-2 border-2 border-red-500 rounded-full bg-red-500/30 pointer-events-none"
                    style={{
                      left: toDisplayCoords(clickPosition.x, 0).x,
                      top: toDisplayCoords(0, clickPosition.y).y,
                    }}
                  />
                )}
              </div>
            ) : (
              <div className="text-gray-500 text-center">
                <p>No screenshot captured</p>
                <button
                  onClick={() => captureScreenshot()}
                  className="mt-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded text-white text-sm"
                >
                  Capture Screenshot
                </button>
              </div>
            )}
          </div>

          {/* Vision Search Bar */}
          <div className="border-t border-gray-700 p-3 bg-gray-900">
            <div className="flex gap-2">
              <input
                type="text"
                value={visionQuery}
                onChange={(e) => setVisionQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleVisionSearch()}
                placeholder="Find element by description (e.g., 'blue submit button')"
                className="flex-1 bg-gray-800 border border-gray-600 rounded px-3 py-2 text-white text-sm placeholder-gray-500 focus:outline-none focus:border-blue-500"
              />
              <button
                onClick={handleVisionSearch}
                disabled={isLoading || !visionQuery.trim()}
                className="px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-gray-700 disabled:text-gray-500 rounded text-sm text-white"
              >
                Find
              </button>
            </div>
            {visionResult && (
              <div
                className={`mt-2 p-2 rounded text-sm ${
                  visionResult.found
                    ? "bg-green-900/30 text-green-400 border border-green-700"
                    : "bg-yellow-900/30 text-yellow-400 border border-yellow-700"
                }`}
              >
                {visionResult.found ? (
                  <>
                    <span className="font-medium">Found:</span> {visionResult.description}
                    {visionResult.center && (
                      <span className="ml-2 text-gray-400">
                        at ({visionResult.center.x}, {visionResult.center.y})
                      </span>
                    )}
                    <span className="ml-2 text-gray-400">
                      Confidence: {(visionResult.confidence * 100).toFixed(1)}%
                    </span>
                  </>
                ) : (
                  <span>Element not found</span>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Element List Panel */}
        <div className="w-80 border-l border-gray-700 flex flex-col bg-gray-900">
          {/* Filters */}
          <div className="p-3 border-b border-gray-700 space-y-2">
            <div className="flex gap-2">
              <select
                value={filterApp}
                onChange={(e) => setFilterApp(e.target.value)}
                className="flex-1 bg-gray-800 border border-gray-600 rounded px-2 py-1.5 text-sm text-white"
              >
                <option value="">All Apps</option>
                {applications.map((app) => (
                  <option key={app} value={app}>
                    {app}
                  </option>
                ))}
              </select>
              <select
                value={filterRole}
                onChange={(e) => setFilterRole(e.target.value)}
                className="flex-1 bg-gray-800 border border-gray-600 rounded px-2 py-1.5 text-sm text-white"
              >
                <option value="">All Roles</option>
                {roles.map((role) => (
                  <option key={role} value={role}>
                    {role}
                  </option>
                ))}
              </select>
            </div>
            <button
              onClick={handleRequery}
              disabled={isLoading}
              className="w-full px-3 py-1.5 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 rounded text-sm text-white"
            >
              Query Elements ({filteredElements.length})
            </button>
          </div>

          {/* Elements List */}
          <div className="flex-1 overflow-y-auto">
            {filteredElements.length === 0 ? (
              <div className="p-4 text-gray-500 text-center text-sm">
                No elements found
              </div>
            ) : (
              filteredElements.map((el) => (
                <div
                  key={el.id}
                  className={`p-3 border-b border-gray-800 cursor-pointer hover:bg-gray-800 transition-colors ${
                    selectedElement?.id === el.id ? "bg-blue-900/30 border-l-2 border-l-blue-500" : ""
                  }`}
                  onClick={() => setSelectedElement(selectedElement?.id === el.id ? null : el)}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-white text-sm font-medium truncate">
                      {el.name || el.role || "Unnamed"}
                    </span>
                    <span className="text-xs text-gray-500 bg-gray-800 px-1.5 py-0.5 rounded">
                      {el.role}
                    </span>
                  </div>
                  <div className="text-xs text-gray-400 mt-1 truncate">
                    {el.application}
                  </div>
                  {selectedElement?.id === el.id && (
                    <div className="mt-2 pt-2 border-t border-gray-700 space-y-1">
                      {el.description && (
                        <p className="text-xs text-gray-400">{el.description}</p>
                      )}
                      {el.bounds && (
                        <p className="text-xs text-gray-500">
                          Position: ({el.bounds.x}, {el.bounds.y}) Size:{" "}
                          {el.bounds.width}x{el.bounds.height}
                        </p>
                      )}
                      {el.states.length > 0 && (
                        <div className="flex flex-wrap gap-1 mt-1">
                          {el.states.map((state) => (
                            <span
                              key={state}
                              className="text-xs bg-gray-700 text-gray-300 px-1.5 py-0.5 rounded"
                            >
                              {state}
                            </span>
                          ))}
                        </div>
                      )}
                      {el.actions.length > 0 && (
                        <div className="flex flex-wrap gap-1 mt-1">
                          {el.actions.map((action) => (
                            <span
                              key={action}
                              className="text-xs bg-blue-900/50 text-blue-300 px-1.5 py-0.5 rounded"
                            >
                              {action}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              ))
            )}
          </div>

          {/* Element Count Footer */}
          <div className="p-2 border-t border-gray-700 text-xs text-gray-500 text-center">
            {filteredElements.length} of {elements.length} elements
          </div>
        </div>
      </div>

      {/* Status Bar */}
      <div className="px-4 py-2 border-t border-gray-700 bg-gray-900 flex items-center justify-between text-xs text-gray-500">
        <div>
          {screenshotDimensions && (
            <span>
              Screen: {screenshotDimensions.width}x{screenshotDimensions.height}
            </span>
          )}
        </div>
        <div className="flex gap-4">
          <span>Click on screenshot to interact</span>
          <span>Press Escape to close</span>
        </div>
      </div>
    </div>
  );
}
