use windows::Win32::System::Registry::*;
use windows::core::*;
use std::path::Path;

const RUN_KEY_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "A50DockSwitch";

/// Check if autostart is enabled.
pub fn is_enabled() -> Result<bool, String> {
    unsafe {
        let mut key: HKEY = Default::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY_PATH),
            0,
            KEY_READ,
            &mut key,
        );

        if result.is_err() {
            return Ok(false);
        }

        let result = RegQueryValueExW(
            key,
            &HSTRING::from(APP_NAME),
            None,
            None,
            None,
            None,
        );

        let _ = RegCloseKey(key);
        Ok(result.is_ok())
    }
}

/// Enable autostart by writing to the Run registry key.
pub fn enable(exe_path: &Path) -> Result<(), String> {
    unsafe {
        let mut key: HKEY = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY_PATH),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

        let exe_str = HSTRING::from(exe_path.to_string_lossy().as_ref());

        RegSetValueExW(
            key,
            &HSTRING::from(APP_NAME),
            0,
            REG_SZ,
            Some(&exe_str.as_bytes()),
        )
        .map_err(|e| format!("Failed to set Run value: {}", e))?;

        let _ = RegCloseKey(key);
        Ok(())
    }
}

/// Disable autostart by removing the Run registry key entry.
pub fn disable() -> Result<(), String> {
    unsafe {
        let mut key: HKEY = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY_PATH),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

        RegDeleteValueW(key, &HSTRING::from(APP_NAME))
            .map_err(|e| format!("Failed to delete Run value: {}", e))?;

        let _ = RegCloseKey(key);
        Ok(())
    }
}
