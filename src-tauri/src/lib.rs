mod audio;
mod autostart;
mod config;
mod poller;
mod usb;

use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tauri::{
    Manager,
    menu::{MenuBuilder, MenuItemBuilder},
    State,
};

pub struct AppState {
    pub config: Mutex<config::AppConfig>,
    pub status: Mutex<poller::DeviceStatus>,
    pub poller_control: Arc<poller::PollerControl>,
    pub app_dir: PathBuf,
    pub resource_dir: PathBuf,
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Result<poller::DeviceStatus, String> {
    let status = state.status.lock().map_err(|e| e.to_string())?;
    Ok(status.clone())
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<config::AppConfig, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, new_config: config::AppConfig) -> Result<(), String> {
    config::save(&state.app_dir, &new_config)?;
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    *cfg = new_config;
    Ok(())
}

#[tauri::command]
fn list_audio_devices() -> Result<Vec<audio::AudioEndpoint>, String> {
    audio::enumerate_endpoints()
}

#[tauri::command]
fn set_auto_start(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if enabled {
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        autostart::enable(&exe_path)?;
    } else {
        autostart::disable()?;
    }
    // Update config in memory only — save_config handles persistence
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.auto_start = enabled;
    Ok(())
}

#[tauri::command]
fn refresh_now(state: State<'_, AppState>) -> Result<(), String> {
    // Wake the poller thread from its sleep to trigger an immediate poll
    let (lock, cvar) = state.poller_control.wake.as_ref();
    let mut wake_flag = lock.lock().map_err(|e| e.to_string())?;
    *wake_flag = true;
    cvar.notify_one();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_dir = app.path().app_config_dir().expect("failed to resolve app config dir");
            let resource_dir = app.path().resource_dir().expect("failed to resolve resource dir");
            let cfg = config::load(&app_dir);

            // Create poller control (shared between poller thread and commands)
            let poller_control = Arc::new(poller::PollerControl {
                stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                wake: Arc::new((Mutex::new(false), std::sync::Condvar::new())),
                thread_handle: Mutex::new(None),
            });

            // Manage shared state before starting poller (poller reads config from state)
            app.manage(AppState {
                config: Mutex::new(cfg),
                status: Mutex::new(poller::DeviceStatus::default()),
                poller_control: poller_control.clone(),
                app_dir,
                resource_dir,
            });

            // Start poller thread
            let app_handle = app.handle().clone();
            poller::start_poller(app_handle, poller_control);

            // Set up system tray — use the declarative tray icon from tauri.conf.json
            let tray = app.tray_by_id("main-tray").expect("main-tray not found");

            let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let hide_item = MenuItemBuilder::with_id("hide", "Hide Window").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&hide_item)
                .separator()
                .item(&quit_item)
                .build()?;

            tray.set_menu(Some(menu))?;
            tray.set_tooltip(Some("A50 Dock Switch"))?;
            tray.on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        "quit" => {
                            // Signal poller to stop cleanly and wait for it
                            if let Some(state) = app.try_state::<AppState>() {
                                state.poller_control.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                let (_, cvar) = state.poller_control.wake.as_ref();
                                cvar.notify_one();
                                // Wait for the poller thread to finish
                                if let Ok(mut h) = state.poller_control.thread_handle.lock() {
                                    if let Some(handle) = h.take() {
                                        let _ = handle.join();
                                    }
                                }
                            }
                            app.exit(0);
                        }
                        _ => {}
                    }
                });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide to tray instead of closing
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_config,
            save_config,
            list_audio_devices,
            set_auto_start,
            refresh_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
