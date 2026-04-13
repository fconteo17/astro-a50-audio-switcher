use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// An audio endpoint discovered on the system.
#[derive(Debug, Clone, Serialize)]
pub struct AudioEndpoint {
    pub id: String,
    pub name: String,
}

/// Enumerate all active render (playback) audio endpoints via PowerShell.
pub fn enumerate_endpoints() -> Result<Vec<AudioEndpoint>, String> {
    let script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'AudioEndpoint' -and $_.Status -eq 'OK' } | ForEach-Object {
    $id = $_.PNPDeviceID -replace '&amp;','&' -replace '^','{0.0.0.00000000}.'
    "$id|$($_.Name)"
}
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut endpoints = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((id, name)) = line.split_once('|') {
            endpoints.push(AudioEndpoint {
                id: id.to_string(),
                name: name.to_string(),
            });
        }
    }

    Ok(endpoints)
}

/// Find SoundVolumeView.exe — check multiple locations.
fn find_svv(resource_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
    let mut tried = Vec::new();

    // Production: look in the resource directory (root and resources/ subdirectory)
    if let Some(dir) = resource_dir {
        let candidate = dir.join("SoundVolumeView.exe");
        tried.push(candidate.display().to_string());
        if candidate.exists() {
            return Ok(candidate);
        }

        let candidate = dir.join("resources").join("SoundVolumeView.exe");
        tried.push(candidate.display().to_string());
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // Dev mode: look next to the running executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("SoundVolumeView.exe");
            tried.push(candidate.display().to_string());
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "SoundVolumeView.exe not found. Tried: {}",
        tried.join(", ")
    ))
}

/// Set the default audio endpoint for a specific role using SoundVolumeView.
/// SoundVolumeView role numbers: 0=Console, 1=Multimedia, 2=Communications
fn set_default_endpoint(device_name: &str, role: &str, resource_dir: Option<&PathBuf>) -> Result<(), String> {
    let svv = find_svv(resource_dir)?;

    // SoundVolumeView uses just the short name (e.g. "VG245"), not the full
    // CimInstance name with parenthetical (e.g. "VG245 (NVIDIA High Definition Audio)")
    let short_name = device_name
        .find(" (")
        .map_or(device_name, |pos| &device_name[..pos]);

    let output = Command::new(&svv)
        .args(["/SetDefault", short_name, role])
        .output()
        .map_err(|e| format!("Failed to run SoundVolumeView: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!(
            "SVV /SetDefault '{}' role={} failed (exit={}): stdout={} stderr={}",
            device_name, role, output.status, stdout.trim(), stderr.trim()
        ));
    }

    // SoundVolumeView returns exit 0 even if device not found — check stdout for clues
    if stdout.to_lowercase().contains("error") || stdout.to_lowercase().contains("not found") {
        return Err(format!(
            "SVV /SetDefault '{}' role={} may have failed: stdout={}",
            device_name, role, stdout.trim()
        ));
    }

    Ok(())
}

/// Switch audio devices for a dock state transition.
/// game_device_name and voice_device_name are friendly names from enumerate_endpoints.
/// If same_device is true, voice_device_name is ignored and game device is used for all roles.
pub fn switch_audio(
    game_device_name: &str,
    voice_device_name: &str,
    same_device: bool,
    resource_dir: Option<&PathBuf>,
) -> Result<String, String> {
    let endpoints = enumerate_endpoints()?;

    let game_endpoint = endpoints
        .iter()
        .find(|e| e.name == game_device_name)
        .ok_or_else(|| format!("Game device not found: '{}'", game_device_name))?;

    // Set game device for Multimedia (1) and Console (0) — use device name, not ID
    set_default_endpoint(&game_endpoint.name, "1", resource_dir)?;
    set_default_endpoint(&game_endpoint.name, "0", resource_dir)?;

    if same_device {
        // Same device for Communications (2)
        set_default_endpoint(&game_endpoint.name, "2", resource_dir)?;
    } else {
        let voice_endpoint = endpoints
            .iter()
            .find(|e| e.name == voice_device_name)
            .ok_or_else(|| format!("Voice device not found: '{}'", voice_device_name))?;
        set_default_endpoint(&voice_endpoint.name, "2", resource_dir)?;
    }

    let voice_name = if same_device {
        game_device_name
    } else {
        voice_device_name
    };
    Ok(format!("Game: {}, Voice: {}", game_device_name, voice_name))
}
