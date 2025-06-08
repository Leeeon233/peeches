import { invoke } from "@tauri-apps/api/core";
import { Pause, Pin, PinOff, Play, Settings, X, History } from "lucide-react";
import { useLyrics } from "../hooks/useLyrics";

function Lyrics() {
  const {
    originalText,
    translatedText,
    isPinned,
    isRecording,
    textDisplayClasses,
    handlePin,
    handleRecording,
    handleMouseEnter,
    handleMouseLeave,
    handleHistoryToggle,
    setIsHovered,
  } = useLyrics();

  return (
    <div
      className="translation-container"
      // Only allow drag when not pinned
      {...(!isPinned && { "data-tauri-drag-region": true })}
    >
      <div
        className={textDisplayClasses}
        // Only allow drag when not pinned
        {...(!isPinned && { "data-tauri-drag-region": true })}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        <div
          className="button-group"
          onMouseEnter={(e) => {
            e.stopPropagation();
            setIsHovered(true);
          }}
          onMouseLeave={(e) => {
            e.stopPropagation();
            setIsHovered(false);
          }}
        >
          <button
            className={`pin-button ${isPinned ? "active" : ""}`}
            onClick={() => {
              handlePin();
            }}
            title={isPinned ? "取消固定" : "固定窗口"}
          >
            {isPinned ? <PinOff size={16} /> : <Pin size={16} />}
          </button>
          <button
            onClick={handleRecording}
            title={isRecording ? "停止录制" : "开始录制"}
            className={`record-button ${isRecording ? "recording" : ""}`}
          >
            {isRecording ? <Pause size={16} /> : <Play size={16} />}
          </button>
          <button
            onClick={handleHistoryToggle}
            title="历史记录"
            className="history-button"
          >
            <History size={16} />
          </button>
          <button
            onClick={() => {
              invoke("open_settings");
            }}
            title="设置"
          >
            <Settings size={16} />
          </button>
          <button
            className="close-button"
            onClick={() => {
              invoke("close_app");
            }}
            title="关闭应用"
          >
            <X size={16} />
          </button>
        </div>
        <div
          className="original-text"
          {...(!isPinned && { "data-tauri-drag-region": true })}
        >
          {originalText || "等待输入..."}
        </div>
        <div
          className="translated-text"
          {...(!isPinned && { "data-tauri-drag-region": true })}
        >
          {translatedText || "等待翻译..."}
        </div>
      </div>
    </div>
  );
}

export default Lyrics;
