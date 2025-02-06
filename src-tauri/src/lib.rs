use std::time::Instant;

use audio::AudioOutput;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Emitter, Manager,
};
use translate::Translator;
use whisper::Whisper;

mod audio;
mod translate;
mod whisper;

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

#[tauri::command]
async fn start_recording(
    app: AppHandle,
    output: tauri::State<'_, AudioOutput>,
    whisper: tauri::State<'_, Whisper>,
    translator: tauri::State<'_, Translator>,
) -> Result<(), String> {
    println!("start_recording");

    let mut rx = output.sender.subscribe();
    // Use the current thread runtime
    output.start_recording();
    while let Ok(sample_buf) = rx.recv().await {
        if output.is_stopped() || sample_buf.is_none() {
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
        if whisper.can_transcribe() {
            let whisper = whisper.inner().clone();
            let translator = translator.inner().clone();
            let app = app.clone();
            std::thread::spawn(move || {
                if let Some(text) = whisper.transcribe() {
                    let start_time = Instant::now();
                    println!("text: {}", text);
                    let translated_text = translator.translate(&text).unwrap();
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
    Ok(())
}

#[tauri::command]
fn stop_recording(output: tauri::State<'_, AudioOutput>) -> Result<(), String> {
    println!("stop_recording");
    output.stop_recording();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            let resource_dir = app.path().resource_dir()?;
            let model_dir = resource_dir.join("model");
            let whisper_model = model_dir.join("ggml-base-q5_1.bin");
            // let vad_model = model_dir.join("silero_vad.onnx");
            let translate_model = model_dir.join("opus-mt-en-zh.safetensors");
            let en_token = model_dir.join("tokenizer-marian-base-en.json");
            let zh_token = model_dir.join("tokenizer-marian-base-zh.json");
            let whisper = Whisper::new(whisper_model.to_str().unwrap());
            let translator = Translator::new(
                translate_model.to_str().unwrap(),
                en_token.to_str().unwrap(),
                zh_token.to_str().unwrap(),
            )?;

            app.manage(whisper);
            app.manage(translator);
            Ok(())
        })
        .manage(AudioOutput::new())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            close_app,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
