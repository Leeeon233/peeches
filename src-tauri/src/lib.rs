use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::{
    fs,
    sync::{Arc, Mutex},
};

use audio::AudioOutput;
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Emitter, Manager, WebviewWindowBuilder,
};
use tauri_plugin_store::StoreExt as _;
use translate::Translator;
use whisper::Whisper;

mod audio;
mod translate;
mod whisper;

#[derive(Serialize, Deserialize, Clone)]
struct ModelInfo {
    name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    status: String,
}

#[derive(Clone)]
struct AppState {
    audio_output: Arc<Mutex<AudioOutput>>,
    whisper: Arc<Mutex<Option<Whisper>>>,
    translator: Arc<Mutex<Option<Translator>>>,
}

impl AppState {
    pub fn new(app: AppHandle) -> anyhow::Result<Self> {
        let (audio_sender, audio_receiver) = mpsc::channel();
        let (transcript_sender, transcript_receiver) = mpsc::channel();
        let audio_output = AudioOutput::new(audio_sender)?;
        let whisper = Arc::new(Mutex::new(None::<Whisper>));
        let whisper_arc = whisper.clone();
        let translator = Arc::new(Mutex::new(None::<Translator>));
        let translator_arc = translator.clone();

        std::thread::spawn(move || {
            while let Ok(samples) = audio_receiver.recv() {
                let mut whisper = whisper_arc.lock().unwrap();
                if whisper.is_none() {
                    continue;
                }
                let text = whisper.as_mut().unwrap().transcribe(samples).unwrap();
                transcript_sender.send(text).unwrap();
            }
        });

        std::thread::spawn(move || {
            while let Ok(text) = transcript_receiver.recv() {
                let mut translator = translator_arc.lock().unwrap();
                if translator.is_none() {
                    continue;
                }
                let translated_text = translator.as_mut().unwrap().translate(&text).unwrap();
                if text == " [BLANK_AUDIO]" {
                    app.emit(
                        "event",
                        Event {
                            original_text: "BLANK_AUDIO".to_string(),
                            translated_text: "空白".to_string(),
                        },
                    )
                    .unwrap();
                    continue;
                }
                log::debug!("original_text: {}", text);
                log::debug!("translated_text: {}", translated_text);
                app.emit(
                    "event",
                    Event {
                        original_text: text,
                        translated_text,
                    },
                )
                .unwrap();
            }
        });

        Ok(Self {
            audio_output: Arc::new(Mutex::new(audio_output)),
            whisper,
            translator,
        })
    }

    fn is_ready(&self) -> bool {
        self.whisper.lock().unwrap().is_some() && self.translator.lock().unwrap().is_some()
    }

