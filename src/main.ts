import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface Settings {
  microphone: string;
  engine: string;
  whisperModel: string;
  groqApiKey: string;
  recordingMode: string;
  hotkey: string;
  language: string;
}

interface MicDevice {
  name: string;
  is_default: boolean;
}

interface DownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
}

// DOM elements
const statusDot = document.getElementById("status-dot")!;
const statusText = document.getElementById("status-text")!;
const micSelect = document.getElementById("mic-select") as HTMLSelectElement;
const engineLocal = document.getElementById("engine-local")!;
const engineCloud = document.getElementById("engine-cloud")!;
const localSettings = document.getElementById("local-settings")!;
const cloudSettings = document.getElementById("cloud-settings")!;
const modelSelect = document.getElementById("model-select") as HTMLSelectElement;
const downloadBtn = document.getElementById("download-btn")!;
const downloadProgress = document.getElementById("download-progress")!;
const progressFill = document.getElementById("progress-fill")!;
const groqKey = document.getElementById("groq-key") as HTMLInputElement;
const languageSelect = document.getElementById("language-select") as HTMLSelectElement;
const modeToggle = document.getElementById("mode-toggle")!;
const modePtt = document.getElementById("mode-ptt")!;
const hotkeyDisplay = document.getElementById("hotkey-display")!;
const hotkeyRecordBtn = document.getElementById("hotkey-record-btn") as HTMLButtonElement;

// Section navigation
const navItems = document.querySelectorAll(".nav-item");
const sections = document.querySelectorAll(".content-section");

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const target = item.getAttribute("data-section");
    navItems.forEach((n) => n.classList.remove("active"));
    sections.forEach((s) => s.classList.remove("active"));
    item.classList.add("active");
    document.getElementById(`section-${target}`)?.classList.add("active");
  });
});

// Window drag — titlebar and sidebar empty space
const titlebar = document.getElementById("titlebar")!;
const sidebar = document.getElementById("sidebar")!;
const appWindow = getCurrentWindow();

titlebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow.startDragging();
});

sidebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow.startDragging();
});

let currentSettings: Settings;

async function loadSettings() {
  currentSettings = await invoke<Settings>("get_settings");

  // Populate mic dropdown
  const mics = await invoke<MicDevice[]>("list_microphones");
  micSelect.innerHTML = "";
  mics.forEach((mic) => {
    const option = document.createElement("option");
    option.value = mic.name;
    option.textContent = mic.name + (mic.is_default ? " (default)" : "");
    micSelect.appendChild(option);
  });
  micSelect.value = currentSettings.microphone;

  // Engine
  setEngine(currentSettings.engine);

  // Model
  modelSelect.value = currentSettings.whisperModel;
  await checkModelStatus();

  // Groq key
  groqKey.value = currentSettings.groqApiKey;

  // Language
  languageSelect.value = currentSettings.language;

  // Recording mode
  setRecordingMode(currentSettings.recordingMode);

  // Hotkey
  hotkeyDisplay.textContent = currentSettings.hotkey.replace("CmdOrCtrl", "Cmd");
}

function formatHotkey(hotkey: string): string {
  return hotkey.replace("CmdOrCtrl", "Cmd").replace(/\+/g, " + ");
}

// Modifier codes recognized while recording; anything else is the "main" key
// that completes the combo. Modifier-only combos aren't supported here since
// tauri-plugin-global-shortcut requires a real key, not just modifiers.
const MODIFIER_CODES: Record<string, string> = {
  ControlLeft: "Control",
  ControlRight: "Control",
  AltLeft: "Alt",
  AltRight: "Alt",
  MetaLeft: "CmdOrCtrl",
  MetaRight: "CmdOrCtrl",
  ShiftLeft: "Shift",
  ShiftRight: "Shift",
};

