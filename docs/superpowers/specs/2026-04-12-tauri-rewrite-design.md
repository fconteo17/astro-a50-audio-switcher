# A50 Dock Switch — Tauri Rewrite Design

## Overview

Rewrite the Python-based `a50-dock-switch` utility as a native Windows desktop app using Rust + Tauri. The app polls an ASTRO A50 Gen 4 base station over USB, detects dock/undock transitions, and automatically switches the Windows default audio device. Features a dark-mode GUI with live status, event log, and system tray support.

## Architecture

Monolithic Rust backend with thin vanilla HTML/CSS/JS frontend.

### Rust Backend

All core logic runs in Rust on a background thread:

- **USB Poller** — communicates with A50 base station via `rusb` (libusb bindings), same protocol as the Python version
- **Audio Switcher** — uses `windows-rs` COM interop to call `IPolicyConfig::SetDefaultEndpoint` and enumerate devices via `Win32_PnPEntity`
- **Config Manager** — reads/writes settings via Tauri's `fs` plugin (JSON file in app data directory)
- **Auto-Start Manager** — writes/removes registry key at `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`

### Tauri Bridge

Commands (frontend → Rust):

| Command | Description |
|---|---|
| `get_status` | Returns current dock/battery/charging state |
| `get_config` | Returns current config values |
| `save_config` | Persists config changes |
| `list_audio_devices` | Returns list of available audio endpoint names (for dropdowns) |
| `set_auto_start` | Enables/disables Windows auto-start |
| `refresh_now` | Triggers an immediate USB poll |

Events (Rust → frontend):

| Event | Payload |
|---|---|
| `status-update` | `{ docked, is_on, battery, charging, game_device, voice_device, same_device }` |
| `event-log` | `{ timestamp, message, level }` |
| `device-error` | `{ message }` |

### Frontend

Vanilla HTML/CSS/JS with dark theme. No framework, no npm dependencies.

## UI Layout

Dark mode, minimal, compact ~400x500px window.

**Top — Status Dashboard:**
- Grid of labeled values: Status (DOCKED/UNDOCKED/OFF), Battery (bar + %), Charging (Yes/No), Game Audio Device, Voice Audio Device (hidden when same_device is true for current dock state), Poll Interval
- Color-coded: docked=green, undocked=cyan, off=dim gray, connecting=yellow
- Battery bar: 10-block bar, green (>60%), yellow (20-60%), red (<20%)

**Middle — Event Log:**
- Fixed-height scrollable list, last 50 entries
- Timestamped entries: dock transitions, audio switch results, errors
- Auto-scrolls to bottom

**Bottom — Settings (collapsible):**
- Docked section:
  - Game audio device (dropdown of audio endpoints)
  - Checkbox: "Use same device for voice" (default: checked)
  - Voice/communications device (dropdown, hidden when checkbox is checked — same device used for both)
- Undocked section:
  - Game audio device (dropdown of audio endpoints)
  - Checkbox: "Use same device for voice" (default: unchecked)
  - Voice/communications device (dropdown, shown by default — only hidden when checkbox is checked)
- Poll interval in seconds (number input)
- Auto-start with Windows (checkbox/toggle)
- Save button

The ASTRO A50 has separate voice and game sound channels, so each dock state can have two device assignments — one for multimedia (game sound) and one for communications (voice chat). A "use same device" toggle per dock state simplifies the common case where both go to the same endpoint.

**System Tray:**
- Headset icon
- Right-click menu: "Show Window", "Hide Window", separator, "Quit"
- Closing window hides to tray (does not quit)
- Title: "A50 Dock Switch"

## USB Protocol

Same protocol as the Python version:

| Constant | Value | Meaning |
|---|---|---|
| VID | `0x9886` | Astro vendor ID |
| PID | `0x002C` | A50 product ID |
| INTERFACE | `6` | USB interface for control commands |
| EP_OUT | `0x05` | Outbound endpoint |
| EP_IN | `0x85` | Inbound endpoint |
| TIMEOUT_MS | `3000` | 3-second USB timeout |

Request frame: `[0x02, command_type, payload_length, ...payload]`
Response frame: `[0x02, status, length, ...data]`

