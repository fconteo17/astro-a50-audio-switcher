const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const MAX_EVENTS = 50;
let events = [];
let audioDevices = [];

// ── DOM References ──────────────────────────────────────────────

const dockState     = document.getElementById('dock-state');
const dockDot       = document.getElementById('dock-dot');
const dockLabel     = document.getElementById('dock-label');
const dockSublabel  = document.getElementById('dock-sublabel');
const batteryFillRow = document.getElementById('battery-fill-row');
const batteryPercent = document.getElementById('battery-percent');
const chargingValue = document.getElementById('charging-value');
const gameAudioValue = document.getElementById('game-audio-value');
const voiceAudioValue = document.getElementById('voice-audio-value');
const pollValue      = document.getElementById('poll-value');
const eventLog      = document.getElementById('event-log');

// Settings
const settingsToggle  = document.getElementById('settings-toggle');
const toggleIcon      = document.getElementById('toggle-icon');
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
    toggleIcon.classList.toggle('open');
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
    select.innerHTML = '<option value="">-- Select --</option>';
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
        setSelectedOption(dockedGameSelect, config.docked_game_device);
        setSelectedOption(dockedVoiceSelect, config.docked_voice_device);
        setSelectedOption(undockedGameSelect, config.undocked_game_device);
        setSelectedOption(undockedVoiceSelect, config.undocked_voice_device);
        dockedSameCheckbox.checked = config.docked_same_device;
        undockedSameCheckbox.checked = config.undocked_same_device;
        dockedVoiceLabel.classList.toggle('hidden', config.docked_same_device);
        undockedVoiceLabel.classList.toggle('hidden', config.undocked_same_device);
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
        await invoke('save_config', { newConfig: config });
        if (config.auto_start) {
            await invoke('set_auto_start', { enabled: true });
        } else {
            await invoke('set_auto_start', { enabled: false });
        }
        saveBtn.classList.add('saved');
        saveBtn.querySelector('.save-btn-text').textContent = 'SAVED';
        setTimeout(() => {
            saveBtn.classList.remove('saved');
            saveBtn.querySelector('.save-btn-text').textContent = 'SAVE CONFIG';
        }, 1500);
    } catch (e) {
        console.error('Failed to save config:', e);
        saveBtn.classList.add('error');
        saveBtn.querySelector('.save-btn-text').textContent = 'ERROR';
        setTimeout(() => {
            saveBtn.classList.remove('error');
            saveBtn.querySelector('.save-btn-text').textContent = 'SAVE CONFIG';
        }, 1500);
    }
});

// ── Event listeners from Rust backend ──────────────────────────

listen('status-update', (event) => {
    updateStatus(event.payload);
});

listen('event-log', (event) => {
    addEvent(event.payload);
});

listen('device-error', () => {
    setDockState('off', 'NOT FOUND', 'Device unavailable');
});

// ── Update status dashboard ────────────────────────────────────

function setDockState(state, label, sublabel) {
    dockState.className = 'dock-state state-' + state;
    dockLabel.textContent = label;
    dockSublabel.textContent = sublabel || '';
}

function updateStatus(s) {
    // Dock state
    if (s.docked === null || s.docked === undefined) {
        setDockState('connecting', 'CONNECTING', 'Awaiting device');
    } else if (s.docked) {
        setDockState('docked', 'DOCKED', 'Base station connected');
    } else if (s.is_on) {
        setDockState('undocked', 'UNDOCKED', 'Wireless mode');
    } else {
        setDockState('off', 'OFF', 'Headset powered off');
    }

    // Battery
    if (s.battery !== null && s.battery !== undefined) {
        batteryFillRow.style.width = s.battery + '%';
        batteryPercent.textContent = s.battery + '%';

        batteryFillRow.classList.remove('battery-low', 'battery-mid', 'battery-ok');
        if (s.battery > 60) batteryFillRow.classList.add('battery-ok');
        else if (s.battery > 20) batteryFillRow.classList.add('battery-mid');
        else batteryFillRow.classList.add('battery-low');
    } else {
        batteryFillRow.style.width = '0%';
        batteryPercent.textContent = 'N/A';
        batteryFillRow.classList.remove('battery-low', 'battery-mid', 'battery-ok');
    }

    // Charging
    if (s.charging) {
        chargingValue.textContent = 'ACTIVE';
        chargingValue.classList.add('charging-on');
    } else {
        chargingValue.textContent = 'OFF';
        chargingValue.classList.remove('charging-on');
    }

    // Audio devices
    gameAudioValue.textContent = s.game_device || '--';
    voiceAudioValue.textContent = s.same_device ? '(same)' : (s.voice_device || '--');

    // Poll — show actual running interval from config, not the input field
    const pollSecs = s.poll_interval || parseInt(pollIntervalInput.value, 10) || 2;
    pollValue.textContent = pollSecs + 's';
}

// ── Event log ──────────────────────────────────────────────────

function addEvent(entry) {
    events.push(entry);
    if (events.length > MAX_EVENTS) events.shift();

    const div = document.createElement('div');
    div.className = 'log-' + entry.level;
    div.textContent = entry.timestamp + '  ' + entry.message;
    eventLog.appendChild(div);

    while (eventLog.children.length > MAX_EVENTS) {
        eventLog.removeChild(eventLog.firstChild);
    }

    eventLog.scrollTop = eventLog.scrollHeight;
}

// ── Initialize ──────────────────────────────────────────────────

async function init() {
    await loadAudioDevices();
    await loadConfig();
}

init();
