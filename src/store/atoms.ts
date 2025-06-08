import { atom } from 'jotai';
import { atomWithStorage } from 'jotai/utils';

// Model related types
export interface ModelInfo {
    name: string;
    fileName: string;
    description: string;
    status: "idle" | "downloading" | "completed" | "error";
    url: string;
    progress: number;
    error?: string;
}

export type ModelsRecord = Record<string, ModelInfo>;

// Default models configuration
export const defaultModels: ModelsRecord = {
    "ggml-base-q5_1.bin": {
        name: "转录模型",
        fileName: "ggml-base-q5_1.bin",
        description: "whisper ggml base-q5_1",
        status: "idle",
        progress: 0,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin",
    },
    "opus-mt-en-zh.bin": {
        name: "翻译模型",
        fileName: "opus-mt-en-zh.bin",
        description: "opus-mt-en-zh",
        status: "idle",
        progress: 0,
        url: "https://huggingface.co/Helsinki-NLP/opus-mt-en-zh/resolve/refs%2Fpr%2F26/model.safetensors",
    },
};

// Models state atom (persisted in localStorage)
export const modelsAtom = atomWithStorage<ModelsRecord>('models', defaultModels);

// Text display atoms
export const originalTextAtom = atom<string>('');
export const translatedTextAtom = atom<string>('');

// UI state atoms
export const isPinnedAtom = atomWithStorage<boolean>('isPinned', false);
export const isHoveredAtom = atom<boolean>(false);
export const isRecordingAtom = atom<boolean>(false);

// Derived atoms
export const textDisplayClassesAtom = atom((get) => {
    const isPinned = get(isPinnedAtom);
    const isHovered = get(isHoveredAtom);

    return [
        "text-display",
        // Only show hover background when not pinned
        !isPinned && isHovered ? "show-hover-bg" : "",
        // Only show buttons when hovered
        isHovered ? "show-buttons" : "",
    ]
        .filter(Boolean)
        .join(" ");
});

// Model-specific derived atoms
export const modelValuesAtom = atom((get) => {
    const models = get(modelsAtom);
    return Object.values(models);
});

export const getModelByFileNameAtom = atom(
    null,
    (get, set, fileName: string) => {
        const models = get(modelsAtom);
        return models[fileName];
    }
);

// Model update actions
export const updateModelAtom = atom(
    null,
    (get, set, fileName: string, updates: Partial<ModelInfo>) => {
        const models = get(modelsAtom);
        const updatedModels = {
            ...models,
            [fileName]: {
                ...models[fileName],
                ...updates,
            },
        };
        set(modelsAtom, updatedModels);
    }
);

export const setModelProgressAtom = atom(
    null,
    (get, set, fileName: string, progress: number) => {
        const models = get(modelsAtom);
        const updatedModels = {
            ...models,
            [fileName]: {
                ...models[fileName],
                progress,
                ...(progress === 100 && { status: "completed" as const }),
            },
        };
        set(modelsAtom, updatedModels);
    }
); 