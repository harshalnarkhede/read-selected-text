const { invoke } = window.__TAURI__.core;
const opener = window.__TAURI__.opener;

const $ = (id) => document.getElementById(id);

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
let currentProvider = "openai";

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------
async function boot() {
  try {
    const s = await invoke("get_settings");
    applySettings(s);
    await refreshKeyStatus();
    await loadLocalVoices(s.local_voice);
    updateStatus();
  } catch (e) {
    setStatus("Failed to load settings: " + e, true);
  }
}

function applySettings(s) {
  currentProvider = s.provider;
  document.querySelector(`input[name="provider"][value="${s.provider}"]`).checked = true;
  showPanel(s.provider);

  $("openai-model").value = s.openai_model;
  $("openai-voice").value = s.openai_voice;
  $("elevenlabs-model").value = s.elevenlabs_model;
  $("elevenlabs-voice").value = s.elevenlabs_voice_id;
  $("local-rate").value = s.local_rate;
  $("rate-out").textContent = s.local_rate;

  $("speed").value = s.speed;
  $("speed-out").textContent = Number(s.speed).toFixed(2) + "×";
  $("restore-clipboard").checked = s.restore_clipboard;
  $("launch-startup").checked = s.launch_at_startup;

  $("read-hotkey").value = s.read_hotkey;
  $("stop-hotkey").value = s.stop_hotkey;
  $("hk-hint").textContent = s.read_hotkey;
}

function gatherSettings() {
  return {
    provider: currentProvider,
    read_hotkey: $("read-hotkey").value,
    stop_hotkey: $("stop-hotkey").value,
    restore_clipboard: $("restore-clipboard").checked,
    speed: parseFloat($("speed").value),
    launch_at_startup: $("launch-startup").checked,
    openai_model: $("openai-model").value,
    openai_voice: $("openai-voice").value,
    elevenlabs_model: $("elevenlabs-model").value,
    elevenlabs_voice_id: $("elevenlabs-voice").value.trim(),
    local_voice: $("local-voice").value,
    local_rate: parseInt($("local-rate").value, 10),
  };
}

// ---------------------------------------------------------------------------
// Provider panels
// ---------------------------------------------------------------------------
function showPanel(provider) {
  document.querySelectorAll(".provider-panel").forEach((p) => {
    p.hidden = p.dataset.provider !== provider;
  });
  // Speed only meaningful for cloud providers.
  $("speed-card").hidden = false;
}

document.querySelectorAll('input[name="provider"]').forEach((r) => {
  r.addEventListener("change", (e) => {
    currentProvider = e.target.value;
    showPanel(currentProvider);
    updateStatus();
  });
});

// ---------------------------------------------------------------------------
// API keys
// ---------------------------------------------------------------------------
async function refreshKeyStatus() {
  const status = await invoke("get_provider_status");
  setKeyStatus("openai", status.has_openai);
  setKeyStatus("elevenlabs", status.has_elevenlabs);
}

function setKeyStatus(provider, present) {
  const el = $(`${provider}-key-status`);
  if (!el) return;
  el.textContent = present ? "✓ Key saved securely" : "No key saved yet";
  el.className = "key-status " + (present ? "ok" : "");
}

document.querySelectorAll("[data-savekey]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const provider = btn.dataset.savekey;
    const input = $(`${provider}-key`);
    const key = input.value.trim();
    try {
      await invoke("set_api_key", { provider, key });
      input.value = "";
      await refreshKeyStatus();
      updateStatus();
    } catch (e) {
      const el = $(`${provider}-key-status`);
      el.textContent = "Error: " + e;
      el.className = "key-status err";
    }
  });
});

// ---------------------------------------------------------------------------
// Local voices
// ---------------------------------------------------------------------------
async function loadLocalVoices(selected) {
  try {
    const voices = await invoke("list_local_voices");
    const sel = $("local-voice");
    for (const v of voices) {
      const opt = document.createElement("option");
      opt.value = v;
      opt.textContent = v;
      sel.appendChild(opt);
    }
    if (selected) sel.value = selected;
  } catch (_) {
    /* best effort */
  }
}

// ---------------------------------------------------------------------------
// Status line
// ---------------------------------------------------------------------------
function setStatus(text, isError) {
  const el = $("status");
  el.textContent = text;
  el.style.opacity = isError ? "1" : "0.9";
}

async function updateStatus() {
  const names = { openai: "OpenAI", elevenlabs: "ElevenLabs", local: "System voice" };
  let ready = true;
  if (currentProvider !== "local") {
    ready = await invoke("has_api_key", { provider: currentProvider });
  }
  setStatus(
    ready
      ? `Ready · ${names[currentProvider]}`
      : `${names[currentProvider]} selected — add an API key to start`
  );
}

// ---------------------------------------------------------------------------
// Sliders
// ---------------------------------------------------------------------------
$("speed").addEventListener("input", (e) => {
  $("speed-out").textContent = Number(e.target.value).toFixed(2) + "×";
});
$("local-rate").addEventListener("input", (e) => {
  $("rate-out").textContent = e.target.value;
});

// ---------------------------------------------------------------------------
// Hotkey recorder
// ---------------------------------------------------------------------------
function keyToAccelerator(e) {
  const mods = [];
  if (e.ctrlKey || e.metaKey) mods.push("CmdOrControl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");

  let key = e.key;
  const map = {
    " ": "Space",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Escape: "Esc",
  };
  if (map[key]) key = map[key];
  else if (key.length === 1) key = key.toUpperCase();

  // Ignore pure modifier presses.
  if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return null;
  if (mods.length === 0) return null; // require at least one modifier
  return [...mods, key].join("+");
}

function setupHotkeyInput(id) {
  const input = $(id);
  input.addEventListener("focus", () => input.classList.add("recording"));
  input.addEventListener("blur", () => input.classList.remove("recording"));
  input.addEventListener("keydown", (e) => {
    e.preventDefault();
    const acc = keyToAccelerator(e);
    if (acc) {
      input.value = acc;
      if (id === "read-hotkey") $("hk-hint").textContent = acc;
      input.blur();
    }
  });
}
setupHotkeyInput("read-hotkey");
setupHotkeyInput("stop-hotkey");

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
$("test-btn").addEventListener("click", async () => {
  const s = gatherSettings();
  try {
    await invoke("test_voice", { settings: s });
  } catch (e) {
    setSave("Test failed: " + e, true);
  }
});

$("stop-btn").addEventListener("click", () => invoke("stop_playback"));

function setSave(text, isError) {
  const el = $("save-status");
  el.textContent = text;
  el.className = "save-status " + (isError ? "err" : text ? "ok" : "");
  if (text && !isError) setTimeout(() => (el.textContent = ""), 2500);
}

$("save-btn").addEventListener("click", async () => {
  try {
    await invoke("save_settings", { new: gatherSettings() });
    setSave("Settings saved ✓", false);
    updateStatus();
  } catch (e) {
    setSave("Could not save: " + e, true);
  }
});

// External links via the OS browser.
document.querySelectorAll("[data-open]").forEach((a) => {
  a.addEventListener("click", (e) => {
    e.preventDefault();
    opener.openUrl(a.dataset.open);
  });
});

boot();