Commands:
- `0x54` (GET_HEADSET_STATUS) — response byte: bit 0 = `is_docked`, bit 1 = `is_on`
- `0x7C` (GET_BATTERY_STATUS) — response byte: bit 7 = `is_charging`, bits 0-6 = `charge_percent`

Open/query/release per cycle to avoid disrupting the base station's charging management. Detach/re-attach kernel driver as needed.

## Audio Switching

Native COM interop via `windows-rs`:

1. Enumerate `Win32_PnPEntity` where `PNPClass = 'AudioEndpoint'` to find the target device ID by name
2. When a dock state transition occurs, set two devices:
   - **Game (multimedia) device** → `IPolicyConfig::SetDefaultEndpoint` with role 0 (eMultimedia) and role 2 (eConsole)
   - **Voice (communications) device** → `IPolicyConfig::SetDefaultEndpoint` with role 1 (eCommunications)
3. This separation is essential for the ASTRO A50 which has distinct voice and game sound volume channels
4. COM interface GUID: `870AF99C-171D-4F9E-AF0D-E63DF40C3BC9`

No external tools required (SoundSwitch, NirCmd, PowerShell subprocesses not needed).

## Data Flow

**Polling cycle:**
1. USB poller thread wakes every N seconds (configurable, default 2)
2. Opens device via `rusb`, claims interface 6, sends commands `0x54` and `0x7C`, reads responses, releases interface
3. Emits `status-update` event to frontend with current state
4. If dock state changed since last poll, calls audio switcher — sets game device for multimedia+console roles and voice device for communications role
5. Emits `event-log` entry with switch result

**Startup:**
1. Load config (or use defaults)
2. Start USB poller thread
3. First successful poll → immediately apply matching audio profile
4. Frontend shows "CONNECTING..." until first poll succeeds

**Shutdown:**
1. Stop event signaled → poller thread exits cleanly
2. Release USB device if held
3. App exits

## Error Handling

- Device not found → emit `device-error`, show "Not Found" in status, log warning
- USB timeout → retry next cycle, log error
- Audio switch fails → log error, don't crash polling
- Config read/write fails → log error, use defaults

## Configuration

Stored as JSON via Tauri's `fs` plugin in app data directory.

Default values:
```json
{
  "docked_game_device": "",
  "docked_voice_device": "",
  "docked_same_device": true,
  "undocked_game_device": "",
  "undocked_voice_device": "",
  "undocked_same_device": false,
  "poll_interval": 2,
  "auto_start": false
}
```

When `docked_same_device` is true, `docked_voice_device` is ignored and the game device is used for all roles. Same for `undocked_same_device`. Device names must match Windows audio endpoint names exactly (e.g. "Speakers (Realtek HD Audio)"). The settings UI provides dropdowns populated with available audio endpoint names to avoid typos.

## Auto-Start

Writes/removes registry key at `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` with the app's executable path. Toggled via settings UI.

## Project Structure

```
a50-dock-switch-tauri/
  src-tauri/
    src/
      main.rs            # Entry point, Tauri app setup, system tray
      usb.rs             # USB protocol: open/query/release device
      audio.rs           # COM interop: enumerate devices, set default endpoint
      config.rs          # Config model, load/save via Tauri fs plugin
      autostart.rs       # Registry key management for Windows startup
      poller.rs          # Background thread: poll loop, state tracking, event emission
    Cargo.toml
    tauri.conf.json
    icons/
  src/
    index.html           # Main window markup
    style.css            # Dark theme styles
    app.js               # Frontend logic: event listeners, DOM updates, settings form
  package.json           # Minimal, just for Tauri CLI dev tooling
```

## Key Dependencies

| Crate | Purpose |
|---|---|
| `tauri` | App framework, system tray, events, commands |
| `rusb` | libusb bindings for USB communication |
| `windows` | COM interop for audio policy config |
| `serde` / `serde_json` | Config serialization |
| `tauri-plugin-fs` | Config file read/write in app data dir |

No npm frontend dependencies — pure vanilla HTML/CSS/JS.

## Prerequisites (End User)

1. WinUSB driver for A50 interface 6 (installed via Zadig, same as Python version)
2. `libusb-1.0.dll` must be alongside the executable (bundled or placed manually)
