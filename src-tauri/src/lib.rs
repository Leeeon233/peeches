use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
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
    fileName: String,
    status: String,
}

struct AppState {
    whisper: Arc<Mutex<Option<Whisper>>>,
    translator: Arc<Mutex<Option<Translator>>>,
}

impl AppState {
    fn set_model(&self, app: &AppHandle, file_name: &str) -> Result<(), String> {
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

    fn is_ready(&self) -> bool {
        self.whisper.lock().unwrap().is_some() && self.translator.lock().unwrap().is_some()
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
    originalText: String,
    translatedText: String,
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
    output: tauri::State<'_, AudioOutput>,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    println!("start_recording");
    if !state.is_ready() {
        open_settings(app)?;
        return Ok(false);
    }

    let mut rx = output.sender.subscribe();
    // Use the current thread runtime
    output.start_recording();
    let output_clone = output.inner().clone();
    let whisper = state.whisper.lock().unwrap().as_ref().unwrap().clone();
    let translator = state.translator.lock().unwrap().as_ref().unwrap().clone();
    tokio::spawn(async move {
        while let Ok(sample_buf) = rx.recv().await {
            if output_clone.is_stopped() || sample_buf.is_none() {
                break;
            }
            let audio_buf_list = sample_buf.unwrap().audio_buf_list::<2>().unwrap();
            let buffer_list = audio_buf_list.list();
            let samples = unsafe {
                let buffer = buffer_list.buffers[0];
                std::slice::from_raw_parts(
                    buffer.data as *const f32,
                    buffer.data_bytes_size as usize / std::mem::size_of::<f32>(),
                )
            };
            whisper.add_new_samples(samples, 48000, 1);
            let whisper_clone = whisper.clone();
            let translator_clone = translator.clone();
            if whisper.can_transcribe() {
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Some(text) = whisper_clone.transcribe() {
                        let start_time = Instant::now();
                        println!("text: {}", text);
                        let translated_text = translator_clone.translate(&text).unwrap();
                        app.emit(
                            "event",
                            Event {
                                originalText: text,
                                translatedText: translated_text,
                            },
                        )
                        .unwrap();
                        let elapsed_time = start_time.elapsed();
                        println!("transcribe_in_background time: {:?}", elapsed_time);
                    }
                });
            }
        }
    });
    Ok(true)
}

#[tauri::command]
fn stop_recording(output: tauri::State<'_, AudioOutput>) -> Result<(), String> {
    println!("stop_recording");
    output.stop_recording();
    Ok(())
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    // Check if settings window already exists and focus it
    if let Some(settings_window) = app.get_webview_window("settings") {
        settings_window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let settings = WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("/#/settings".into()),
        )
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .inner_size(400.0, 300.0)
        .resizable(false)
        .center()
        .build();

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

    // Download the file
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;

    let total_size = response
        .content_length()
        .ok_or_else(|| "Failed to get content length".to_string())?;
    let mut file = fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
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
    state.set_model(&app, &file_name)?;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        // .plugin(tauri_plugin_log::Builder::new().build())
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

            #[cfg(target_os = "macos")]
            {
                // Get the main window
                let window = app.get_webview_window("main").unwrap();
                window.set_shadow(false)?;
                // Hide dock icon
                app.set_activation_policy(tauri::ActivationPolicy::Accessory); // Get the primary monitor size
                if let Some(monitor) = window.primary_monitor().unwrap() {
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
            }

            // let whisper = Whisper::new(whisper_model.to_str().unwrap());
            // let translator = Translator::new(
            //     translate_model.to_str().unwrap(),
            //     en_token.to_str().unwrap(),
            //     zh_token.to_str().unwrap(),
            // )?;

            let model_dir = model_dir(app.handle())?;

            // Get model store state
            let store = app.store("models.dat")?;
            let models = store
                .get("models")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let models: HashMap<String, ModelInfo> =
                serde_json::from_value(models).unwrap_or_default();

            let whisper_model = {
                if let Some(info) = models.get("ggml-base-q5_1.bin") {
                    if info.status == "completed" {
                        let model_path = model_dir.join(&info.fileName);
                        if model_path.exists() {
                            let whisper = Whisper::new(model_path.to_str().unwrap());
                            Some(whisper)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let translator = {
                if let Some(info) = models.get("opus-mt-en-zh.bin") {
                    if info.status == "completed" {
                        let model_path = model_dir.join(&info.fileName);
                        if model_path.exists() {
                            let (en_token, zh_token) = get_token_path(app.handle());
                            let translator = Translator::new(
                                model_path.to_str().unwrap(),
                                en_token.to_str().unwrap(),
                                zh_token.to_str().unwrap(),
                            )?;
                            Some(translator)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            app.manage(AppState {
                whisper: Arc::new(Mutex::new(whisper_model)),
                translator: Arc::new(Mutex::new(translator)),
            });

            Ok(())
        })
        .manage(AudioOutput::new())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            close_app,
            start_recording,
            stop_recording,
            open_settings,
            download_model
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