function startRecordingHotkey() {
  const heldMods = new Set<string>();
  let finished = false;

  hotkeyRecordBtn.textContent = "Hold modifiers + press a key to finish (Esc to cancel)";
  hotkeyRecordBtn.disabled = true;
  const previousHotkey = currentSettings.hotkey;
  hotkeyDisplay.textContent = "...";

  const finish = (hotkey: string | null) => {
    if (finished) return;
    finished = true;
    window.removeEventListener("keydown", onKeyDown, true);
    hotkeyRecordBtn.textContent = "Record Shortcut";
    hotkeyRecordBtn.disabled = false;
    if (hotkey) {
      currentSettings.hotkey = hotkey;
      hotkeyDisplay.textContent = formatHotkey(hotkey);
      saveSettings();
    } else {
      hotkeyDisplay.textContent = formatHotkey(previousHotkey);
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    e.preventDefault();
    const modName = MODIFIER_CODES[e.code];
    if (modName) {
      heldMods.add(modName);
      hotkeyDisplay.textContent = [...heldMods].join(" + ");
    } else if (e.code === "Escape") {
      finish(null); // cancel, restore previous hotkey
    } else {
      // A real key completes the combo, with or without modifiers held.
      finish([...heldMods, e.code].join("+"));
    }
  };

  window.addEventListener("keydown", onKeyDown, true);
}

function setEngine(engine: string) {
  currentSettings.engine = engine;
  engineLocal.classList.toggle("active", engine === "local");
  engineCloud.classList.toggle("active", engine === "cloud");
  localSettings.classList.toggle("hidden", engine !== "local");
  cloudSettings.classList.toggle("hidden", engine !== "cloud");
}

function setRecordingMode(mode: string) {
  currentSettings.recordingMode = mode;
  modeToggle.classList.toggle("active", mode === "toggle");
  modePtt.classList.toggle("active", mode === "push-to-talk");
}

async function checkModelStatus() {
  const downloaded = await invoke<boolean>("check_model_downloaded", {
    modelSize: modelSelect.value,
  });
  downloadBtn.textContent = downloaded ? "\u2713" : "Download";
  (downloadBtn as HTMLButtonElement).disabled = downloaded;
}

async function saveSettings() {
  // Only overwrite if the dropdown actually has a selection — an empty
  // value (e.g. saved before mics finished populating) would otherwise
  // silently break recording, since no device is named "".
  if (micSelect.value) currentSettings.microphone = micSelect.value;
  currentSettings.whisperModel = modelSelect.value;
  currentSettings.groqApiKey = groqKey.value;
  currentSettings.language = languageSelect.value;
  try {
    await invoke("save_settings", { settings: currentSettings });
  } catch (e) {
    console.error("save_settings failed:", e);
    alert("Failed to save settings: " + e);
  }
}

// Event listeners
engineLocal.addEventListener("click", () => {
  setEngine("local");
  saveSettings();
});

engineCloud.addEventListener("click", () => {
  setEngine("cloud");
  saveSettings();
});

micSelect.addEventListener("change", () => saveSettings());

modelSelect.addEventListener("change", async () => {
  await checkModelStatus();
  saveSettings();
});

downloadBtn.addEventListener("click", async () => {
  (downloadBtn as HTMLButtonElement).disabled = true;
  downloadProgress.classList.remove("hidden");
  progressFill.style.width = "0%";

  try {
    await invoke("download_model", { modelSize: modelSelect.value });
    downloadBtn.textContent = "\u2713";
  } catch (e) {
    downloadBtn.textContent = "Retry";
    (downloadBtn as HTMLButtonElement).disabled = false;
    console.error("Download failed:", e);
  }
  downloadProgress.classList.add("hidden");
});

groqKey.addEventListener("change", () => saveSettings());

languageSelect.addEventListener("change", () => saveSettings());

modeToggle.addEventListener("click", () => {
  setRecordingMode("toggle");
  saveSettings();
});

modePtt.addEventListener("click", () => {
  setRecordingMode("push-to-talk");
  saveSettings();
});

hotkeyRecordBtn.addEventListener("click", () => startRecordingHotkey());

// Listen for recording state changes
listen<string>("recording-state", (event) => {
  const state = event.payload;
  statusDot.className = "";
  if (state === "Recording") {
    statusDot.classList.add("recording");
    statusText.textContent = "Recording...";
  } else if (state === "Transcribing") {
    statusDot.classList.add("transcribing");
    statusText.textContent = "Transcribing...";
  } else {
    statusDot.classList.add("ready");
    statusText.textContent = "Ready";
  }
});

// Listen for download progress
listen<DownloadProgress>("download-progress", (event) => {
  const { percent } = event.payload;
  progressFill.style.width = `${percent}%`;
});

// Initialize
loadSettings();
