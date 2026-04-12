const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const MAX_EVENTS = 50;
let events = [];
let audioDevices = [];

// ── DOM References ──────────────────────────────────────────────

const statusValue = document.getElementById('status-value');
const batteryFill = document.getElementById('battery-fill');
const batteryPercent = document.getElementById('battery-percent');
const batteryContainer = document.getElementById('battery-value');
const chargingValue = document.getElementById('charging-value');
const gameAudioValue = document.getElementById('game-audio-value');
const voiceAudioValue = document.getElementById('voice-audio-value');
const pollValue = document.getElementById('poll-value');
const eventLog = document.getElementById('event-log');

// Settings
const settingsToggle = document.getElementById('settings-toggle');
const settingsContent = document.getElementById('settings-content');
const dockedGameSelect = document.getElementById('docked-game-device');
const dockedVoiceSelect = document.getElementById('docked-voice-device');
const dockedSameCheckbox = document.getElementById('docked-same-device');
const dockedVoiceLabel = document.getElementById('docked-voice-label');
const undockedGameSelect = document.getElementById('undocked-game-device');
const undockedVoiceSelect = document.getElementById('undocked-voice-device');
const undockedSameCheckbox = document.getElementById('undocked-same-device');
const undockedVoiceLabel = document.getElementById('undocked-voice-label');
const pollIntervalInput = document.getElementById('poll-interval');
const autoStartCheckbox = document.getElementById('auto-start');
const saveBtn = document.getElementById('save-btn');

// ── Settings Toggle ─────────────────────────────────────────────

settingsToggle.addEventListener('click', () => {
    settingsContent.classList.toggle('hidden');
    settingsToggle.textContent = settingsContent.classList.contains('hidden')
        ? 'Settings \u25B6'
        : 'Settings \u25BC';
});

// ── Same-device checkboxes ──────────────────────────────────────

dockedSameCheckbox.addEventListener('change', () => {
    dockedVoiceLabel.classList.toggle('hidden', dockedSameCheckbox.checked);
});

undockedSameCheckbox.addEventListener('change', () => {
    undockedVoiceLabel.classList.toggle('hidden', undockedSameCheckbox.checked);
});

// ── Populate device dropdowns ────────────────────────────────────

async function loadAudioDevices() {
    try {
        audioDevices = await invoke('list_audio_devices');
        populateSelect(dockedGameSelect, audioDevices);
        populateSelect(dockedVoiceSelect, audioDevices);
        populateSelect(undockedGameSelect, audioDevices);
        populateSelect(undockedVoiceSelect, audioDevices);
    } catch (e) {
        console.error('Failed to load audio devices:', e);
    }
}

function populateSelect(select, devices) {
    select.innerHTML = '<option value="">-- Select device --</option>';
    for (const d of devices) {
        const opt = document.createElement('option');
        opt.value = d.name;
        opt.textContent = d.name;
        select.appendChild(opt);
    }
}

// ── Load config into settings form ──────────────────────────────

async function loadConfig() {
    try {
        const config = await invoke('get_config');

        // Set select values
        setSelectedOption(dockedGameSelect, config.docked_game_device);
        setSelectedOption(dockedVoiceSelect, config.docked_voice_device);
        setSelectedOption(undockedGameSelect, config.undocked_game_device);
        setSelectedOption(undockedVoiceSelect, config.undocked_voice_device);

        // Set checkboxes
        dockedSameCheckbox.checked = config.docked_same_device;
        undockedSameCheckbox.checked = config.undocked_same_device;
        dockedVoiceLabel.classList.toggle('hidden', config.docked_same_device);
        undockedVoiceLabel.classList.toggle('hidden', config.undocked_same_device);

        // Set other values
        pollIntervalInput.value = config.poll_interval;
        autoStartCheckbox.checked = config.auto_start;
    } catch (e) {
        console.error('Failed to load config:', e);
    }
}

