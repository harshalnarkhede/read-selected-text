const { invoke } = window.__TAURI__.core;
const opener = window.__TAURI__.opener;
const { listen } = window.__TAURI__.event;

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
    await loadPiperVoices(s.piper_voice);
    await refreshPiperStatus();
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
  if (s.kokoro_voice) $("kokoro-voice").value = s.kokoro_voice;
  if (s.kokoro_dtype) $("kokoro-dtype").value = s.kokoro_dtype;
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
    piper_voice: $("piper-voice").value,
    kokoro_voice: $("kokoro-voice").value,
    kokoro_dtype: $("kokoro-dtype").value,
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
  // The System voice has its own rate control; everything else uses speed.
  $("speed-card").hidden = provider === "local";
}

document.querySelectorAll('input[name="provider"]').forEach((r) => {
  r.addEventListener("change", (e) => {
    currentProvider = e.target.value;
    showPanel(currentProvider);
    if (currentProvider === "piper") refreshPiperStatus();
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
// Piper (Local HD)
// ---------------------------------------------------------------------------
async function loadPiperVoices(selected) {
  try {
    const voices = await invoke("list_piper_voices");
    const sel = $("piper-voice");
    sel.innerHTML = "";
    for (const v of voices) {
      const opt = document.createElement("option");
      opt.value = v.key;
      opt.textContent = v.name + (v.installed ? "  ✓" : "");
      sel.appendChild(opt);
    }
    if (selected) sel.value = selected;
  } catch (_) {
    /* best effort */
  }
}

function setPiperStatus(text, kind) {
  const el = $("piper-status");
  el.textContent = text;
  el.className = "key-status " + (kind === "ok" ? "ok" : kind === "err" ? "err" : "");
}

async function refreshPiperStatus() {
  try {
    const st = await invoke("piper_status", { voice: $("piper-voice").value });
    if (st.engine && st.voice) setPiperStatus("✓ Downloaded — ready offline", "ok");
    else if (st.engine) setPiperStatus("Engine ready; this voice not downloaded yet.", "");
    else setPiperStatus("Not downloaded yet — click Download.", "");
  } catch (_) {
    /* ignore */
  }
}

let piperBusy = false;
$("piper-download").addEventListener("click", async () => {
  if (piperBusy) return;
  piperBusy = true;
  $("piper-download").disabled = true;
  setPiperStatus("Starting download…", "");
  try {
    await invoke("ensure_piper", { voice: $("piper-voice").value });
  } catch (e) {
    setPiperStatus("Could not start: " + e, "err");
    piperBusy = false;
    $("piper-download").disabled = false;
  }
});

$("piper-voice").addEventListener("change", refreshPiperStatus);

listen("hd-progress", (e) => {
  if (e.payload && e.payload.message) setPiperStatus(e.payload.message, "");
});
listen("hd-done", async (e) => {
  piperBusy = false;
  $("piper-download").disabled = false;
  if (e.payload && e.payload.ok) {
    setPiperStatus("✓ Voice ready — works offline from now on", "ok");
    await loadPiperVoices($("piper-voice").value);
  } else {
    setPiperStatus("Download failed: " + (e.payload && e.payload.error), "err");
  }
  updateStatus();
});

// ---------------------------------------------------------------------------
// Kokoro (beta) — neural TTS running in the webview via kokoro-js
// ---------------------------------------------------------------------------
let kokoroTTS = null;
let kokoroLoadingFor = null; // dtype currently being/loaded
let kokoroLoadPromise = null;
let kokoroCtx = null;
let kokoroSource = null;

function setKokoroStatus(text, kind) {
  const el = $("kokoro-status");
  if (!el) return;
  el.textContent = text;
  el.className = "key-status " + (kind === "ok" ? "ok" : kind === "err" ? "err" : "");
}

async function ensureKokoro(dtype) {
  dtype = dtype || "q8";
  if (kokoroTTS && kokoroLoadingFor === dtype) return kokoroTTS;
  if (kokoroLoadPromise && kokoroLoadingFor === dtype) return kokoroLoadPromise;

  kokoroLoadingFor = dtype;
  kokoroTTS = null;
  setKokoroStatus("Loading model (first time downloads once)…", "");

  kokoroLoadPromise = (async () => {
    const mod = await import("https://cdn.jsdelivr.net/npm/kokoro-js@1.2.1/+esm");
    const KokoroTTS = mod.KokoroTTS;

    // Prefer WebGPU when the webview supports it; otherwise fall back to WASM.
    let device = "wasm";
    try {
      if (navigator.gpu && (await navigator.gpu.requestAdapter())) device = "webgpu";
    } catch (_) {
      device = "wasm";
    }

    const tts = await KokoroTTS.from_pretrained(
      "onnx-community/Kokoro-82M-v1.0-ONNX",
      {
        dtype,
        device,
        progress_callback: (p) => {
          if (p && p.status === "progress" && p.file) {
            const pct = Math.round(p.progress || 0);
            setKokoroStatus(`Downloading ${p.file} … ${pct}%`, "");
          }
        },
      }
    );
    kokoroTTS = tts;
    setKokoroStatus(`✓ Model ready (${device})`, "ok");
    return tts;
  })();

  try {
    return await kokoroLoadPromise;
  } catch (e) {
    kokoroLoadPromise = null;
    kokoroLoadingFor = null;
    setKokoroStatus("Load failed: " + (e && e.message ? e.message : e), "err");
    throw e;
  }
}

function kokoroStop() {
  if (kokoroSource) {
    try {
      kokoroSource.stop();
    } catch (_) {}
    kokoroSource = null;
  }
}

async function playSamples(float32, sampleRate) {
  if (!kokoroCtx) kokoroCtx = new (window.AudioContext || window.webkitAudioContext)();
  if (kokoroCtx.state === "suspended") {
    try {
      await kokoroCtx.resume();
    } catch (_) {}
  }
  const buffer = kokoroCtx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);
  const src = kokoroCtx.createBufferSource();
  src.buffer = buffer;
  src.connect(kokoroCtx.destination);
  kokoroStop();
  kokoroSource = src;
  src.start();
}

async function kokoroSpeak(text, voice, speed, dtype) {
  try {
    const tts = await ensureKokoro(dtype);
    setKokoroStatus("Synthesizing…", "");
    const out = await tts.generate(text, { voice: voice || "af_heart", speed: speed || 1 });
    // kokoro-js returns a RawAudio with `.audio` (Float32Array) + `.sampling_rate`.
    await playSamples(out.audio, out.sampling_rate);
    setKokoroStatus("▶ Playing", "ok");
  } catch (e) {
    setKokoroStatus("Error: " + (e && e.message ? e.message : e), "err");
  }
}

listen("kokoro-speak", (e) => {
  const p = e.payload || {};
  kokoroSpeak(p.text, p.voice, p.speed, p.dtype);
});
listen("kokoro-stop", () => kokoroStop());

$("kokoro-load").addEventListener("click", async () => {
  $("kokoro-load").disabled = true;
  try {
    await kokoroSpeak(
      "Kokoro is ready. This is how your selected text will sound.",
      $("kokoro-voice").value,
      parseFloat($("speed").value),
      $("kokoro-dtype").value
    );
  } finally {
    $("kokoro-load").disabled = false;
    updateStatus();
  }
});

// ---------------------------------------------------------------------------
// Status line
// ---------------------------------------------------------------------------
function setStatus(text, isError) {
  const el = $("status");
  el.textContent = text;
  el.style.opacity = isError ? "1" : "0.9";
}

async function updateStatus() {
  const names = {
    openai: "OpenAI",
    elevenlabs: "ElevenLabs",
    piper: "Local HD",
    kokoro: "Kokoro (beta)",
    local: "System voice",
  };
  let ready = true;
  let hint = "";
  if (currentProvider === "openai" || currentProvider === "elevenlabs") {
    ready = await invoke("has_api_key", { provider: currentProvider });
    hint = "add an API key to start";
  } else if (currentProvider === "piper") {
    const st = await invoke("piper_status", { voice: $("piper-voice").value });
    ready = st.engine && st.voice;
    hint = "download the voice to start";
  }
  setStatus(
    ready
      ? `Ready · ${names[currentProvider]}`
      : `${names[currentProvider]} selected — ${hint}`
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
