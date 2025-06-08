import { useEffect, useCallback } from 'react';
import { useAtom, useSetAtom } from 'jotai';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import {
    originalTextAtom,
    translatedTextAtom,
    isPinnedAtom,
    isHoveredAtom,
    isRecordingAtom,
    textDisplayClassesAtom,
} from '../store/atoms';

type LyricsEvent = {
    originalText: string;
    translatedText: string;
};

export function useLyrics() {
    const [originalText, setOriginalText] = useAtom(originalTextAtom);
    const [translatedText, setTranslatedText] = useAtom(translatedTextAtom);
    const [isPinned, setIsPinned] = useAtom(isPinnedAtom);
    const [isHovered, setIsHovered] = useAtom(isHoveredAtom);
    const [isRecording, setIsRecording] = useAtom(isRecordingAtom);
    const [textDisplayClasses] = useAtom(textDisplayClassesAtom);

    // Initialize event listeners
    useEffect(() => {
        invoke("show_main_window");

        const unlisten = listen<LyricsEvent>("event", (event) => {
            const { originalText, translatedText } = event.payload;
            setOriginalText(originalText);
            setTranslatedText(translatedText);
        });

        return () => {
            unlisten.then((f) => f());
        };
    }, [setOriginalText, setTranslatedText]);

    // Pin/unpin window
    const handlePin = useCallback(async () => {
        setIsPinned(!isPinned);
    }, [isPinned, setIsPinned]);

    // Start/stop recording
    const handleRecording = useCallback(async () => {
        if (isRecording) {
            await invoke("stop_recording");
            setOriginalText("");
            setTranslatedText("");
            setIsRecording(false);
        } else {
            if (await invoke("start_recording")) {
                setIsRecording(true);
            }
        }
    }, [isRecording, setIsRecording, setOriginalText, setTranslatedText]);

    // Mouse events
    const handleMouseEnter = useCallback(() => {
        setIsHovered(true);
    }, [setIsHovered]);

    const handleMouseLeave = useCallback(() => {
        setIsHovered(false);
    }, [setIsHovered]);

    return {
        // State
        originalText,
        translatedText,
        isPinned,
        isHovered,
        isRecording,
        textDisplayClasses,

        // Actions
        handlePin,
        handleRecording,
        handleMouseEnter,
        handleMouseLeave,
        setIsHovered,
    };
} 