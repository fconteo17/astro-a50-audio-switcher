# A50 Dock Switch

A Windows desktop utility that automatically switches audio devices when docking or undocking an ASTRO A50 Gen 4 headset. Built with Rust + Tauri.

## Features

- **Automatic audio switching** — Detects dock/undock transitions via USB polling and switches audio endpoints instantly
- **Separate game & voice devices** — Configure different devices for Multimedia/Console and Communications roles
- **Dual profiles** — Set different audio devices for docked and undocked states
- **Battery & charge monitoring** — Live battery percentage and charging status from the base station
- **System tray** — Minimizes to tray instead of closing; right-click for show/hide/quit
- **Auto-start** — Optional Windows startup via registry entry
- **Event log** — Timestamped log of dock transitions and switch results

## How It Works

The app polls the ASTRO A50 base station over USB (VID `0x9886`, PID `0x002C`) every few seconds. When it detects a dock state change, it uses [SoundVolumeView](https://www.nirsoft.net/utils/sound_volume_view.html) to set the default audio endpoint for three Windows audio roles:

| Role | Purpose |
|------|---------|
| Console (0) | System sounds |
| Multimedia (1) | Games, music, media |
| Communications (2) | Voice chat, Discord |

## Prerequisites

- Windows 10/11
- ASTRO A50 Gen 4 base station connected via USB
- [SoundVolumeView.exe](https://www.nirsoft.net/utils/sound_volume_view.html) — bundled automatically in release builds; for dev mode, place it in `src-tauri/target/debug/`

## Development

```bash
# Install prerequisites
npm install

# Run in dev mode
cargo tauri dev

# Build release installer
cargo tauri build
```

The built installer will be in `src-tauri/target/release/bundle/`.

## Configuration

All settings are accessible from the app UI:

- **Docked/Undocked profiles** — Select game and voice audio devices for each state
- **Same device toggle** — Use the game device for voice too
- **Poll interval** — How often to check the base station (1–30 seconds)
- **Auto-start** — Launch with Windows

Config is stored in `%APPDATA%\com.a50dockswitch.app\config.json`.

## Tech Stack

- **Rust** — USB polling, audio control, registry management
- **Tauri v2** — Desktop framework with system tray support
- **rusb** — USB device communication
- **SoundVolumeView** — Windows audio endpoint switching
- **Vanilla HTML/CSS/JS** — Lightweight frontend, no framework

## License

[MIT](LICENSE)
