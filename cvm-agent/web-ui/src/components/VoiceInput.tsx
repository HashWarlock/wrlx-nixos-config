import { useVoice } from "../hooks/useVoice";

interface VoiceInputProps {
  onTranscript: (text: string) => void;
}

export function VoiceInput({ onTranscript }: VoiceInputProps) {
  const {
    isRecording,
    isProcessing,
    transcript,
    startRecording,
    stopRecording,
    clearTranscript,
  } = useVoice();

  const handleSubmit = () => {
    if (transcript.trim()) {
      onTranscript(transcript.trim());
      clearTranscript();
    }
  };

  return (
    <div className="flex items-center gap-2">
      <button
        onMouseDown={startRecording}
        onMouseUp={stopRecording}
        onMouseLeave={stopRecording}
        className={`p-2 rounded-full transition-colors ${
          isRecording
            ? "bg-red-600 hover:bg-red-700 animate-pulse"
            : "bg-gray-700 hover:bg-gray-600"
        }`}
        title="Hold to speak"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-5 w-5"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path
            fillRule="evenodd"
            d="M7 4a3 3 0 016 0v4a3 3 0 11-6 0V4zm4 10.93A7.001 7.001 0 0017 8a1 1 0 10-2 0A5 5 0 015 8a1 1 0 00-2 0 7.001 7.001 0 006 6.93V17H6a1 1 0 100 2h8a1 1 0 100-2h-3v-2.07z"
            clipRule="evenodd"
          />
        </svg>
      </button>

      {transcript && (
        <div className="flex-1 flex items-center gap-2 bg-gray-800 rounded px-3 py-1">
          <span className="text-sm text-gray-300 truncate">
            {isProcessing && <span className="text-blue-400">... </span>}
            {transcript}
          </span>
          <button
            onClick={handleSubmit}
            className="text-blue-400 hover:text-blue-300 text-sm"
          >
            Send
          </button>
          <button
            onClick={clearTranscript}
            className="text-gray-500 hover:text-gray-400 text-sm"
          >
            Clear
          </button>
        </div>
      )}
    </div>
  );
}
