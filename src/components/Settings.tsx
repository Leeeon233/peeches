import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./Settings.css";
import { Store } from "@tauri-apps/plugin-store";
import { error as logError } from "@tauri-apps/plugin-log";

interface ModelInfo {
  name: string;
  fileName: string;
  description: string;
  status: "idle" | "downloading" | "completed" | "error";
  url: string;
  progress: number;
  error?: string;
}

type ModelsRecord = Record<string, ModelInfo>;

const defaultModels: ModelsRecord = {
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

function Settings() {
  const [models, setModels] = useState<ModelsRecord>({});

  const listenDownloadProgress = async () => {
    const store = await Store.load("models.dat");
    await listen("download-progress", async (event: any) => {
      const { progress, fileName } = event.payload;
      console.log("progress: ", progress, "fileName: ", fileName);
      setModels((prev) => {
        const newModels = { ...prev };
        if (newModels[fileName]) {
          newModels[fileName] = { ...newModels[fileName], progress };
          if (progress === 100) {
            newModels[fileName] = {
              ...newModels[fileName],
              status: "completed",
            };
            store.set("models", newModels);
          }
        }
        return newModels;
      });
    });
  };

  useEffect(() => {
    let needUpdate = false;
    Store.load("models.dat").then((store) => {
      store.get<ModelsRecord>("models").then((value) => {
        if (value) {
          Object.values(value).forEach((model) => {
            if (model.status !== "completed") {
              needUpdate = true;
            }
          });
          if (needUpdate) {
            listenDownloadProgress();
          }
          setModels({...defaultModels,...value});
        } else {
          setModels(defaultModels);
          listenDownloadProgress();
        }
      });
    });
  }, []);

  const handleDownload = async (fileName: string) => {
    try {
      setModels((prev) => ({
        ...prev,
        [fileName]: {
          ...prev[fileName],
          status: "downloading",
          progress: 0,
          error: undefined,
        },
      }));

      // Start the download
      await invoke("download_model", { url: models[fileName].url, fileName });
    } catch (error) {
      logError(`Download error: ${error}`);
      setModels((prev) => ({
        ...prev,
        [fileName]: {
          ...prev[fileName],
          status: "error",
          error: "下载失败，请重试",
        },
      }));
    }
  };

  return (
    <div className="settings-container">
      {Object.values(models).map((model) => (
        <div key={model.fileName} className="model-item">
          <div className="model-info">
            <h3>{model.name}</h3>
            <p className="model-description">{model.description}</p>
            {model.error && <p className="error-message">{model.error}</p>}
          </div>
          <div className="download-section">
            {(model.status === "idle" || model.status === "error") && (
              <button
                className="download-button"
                onClick={() => handleDownload(model.fileName)}
              >
                下载
              </button>
            )}
            {model.status === "downloading" && (
              <div
                style={{
                  width: "100%",
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                <div className="progress-container">
                  <div
                    className="progress-bar"
                    style={{ width: `${model.progress}%` }}
                  ></div>
                </div>
                <span className="progress-text">
                  {model.progress.toFixed(2)}%
                </span>
              </div>
            )}
            {model.status === "completed" && (
              <div className="check-mark">✓</div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

export default Settings;