    fn set_model(&self, app: &AppHandle, file_name: &str) -> Result<(), String> {
        log::info!("create model: {}", file_name);
        match file_name {
            "ggml-base-q5_1.bin" => {
                self.whisper
                    .lock()
                    .unwrap()
                    .replace(Self::create_whisper(app, file_name)?);
            }
            "opus-mt-en-zh.bin" => {
                self.translator
                    .lock()
                    .unwrap()
                    .replace(Self::create_translator(app, file_name)?);
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn create_whisper(app: &AppHandle, file_name: &str) -> Result<Whisper, String> {
        let model_dir = model_dir(app)?;
        let whisper = Whisper::new(model_dir.join(file_name).to_str().unwrap());
        Ok(whisper)
    }

    fn create_translator(app: &AppHandle, file_name: &str) -> Result<Translator, String> {
        let model_dir = model_dir(app)?;
        let (en_token, zh_token) = get_token_path(app);
        Translator::new(
            model_dir.join(file_name).to_str().unwrap(),
            en_token.to_str().unwrap(),
            zh_token.to_str().unwrap(),
        )
        .map_err(|e| e.to_string())
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn close_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[derive(Serialize, Clone)]
struct Event {
    #[serde(rename = "originalText")]
    original_text: String,
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[derive(Serialize, Clone)]
pub struct DownloadProgress {
    #[serde(rename = "fileName")]
    file_name: String,
    progress: f32,
    total_size: u64,
    downloaded: u64,
}

#[tauri::command]
async fn start_recording(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    log::info!("start_recording");
    if !state.is_ready() {
        open_settings(app).await?;
        return Ok(false);
    }
    app.emit(
        "event",
        Event {
            original_text: "wait for audio".to_string(),
            translated_text: "等待音频".to_string(),
        },
    )
    .unwrap();

    state
        .audio_output
        .lock()
        .unwrap()
        .start_recording()
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
fn stop_recording(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    log::info!("stop_recording");
    state.audio_output.lock().unwrap().stop_recording();
    app.emit(
        "event",
        Event {
            original_text: "已暂停".to_string(),
            translated_text: "".to_string(),
        },
    )
    .unwrap();
    Ok(())
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    // Check if settings window already exists and focus it
    if let Some(settings_window) = app.get_webview_window("settings") {
        settings_window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let mut builder = WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("/#/settings".into()),
        )
        .inner_size(400.0, 300.0)
        .resizable(false)
        .title("Model Download")
        .center();

        #[cfg(target_os = "macos")]
        {
            builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true);
        }

        let settings = builder.build();
        match settings {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[tauri::command]
async fn download_model(
    app: AppHandle,
    url: String,
    file_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let model_dir = model_dir(&app)?;
    let file_path = model_dir.join(&file_name);
    log::info!("download model save to: {}", file_path.to_str().unwrap());
    let mut total_size = 0;
    let mut downloaded = 0;

    if file_path.exists() {
        log::info!("model already exists: {}", file_path.to_str().unwrap());
        // TODO: check file md5
    } else {
        // Download the file
        let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;

        total_size = response
            .content_length()
            .ok_or_else(|| "Failed to get content length".to_string())?;
        let mut file = fs::File::create(&file_path).map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        let mut last_update = std::time::Instant::now();
        while let Some(item) = stream.next().await {
            let chunk = item.map_err(|e| e.to_string())?;
            std::io::copy(&mut chunk.as_ref(), &mut file).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;

            // Only update progress every 200ms
            let now = std::time::Instant::now();
            if now.duration_since(last_update).as_millis() >= 200 {
                let progress = (downloaded as f32 / total_size as f32) * 100.0;
                log::info!("download progress: {} %", progress);
                app.emit(
                    "download-progress",
                    DownloadProgress {
                        file_name: file_name.clone(),
                        progress,
                        total_size,
                        downloaded,
                    },
                )
                .map_err(|e| e.to_string())?;
                last_update = now;
            }
        }
    }
    state.set_model(&app, &file_name)?;
    log::debug!("download model: {} completed", file_name);
    app.emit(
        "download-progress",
        DownloadProgress {
            file_name,
            progress: 100.0,
            total_size,
            downloaded,
        },
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn get_token_path(app: &AppHandle) -> (PathBuf, PathBuf) {
    let resource_dir = app.path().resource_dir().unwrap();
    let model_dir = resource_dir.join("model");
    let en_token = model_dir.join("tokenizer-marian-base-en.json");
    let zh_token = model_dir.join("tokenizer-marian-base-zh.json");
    (en_token, zh_token)
}

fn model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let model_dir = app_dir.join("model");
    fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;
    Ok(model_dir)
}

#[tauri::command]
async fn show_main_window(window: tauri::Window) {
    window.get_window("main").unwrap().show().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .targets(vec![
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("Peeches".to_string()),
                    })
                    .filter(|meta| {
                        matches!(meta.level(), log::Level::Info)
                            || matches!(meta.level(), log::Level::Error)
                            || matches!(meta.level(), log::Level::Warn)
                    }),
                ])
                .build(),
        )
        .setup(|app| {
            if let Some(tray_icon) = app.tray_by_id("tray") {
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&quit_i])?;
                tray_icon.set_menu(Some(menu))?;
                tray_icon.on_menu_event(|app, e| {
                    if e.id() == "quit" {
                        app.exit(0);
                    }
                });
            }
            // Get the main window
            let window = app.get_webview_window("main").unwrap();
            #[cfg(target_os = "macos")]
            {
                // Hide dock icon
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            window.set_shadow(false)?;

            if let Some(monitor) = window.primary_monitor().unwrap() {
                window.set_min_size(Some(tauri::Size::Physical(tauri::PhysicalSize {
                    width: 300 * monitor.scale_factor() as u32,
                    height: 120 * monitor.scale_factor() as u32,
                })))?;
                let screen_size = monitor.size();
                // Calculate window width (60% of screen width)
                let window_width = (screen_size.width as f64 * 0.4) as u32;

                // Calculate x position to center window
                let x = (screen_size.width - window_width) / 2;

                // Set window position at bottom center
                // Leave 20px margin from bottom
                let y = screen_size.height - (320_f64 * monitor.scale_factor()) as u32;
                // Update window size and position
                window
                    .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                        width: window_width,
                        height: (148_f64 * monitor.scale_factor()) as u32,
                    }))
                    .unwrap();

                window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                        x: x as i32,
                        y: y as i32,
                    }))
                    .unwrap();
            }

            let app_state = AppState::new(app.handle().clone())?;

            let model_dir = model_dir(app.handle())?;

            // Get model store state
            let store = app.store("models.dat")?;
            let models = store
                .get("models")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let mut models: HashMap<String, ModelInfo> =
                serde_json::from_value(models).unwrap_or_default();

            if let Some(info) = models.get("ggml-base-q5_1.bin") {
                let model_path = model_dir.join(&info.file_name);
                if info.status == "completed" && model_path.exists() {
                    let whisper = Whisper::new(model_path.to_str().unwrap());
                    app_state.whisper.lock().unwrap().replace(whisper);
                } else {
                    models.remove("ggml-base-q5_1.bin");
                    store.set("models", serde_json::to_value(&models).unwrap());
                }
            };

            if let Some(info) = models.get("opus-mt-en-zh.bin") {
                let model_path = model_dir.join(&info.file_name);
                if info.status == "completed" && model_path.exists() {
                    let (en_token, zh_token) = get_token_path(app.handle());
                    let translator = Translator::new(
                        model_path.to_str().unwrap(),
                        en_token.to_str().unwrap(),
                        zh_token.to_str().unwrap(),
                    )?;
                    app_state.translator.lock().unwrap().replace(translator);
                } else {
                    models.remove("opus-mt-en-zh.bin");
                    store.set("models", serde_json::to_value(&models).unwrap());
                }
            };
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            close_app,
            start_recording,
            stop_recording,
            open_settings,
            download_model,
            show_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
