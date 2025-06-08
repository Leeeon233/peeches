import "./Settings.css";
import { useModels } from "../hooks/useModels";
import { useAtom } from "jotai";
import { modelValuesAtom } from "../store/atoms";
import { useEffect } from "react";

function Settings() {
  const { downloadModel, verifyAndSyncModels } = useModels();
  const [modelValues] = useAtom(modelValuesAtom);

  // Verify models when settings page opens
  useEffect(() => {
    verifyAndSyncModels();
  }, [verifyAndSyncModels]);

  const handleDownload = async (fileName: string) => {
    await downloadModel(fileName);
  };

  return (
    <div className="settings-container">
      {modelValues.map((model) => (
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