function setSelectedOption(select, value) {
    select.value = value;
}

// ── Save config ─────────────────────────────────────────────────

saveBtn.addEventListener('click', async () => {
    const config = {
        docked_game_device: dockedGameSelect.value,
        docked_voice_device: dockedVoiceSelect.value,
        docked_same_device: dockedSameCheckbox.checked,
        undocked_game_device: undockedGameSelect.value,
        undocked_voice_device: undockedVoiceSelect.value,
        undocked_same_device: undockedSameCheckbox.checked,
        poll_interval: parseInt(pollIntervalInput.value, 10) || 2,
        auto_start: autoStartCheckbox.checked,
    };

    try {
        await invoke('save_config', { new_config: config });
        if (config.auto_start) {
            await invoke('set_auto_start', { enabled: true });
        } else {
            await invoke('set_auto_start', { enabled: false });
        }
        saveBtn.textContent = 'Saved!';
        setTimeout(() => { saveBtn.textContent = 'Save'; }, 1500);
    } catch (e) {
        console.error('Failed to save config:', e);
        saveBtn.textContent = 'Error';
        setTimeout(() => { saveBtn.textContent = 'Save'; }, 1500);
    }
});

// ── Event listeners from Rust backend ──────────────────────────

listen('status-update', (event) => {
    const s = event.payload;
    updateStatus(s);
});

listen('event-log', (event) => {
    const entry = event.payload;
    addEvent(entry);
});

listen('device-error', (event) => {
    statusValue.textContent = 'NOT FOUND';
    statusValue.className = 'value status-error';
});

// ── Update status dashboard ────────────────────────────────────

function updateStatus(s) {
    // Dock status
    if (s.docked === null || s.docked === undefined) {
        statusValue.textContent = 'CONNECTING...';
        statusValue.className = 'value status-connecting';
    } else if (s.docked) {
        statusValue.textContent = '\u25CF DOCKED';
        statusValue.className = 'value status-docked';
    } else if (s.is_on) {
        statusValue.textContent = '\u25CF UNDOCKED';
        statusValue.className = 'value status-undocked';
    } else {
        statusValue.textContent = '\u25CB OFF';
        statusValue.className = 'value status-off';
    }

    // Battery
    if (s.battery !== null && s.battery !== undefined) {
        batteryFill.style.width = s.battery + '%';
        batteryPercent.textContent = s.battery + '%';

        batteryContainer.className = 'value';
        if (s.battery > 60) batteryContainer.classList.add('battery-ok');
        else if (s.battery > 20) batteryContainer.classList.add('battery-mid');
        else batteryContainer.classList.add('battery-low');
    } else {
        batteryFill.style.width = '0%';
        batteryPercent.textContent = 'N/A';
        batteryContainer.className = 'value';
    }

    // Charging
    chargingValue.textContent = s.charging ? '\u26A1 Yes' : 'No';
    chargingValue.style.color = s.charging ? '#ff9800' : '#888';

    // Audio devices
    gameAudioValue.textContent = s.game_device || '--';
    voiceAudioValue.textContent = s.same_device ? '(same as game)' : (s.voice_device || '--');

    // Poll interval
    pollValue.textContent = pollIntervalInput.value + 's';
}

// ── Event log ──────────────────────────────────────────────────

function addEvent(entry) {
    events.push(entry);
    if (events.length > MAX_EVENTS) events.shift();

    const div = document.createElement('div');
    div.className = 'log-' + entry.level;
    div.textContent = entry.timestamp + '  ' + entry.message;
    eventLog.appendChild(div);

    // Trim old entries from DOM
    while (eventLog.children.length > MAX_EVENTS) {
        eventLog.removeChild(eventLog.firstChild);
    }

    // Auto-scroll
    eventLog.scrollTop = eventLog.scrollHeight;
}

// ── Initialize ──────────────────────────────────────────────────

async function init() {
    await loadAudioDevices();
    await loadConfig();
}

init();
