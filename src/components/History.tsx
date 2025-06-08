import { useEffect, useRef, useCallback, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import './History.css';

type LyricsEvent = {
    originalText: string;
    translatedText: string;
};

interface HistoryItem {
    id: string;
    originalText: string;
    translatedText: string;
    timestamp: number;
}

function History() {
    const [history, setHistory] = useState<HistoryItem[]>([]);
    const [isAutoScrollEnabled, setIsAutoScrollEnabled] = useState<boolean>(true);
    const [highlightedIndex, setHighlightedIndex] = useState<number>(-1);
    const [_, setTranscriptionCounter] = useState<number>(0);
    const containerRef = useRef<HTMLDivElement>(null);
    const itemRefs = useRef<(HTMLDivElement | null)[]>([]);
    const userScrollTimeout = useRef<number | null>(null);

    // Listen for new lyrics events and add to history
    useEffect(() => {
        const unlisten = listen<LyricsEvent>("event", (event) => {
            const { originalText, translatedText } = event.payload;

            // Check if this is valid content to add to history
            const isValidContent = originalText &&
                translatedText &&
                originalText.trim() !== "" &&
                translatedText.trim() !== "" &&
                originalText !== "wait for audio" &&
                originalText !== "已暂停" &&
                originalText !== "BLANK_AUDIO" &&
                translatedText !== "等待音频" &&
                translatedText !== "空白";

            if (isValidContent) {
                setTranscriptionCounter(prev => {
                    const newCounter = prev + 1;

                    // Only add to history every 6 transcriptions
                    if (newCounter % 4 === 0) {
                        const newItem: HistoryItem = {
                            id: Date.now().toString() + Math.random().toString(36).substr(2, 9),
                            originalText,
                            translatedText,
                            timestamp: Date.now(),
                        };

                        setHistory(prevHistory => {
                            const updatedHistory = [...prevHistory, newItem];

                            // Auto-scroll and highlight if enabled
                            if (isAutoScrollEnabled) {
                                setTimeout(() => {
                                    setHighlightedIndex(updatedHistory.length - 1); // Will be the new item's index
                                    scrollToItem(updatedHistory.length - 1);
                                }, 100);
                            }

                            return updatedHistory;
                        });
                    }

                    return newCounter;
                });
            }
        });

        return () => {
            unlisten.then((f) => f());
        };
    }, [isAutoScrollEnabled]);

    // Scroll to specific item and center it
    const scrollToItem = useCallback((index: number) => {
        if (!containerRef.current || !itemRefs.current[index]) return;

        const container = containerRef.current;
        const item = itemRefs.current[index];

        const containerHeight = container.clientHeight;
        const itemTop = item.offsetTop;
        const itemHeight = item.clientHeight;

        // Calculate scroll position to center the item
        const scrollTop = itemTop - (containerHeight / 2) + (itemHeight / 2);

        container.scrollTo({
            top: Math.max(0, scrollTop),
            behavior: 'smooth'
        });
    }, []);

    // Handle manual scroll - disable auto-scroll temporarily
    const handleScroll = useCallback(() => {
        if (!containerRef.current) return;

        const container = containerRef.current;
        const scrollTop = container.scrollTop;
        const scrollHeight = container.scrollHeight;
        const clientHeight = container.clientHeight;

        // Clear previous timeout
        if (userScrollTimeout.current) {
            clearTimeout(userScrollTimeout.current);
        }

        // Disable auto-scroll when user scrolls manually
        if (isAutoScrollEnabled) {
            setIsAutoScrollEnabled(false);
        }

        // Check if user scrolled back to near the bottom
        const isNearBottom = scrollTop + clientHeight >= scrollHeight - 50;

        if (isNearBottom) {
            // Re-enable auto-scroll after 1 second of being at bottom
            userScrollTimeout.current = setTimeout(() => {
                setIsAutoScrollEnabled(true);
                if (history.length > 0) {
                    setHighlightedIndex(history.length - 1);
                }
            }, 200);
        }
    }, [isAutoScrollEnabled, setIsAutoScrollEnabled, history.length]);

    // Auto-scroll to latest item when new items are added and auto-scroll is enabled
    useEffect(() => {
        if (isAutoScrollEnabled && history.length > 0) {
            setHighlightedIndex(history.length - 1);
            scrollToItem(history.length - 1);
        }
    }, [history.length, isAutoScrollEnabled, scrollToItem]);

    // Initialize refs array
    useEffect(() => {
        itemRefs.current = itemRefs.current.slice(0, history.length);
    }, [history.length]);

    return (
        <div className="history-container">
            <div className="history-header" data-tauri-drag-region>
                <div className="header-spacer"></div>
                <h3 style={{ userSelect: 'none' }}>历史记录</h3>
                <div className="auto-scroll-indicator">
                    <span className={`indicator ${isAutoScrollEnabled ? 'active' : ''}`}>
                        {isAutoScrollEnabled ? '自动跟随' : '手动浏览'}
                    </span>
                </div>
            </div>
            <div
                className="history-content"
                ref={containerRef}
                onScroll={handleScroll}
            >
                {history.length === 0 ? (
                    <div className="empty-state">
                        <p>暂无历史记录</p>
                        <p>开始录制后，转录和翻译结果将显示在这里</p>
                    </div>
                ) : (
                    <div className="history-list">
                        {history.map((item, index) => (
                            <div
                                key={item.id}
                                ref={(el) => (itemRefs.current[index] = el)}
                                className={`history-item ${index === highlightedIndex ? 'highlighted' : ''
                                    }`}
                            >
                                {/* <div className="item-timestamp">
                                    {new Date(item.timestamp).toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3 })}
                                </div> */}
                                <div className="item-original">
                                    {item.originalText}
                                </div>
                                <div className="item-translated">
                                    {item.translatedText}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}

export default History; 