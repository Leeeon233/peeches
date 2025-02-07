import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {  Pause, Pin, PinOff, Play, Settings, X } from "lucide-react";

type Event = {
  originalText: string;
  translatedText: string;
};

function Lyrics() {
  const [originalText, setOriginalText] = useState("");
  const [translatedText, setTranslatedText] = useState("");
  const [isPinned, setIsPinned] = useState(false);
  const [isHovered, setIsHovered] = useState(false);
  const [isRecording, setIsRecording] = useState(false);

  useEffect(() => {
    const unlisten = listen<Event>("event", (event) => {
      const { originalText, translatedText } = event.payload;
      setOriginalText(originalText);
      if (translatedText) {
        setTranslatedText(translatedText);
      }
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handlePin = async () => {
    setIsPinned(!isPinned);
  };

  const handleRecording = async () => {
    if (isRecording) {
       invoke("stop_recording");
       setOriginalText("");
       setTranslatedText("");
       setIsRecording(false);
    } else {
       if (await invoke("start_recording")){
        setIsRecording(true);
       }
    }
  };

  const handleMouseEnter = () => {
    setIsHovered(true);
  };

  const handleMouseLeave = () => {
    setIsHovered(false);
  };

  // 计算 CSS 类名
  const textDisplayClasses = [
    'text-display',
    // 只在非固定状态下显示悬停背景
    !isPinned && isHovered ? 'show-hover-bg' : '',
    // 只在鼠标悬停时显示按钮组
    isHovered ? 'show-buttons' : ''
  ].filter(Boolean).join(' ');

  return (
    <div 
      className="translation-container"
      // Only allow drag when not pinned
      {...(!isPinned && { 'data-tauri-drag-region': true })}
    >
      <div  
        className={textDisplayClasses}
        // Only allow drag when not pinned
        {...(!isPinned && { 'data-tauri-drag-region': true })}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        <div className="button-group"
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
            className={`pin-button ${isPinned ? 'active' : ''}`}
            onClick={() => {
              handlePin();
            }}
            title={isPinned ? "取消固定" : "固定窗口"}
          >
            {isPinned ?  <PinOff size={16}/> : <Pin size={16}/>}
          </button>
          <button 
            onClick={handleRecording}
            title={isRecording ? "停止录制" : "开始录制"}
            className={`record-button ${isRecording ? 'recording' : ''}`}
          >
            {isRecording ?<Pause size={16}/> : <Play size={16} />}
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
          {...(!isPinned && { 'data-tauri-drag-region': true })}
        >
          {originalText || "等待输入..."}
        </div>
        <div 
          className="translated-text" 
          {...(!isPinned && { 'data-tauri-drag-region': true })}
        >
          {translatedText || "等待翻译..."}
        </div>
      </div>
    </div>
  );
}

export default Lyrics; 