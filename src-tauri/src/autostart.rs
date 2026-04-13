use windows::Win32::System::Registry::*;
use windows::core::*;
use std::path::Path;

const RUN_KEY_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "A50DockSwitch";

/// Enable autostart by writing to the Run registry key.
pub fn enable(exe_path: &Path) -> std::result::Result<(), String> {
    unsafe {
        let mut key: HKEY = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY_PATH),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .ok()
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

        // Encode as null-terminated UTF-16 for REG_SZ
        let wide: Vec<u16> = exe_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();

        let result = RegSetValueExW(
            key,
            &HSTRING::from(APP_NAME),
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * 2,
            )),
        )
        .ok();

        let _ = RegCloseKey(key);

        result.map_err(|e| format!("Failed to set Run value: {}", e))
    }
}

/// Disable autostart by removing the Run registry key entry.
pub fn disable() -> std::result::Result<(), String> {
    unsafe {
        let mut key: HKEY = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY_PATH),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .ok()
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

        let result = RegDeleteValueW(key, &HSTRING::from(APP_NAME));
        // It's fine if the value doesn't exist (auto-start was never enabled)
        // ERROR_FILE_NOT_FOUND = 2 (win32) / 0x80070002 (HRESULT)
        if let Err(e) = result.ok() {
            if e.code().0 as u32 != 0x80070002 {
                let _ = RegCloseKey(key);
                return Err(format!("Failed to delete Run value: {}", e));
            }
        }

        let _ = RegCloseKey(key);
        Ok(())
    }
}
