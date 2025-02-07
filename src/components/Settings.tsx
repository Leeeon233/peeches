import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './Settings.css';

interface ModelInfo {
  name: string;
  fileName: string;
  description: string;
  status: 'idle' | 'downloading' | 'completed' | 'error';
  url: string;
  progress: number;
  error?: string;
}

 function Settings() {
  const [models, setModels] = useState<ModelInfo[]>([
    {
      name: '转录模型',
      fileName: 'ggml-base-q5_1.bin',
      description: 'whisper ggml base-q5_1',
      status: 'idle',
      progress: 0,
      url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin',
    },
    {
      name: '翻译模型',
      fileName: 'opus-mt-en-zh.bin',
      description: 'opus-mt-en-zh',
      status: 'downloading',
      progress: 100,
      url: 'https://huggingface.co/Helsinki-NLP/opus-mt-en-zh/resolve/refs%2Fpr%2F26/model.safetensors',
    },
  ]);

  const listenDownloadProgress = async () => {
     await listen('download-progress', async (event: any) => {
      const { progress,index } = event.payload;
      console.log("progress: ", progress, "index: ", index);
      setModels(prev => {
        const newModels = [...prev];
        newModels[index] = { ...newModels[index], progress };
        if (progress === 100) {
          newModels[index] = { ...newModels[index], status: 'completed' };
          
        }
        return newModels;
      })});
  }

  useEffect(() => {
    listenDownloadProgress();
  }, []);
  
  const handleDownload = async (index: number) => {
    try {
      setModels(prev => {
        const newModels = [...prev];
        newModels[index] = { ...newModels[index], status: 'downloading', progress: 0 };
        return newModels;
      });

      // Start the download
      await invoke('download_model', { url: models[index].url,  index, fileName: models[index].fileName });

      
    } catch (error) {
      console.error('Download error:', error);
      setModels(prev => {
        const newModels = [...prev];
        newModels[index] = {
          ...newModels[index],
          status: 'error',
          error: '下载失败，请重试',
        };
        return newModels;
      });
    }
  };

  return (
    <div className="settings-container">
      {models.map((model, index) => (
        <div key={index} className="model-item">
          <div className="model-info">
            <h3>{model.name}</h3>
            <p className="model-description">{model.description}</p>
            {model.error && <p className="error-message">{model.error}</p>}
          </div>
          <div className="download-section">
            {(model.status === 'idle' || model.status === 'error') && (
              <button
                className="download-button"
                onClick={() => handleDownload(index)}
              >
                下载
              </button>
            )}
            {model.status === 'downloading' && (
                <div style={{width: '100%', display: 'flex', flexDirection: 'column', gap: '4px', alignItems: 'center', justifyContent: 'center'}}> <div className="progress-container">
                <div 
                  className="progress-bar"
                  style={{ width: `${model.progress}%` }}
                ></div>
               
              </div> <span className="progress-text">{model.progress.toFixed(2)}%</span></div>
             
            )}
            {model.status === 'completed' && (
              <div className="check-mark">✓</div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
} 

export default Settings;