use crate::usb;
use crate::audio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatus {
    pub docked: Option<bool>,
    pub is_on: Option<bool>,
    pub charging: Option<bool>,
    pub battery: Option<u8>,
    pub game_device: String,
    pub voice_device: String,
    pub same_device: bool,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            docked: None,
            is_on: None,
            charging: None,
            battery: None,
            game_device: String::new(),
            voice_device: String::new(),
            same_device: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EventLogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: String,
}

/// Shared state between the poller thread and the main app.
pub struct PollerControl {
    pub stop_flag: Arc<AtomicBool>,
    pub wake: Arc<(Mutex<bool>, Condvar)>,
}

fn format_time() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn log_event(app: &AppHandle, message: &str, level: &str) {
    let entry = EventLogEntry {
        timestamp: format_time(),
        message: message.to_string(),
        level: level.to_string(),
    };
    let _ = app.emit("event-log", &entry);
}

/// Run the polling loop on a background thread.
/// Reads config from shared AppState on each cycle so settings changes take effect immediately.
pub fn start_poller(app: AppHandle, control: Arc<PollerControl>) {
    let stop_flag = control.stop_flag.clone();
    let wake = control.wake.clone();
    let prev_docked = Mutex::new(None::<bool>);

    thread::spawn(move || {
        // Initialize COM once for this thread's lifetime
        #[cfg(target_os = "windows")]
        let _ = unsafe { windows::Win32::System::Com::CoInitialize(None) };

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Read current config from shared state
            let config = {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    let cfg = state.config.lock().ok();
                    cfg.map(|c| c.clone())
                } else {
                    None
                }
            };

            let Some(config) = config else {
                // Can't read config, wait and retry
                sleep_or_wake(&stop_flag, &wake, Duration::from_secs(2));
                continue;
            };

            let poll_result = usb::poll_once();

            match poll_result {
                Ok((is_docked, is_on, is_charging, battery, _device_name)) => {
                    // Determine game/voice devices based on dock state
                    let (game_device, voice_device, same_device) = if is_docked {
                        (config.docked_game_device.clone(), config.docked_voice_device.clone(), config.docked_same_device)
                    } else {
                        (config.undocked_game_device.clone(), config.undocked_voice_device.clone(), config.undocked_same_device)
                    };

                    // Build current status
                    let status = DeviceStatus {
                        docked: Some(is_docked),
                        is_on: Some(is_on),
                        charging: Some(is_charging),
                        battery: Some(battery),
                        game_device: game_device.clone(),
                        voice_device: voice_device.clone(),
                        same_device,
                    };

                    // Update shared AppState so get_status returns current data
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        if let Ok(mut s) = state.status.lock() {
                            *s = status.clone();
                        }
                    }

                    // Emit status update event
                    let _ = app.emit("status-update", &status);

                    // Check for transition or first poll
                    let mut prev = prev_docked.lock().unwrap();
                    let is_first_poll = prev.is_none();
                    let is_transition = prev.is_some() && *prev != Some(is_docked);

                    if is_first_poll || is_transition {
                        let label = if is_docked { "DOCKED" } else { "UNDOCKED" };
                        let prefix = if is_first_poll { "Startup" } else { label };

                        // Only switch if devices are configured
                        if !game_device.is_empty() {
                            match audio::switch_audio(&game_device, &voice_device, same_device) {
                                Ok(detail) => {
                                    log_event(&app, &format!(
                                        "{} -> battery={}%, charging={} [switch ok: {}]",
                                        prefix, battery, is_charging, detail
                                    ), "info");
                                }
                                Err(e) => {
                                    log_event(&app, &format!(
                                        "{} -> switch FAILED: {}",
                                        prefix, e
                                    ), "error");
                                }
                            }
                        } else {
                            log_event(&app, &format!(
                                "{} -> battery={}%, charging={} [no device configured]",
                                prefix, battery, is_charging
                            ), "warn");
                        }
                    }

                    *prev = Some(is_docked);
                }
                Err(e) => {
                    // Device not found or USB error
                    let status = DeviceStatus::default();
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        if let Ok(mut s) = state.status.lock() {
                            *s = status.clone();
                        }
                    }
                    let _ = app.emit("status-update", &status);
                    let _ = app.emit("device-error", &e);
                    log_event(&app, &format!("Device not found — {}", e), "warn");
                }
            }

            // Sleep for poll interval, wakeable by refresh_now or stop
            sleep_or_wake(&stop_flag, &wake, Duration::from_secs(config.poll_interval));
        }

        // Uninitialize COM on thread exit
        #[cfg(target_os = "windows")]
        unsafe { windows::Win32::System::Com::CoUninitialize(); }
    });
}

/// Sleep for the given duration, but wake early if stop_flag is set or wake is signaled.
fn sleep_or_wake(
    stop_flag: &Arc<AtomicBool>,
    wake: &Arc<(Mutex<bool>, Condvar)>,
    duration: Duration,
) {
    let (lock, cvar) = wake.as_ref();
    let mut wake_flag = lock.lock().unwrap();
    if !stop_flag.load(Ordering::Relaxed) && !*wake_flag {
        let _ = cvar.wait_timeout(wake_flag, duration);
        // Re-acquire the lock after wait
        wake_flag = lock.lock().unwrap();
    }
    // Reset wake flag
    *wake_flag = false;
}
