import { useEffect, useRef, useCallback } from 'react';
import { useAtom, useSetAtom, useAtomValue } from 'jotai';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { Store } from '@tauri-apps/plugin-store';
import { error as logError } from '@tauri-apps/plugin-log';
import {
    modelsAtom,
    setModelProgressAtom,
    updateModelAtom,
    defaultModels,
    type ModelsRecord
} from '../store/atoms';

export function useModels() {
    const [models, setModels] = useAtom(modelsAtom);
    const setModelProgress = useSetAtom(setModelProgressAtom);
    const updateModel = useSetAtom(updateModelAtom);
    const isListeningRef = useRef(false);
    const unlistenRef = useRef<(() => void) | null>(null);

    // Listen to download progress events
    const listenDownloadProgress = async () => {
        // Avoid duplicate listeners
        if (isListeningRef.current) {
            return;
        }
        isListeningRef.current = true;

        const unlisten = await listen("download-progress", async (event: any) => {
            const { progress, fileName } = event.payload;
            console.log("progress: ", progress, "fileName: ", fileName);

            setModelProgress(fileName, progress);

            if (progress === 100) {
                // Get current models state and save to store when download completes
                setTimeout(async () => {
                    try {
                        const store = await Store.load("models.dat");
                        const currentModelsFromStorage = await store.get<ModelsRecord>("models");
                        if (currentModelsFromStorage) {
                            // Get the current model info from models state (which has all the complete info)
                            const currentModelInfo = models[fileName];
                            const updatedModels = {
                                ...currentModelsFromStorage,
                                [fileName]: {
                                    ...currentModelInfo,  // Use complete model info
                                    status: "completed" as const,
                                    progress: 100,
                                },
                            };
                            await store.set("models", updatedModels);
                        }
                    } catch (error) {
                        console.error("Error saving to store:", error);
                    }

                    // Unlisten after download completes
                    if (unlistenRef.current) {
                        unlistenRef.current();
                        unlistenRef.current = null;
                        isListeningRef.current = false;
                    }
                }, 100); // Small delay to ensure state is updated
            }
        });

        // Save the unlisten function
        unlistenRef.current = unlisten;
    };

    // Verify models and sync with store
    const verifyAndSyncModels = useCallback(async () => {
        try {
            // Get verification results from backend
            const verificationResults = await invoke<Record<string, boolean>>("verify_models");

            const store = await Store.load("models.dat");
            const storedModels = await store.get<ModelsRecord>("models") || {};

            // Merge with defaultModels to ensure we have all required models
            const currentModels = { ...defaultModels, ...storedModels };
            let needsUpdate = false;

            // Check each model and update status if needed
            for (const [fileName, modelInfo] of Object.entries(currentModels)) {
                const fileExists = verificationResults[fileName] || false;

                // If model is marked as completed but file doesn't exist, reset status
                if (modelInfo.status === "completed" && !fileExists) {
                    currentModels[fileName] = {
                        ...modelInfo,
                        status: "idle",
                        progress: 0,
                        error: undefined,
                    };
                    needsUpdate = true;
                }
                // If file exists but model is idle, mark as completed
                else if (modelInfo.status === "idle" && fileExists) {
                    currentModels[fileName] = {
                        ...modelInfo,
                        status: "completed",
                        progress: 100,
                    };
                    needsUpdate = true;
                }
            }

            // Update store if needed
            if (needsUpdate) {
                await store.set("models", currentModels);
            }

            // Update state
            setModels(currentModels);
        } catch (error) {
            console.error("Error verifying models:", error);
            // Fallback to loading from store
            const store = await Store.load("models.dat");
            const value = await store.get<ModelsRecord>("models");
            if (value) {
                setModels({ ...defaultModels, ...value });
            } else {
                setModels(defaultModels);
            }
        }
    }, [setModels]);

    // Initialize models from store with verification
    useEffect(() => {
        verifyAndSyncModels();
    }, [verifyAndSyncModels]);

    // Download model function
    const downloadModel = async (fileName: string) => {
        try {
            // Start listening to download progress when download begins
            await listenDownloadProgress();

            updateModel(fileName, {
                status: "downloading",
                progress: 0,
                error: undefined,
            });

            // Start the download
            await invoke("download_model", {
                url: models[fileName].url,
                fileName
            });
        } catch (error) {
            logError(`Download error: ${error}`);
            updateModel(fileName, {
                status: "error",
                error: "下载失败，请重试",
            });

            // Unlisten on error
            if (unlistenRef.current) {
                unlistenRef.current();
                unlistenRef.current = null;
                isListeningRef.current = false;
            }
        }
    };

    return {
        models,
        downloadModel,
        verifyAndSyncModels,
    };
} 