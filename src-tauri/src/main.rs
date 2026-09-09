#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use typeit_lib::audio;
use typeit_lib::downloader;
use typeit_lib::recorder::{Recorder, RecordingState};
use typeit_lib::settings::Settings;
use typeit_lib::transcribe_local;

struct AppState {
    recorder: Recorder,
    settings: Mutex<Settings>,
    app_dir: PathBuf,
}

fn get_app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.typeit.app")
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.app_dir)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn list_microphones() -> Vec<audio::MicDevice> {
    audio::list_microphones()
}

#[tauri::command]
fn get_recording_state(state: State<AppState>) -> RecordingState {
    state.recorder.get_state()
}

#[tauri::command]
fn check_model_downloaded(state: State<AppState>, model_size: String) -> bool {
    let model_file = transcribe_local::model_filename(&model_size);
    state.app_dir.join(&model_file).exists()
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    model_size: String,
) -> Result<(), String> {
    let url = transcribe_local::model_download_url(&model_size);
    let model_file = transcribe_local::model_filename(&model_size);
    let dest = state.app_dir.join(&model_file);
    downloader::download_model(app, &url, &dest).await
}

#[tauri::command]
async fn toggle_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    do_toggle_recording(&app, &state).await
}

/// Shared logic for toggle recording, used by both the Tauri command and hotkey handler.
async fn do_toggle_recording(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let current_state = state.recorder.get_state();
    match current_state {
        RecordingState::Ready => {
            let mic = state.settings.lock().unwrap().microphone.clone();
            state.recorder.start_recording(app, &mic)?;
            Ok("recording".to_string())
        }
        RecordingState::Recording => {
            let settings = state.settings.lock().unwrap().clone();
            let result = state
                .recorder
                .stop_and_transcribe(app, &settings, &state.app_dir)
                .await?;
            Ok(result)
        }
        RecordingState::Transcribing => {
            Err("Currently transcribing, please wait".to_string())
        }
    }
}

fn main() {
    let app_dir = get_app_dir();
    let settings = Settings::load(&app_dir);
    let initial_hotkey = settings.hotkey.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            recorder: Recorder::new(),
            settings: Mutex::new(settings),
            app_dir,
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_microphones,
            get_recording_state,
            check_model_downloaded,
            download_model,
            toggle_recording,
        ])
        .setup(move |app| {
            // Create the overlay window (small mic icon, top-right, always on top)
            let monitor = app.primary_monitor().ok().flatten();
            let (x, y) = if let Some(m) = monitor {
                let size = m.size();
                let scale = m.scale_factor();
                let logical_w = size.width as f64 / scale;
                ((logical_w - 60.0) as i32, 10_i32)
            } else {
                (1380, 10)
            };

            let overlay = WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("src/overlay.html".into()),
            )
            .title("")
            .inner_size(50.0, 50.0)
            .position(x as f64, y as f64)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .shadow(false)
            .build();

            match overlay {
                Ok(_) => println!("[TypeIt] Overlay window created"),
                Err(e) => eprintln!("[TypeIt] Failed to create overlay: {}", e),
            }

            let handle = app.handle().clone();

            println!("[TypeIt] Registering global shortcut: {}", initial_hotkey);

            // macOS repeats "Pressed" while the key is held down; without this,
            // toggle mode would start/stop rapidly for every repeat instead of once.
            let key_down = std::sync::atomic::AtomicBool::new(false);

            match app.global_shortcut().on_shortcut(
                initial_hotkey.as_str(),
                move |_app, shortcut, event| {
                    println!("[TypeIt] Hotkey event: {:?} state={:?}", shortcut, event.state);
                    let handle = handle.clone();
                    let state = handle.state::<AppState>();
                    let mode = state.settings.lock().unwrap().recording_mode.clone();
                    println!("[TypeIt] Recording mode: {}", mode);

                    match event.state {
                        ShortcutState::Pressed => {
                            if key_down.swap(true, std::sync::atomic::Ordering::SeqCst) {
                                return; // repeat fire while held, ignore
                            }
                            tauri::async_runtime::spawn(async move {
                                let state = handle.state::<AppState>();
                                match mode.as_str() {
                                    "toggle" => {
                                        println!("[TypeIt] Toggle mode: calling do_toggle_recording");
                                        match do_toggle_recording(&handle, state.inner()).await {
                                            Ok(result) => println!("[TypeIt] Toggle result: {}", result),
                                            Err(e) => eprintln!("[TypeIt] Toggle error: {}", e),
                                        }
                                    }
                                    "push-to-talk" => {
                                        let current = state.recorder.get_state();
                                        println!("[TypeIt] PTT mode, current state: {:?}", current);
                                        if current == RecordingState::Ready {
                                            let mic = state
                                                .settings
                                                .lock()
                                                .unwrap()
                                                .microphone
                                                .clone();
                                            match state.recorder.start_recording(&handle, &mic) {
                                                Ok(_) => println!("[TypeIt] Recording started"),
                                                Err(e) => eprintln!("[TypeIt] Start recording error: {}", e),
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            });
                        }
                        ShortcutState::Released => {
                            key_down.store(false, std::sync::atomic::Ordering::SeqCst);
                            if mode == "push-to-talk" {
                                tauri::async_runtime::spawn(async move {
                                    let state = handle.state::<AppState>();
                                    let current = state.recorder.get_state();
                                    if current == RecordingState::Recording {
                                        let settings =
                                            state.settings.lock().unwrap().clone();
                                        match state.recorder.stop_and_transcribe(
                                            &handle,
                                            &settings,
                                            &state.app_dir,
                                        ).await {
                                            Ok(result) => println!("[TypeIt] Transcription: {}", result),
                                            Err(e) => eprintln!("[TypeIt] Transcription error: {}", e),
                                        }
                                    }
                                });
                            }
                        }
                    }
                },
            ) {
                Ok(_) => println!("[TypeIt] Global shortcut registered successfully"),
                Err(e) => eprintln!("[TypeIt] ERROR: Failed to register global shortcut: {}", e),
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
