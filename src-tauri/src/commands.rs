//! Tauri commands invoked from the settings UI.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::reader::{self, AppState};
use crate::settings::{self, Provider, Settings};
use crate::tts::{self, SAMPLE_TEXT};

fn parse_provider(s: &str) -> Result<Provider, String> {
    match s {
        "openai" => Ok(Provider::Openai),
        "elevenlabs" => Ok(Provider::Elevenlabs),
        "local" => Ok(Provider::Local),
        other => Err(format!("unknown provider: {other}")),
    }
}

#[derive(Serialize)]
pub struct ProviderStatus {
    pub has_openai: bool,
    pub has_elevenlabs: bool,
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn get_provider_status() -> ProviderStatus {
    ProviderStatus {
        has_openai: settings::has_api_key(Provider::Openai),
        has_elevenlabs: settings::has_api_key(Provider::Elevenlabs),
    }
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<AppState>, new: Settings) -> Result<(), String> {
    // Persist to disk.
    settings::save(&new)?;

    // Apply autostart preference.
    let autostart = app.autolaunch();
    if new.launch_at_startup {
        let _ = autostart.enable();
    } else {
        let _ = autostart.disable();
    }

    // Update in-memory copy.
    if let Ok(mut g) = state.settings.lock() {
        *g = new.clone();
    }

    // Re-register global shortcuts with any new key combos.
    crate::register_hotkeys(&app, &new)?;

    Ok(())
}

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<(), String> {
    let provider = parse_provider(&provider)?;
    settings::set_api_key(provider, &key)
}

#[tauri::command]
pub fn has_api_key(provider: String) -> Result<bool, String> {
    let provider = parse_provider(&provider)?;
    Ok(settings::has_api_key(provider))
}

#[tauri::command]
pub fn list_local_voices() -> Vec<String> {
    tts::local::list_voices()
}

#[tauri::command]
pub fn read_now(app: AppHandle) {
    reader::trigger_read(&app);
}

#[tauri::command]
pub fn stop_playback(app: AppHandle) {
    reader::trigger_stop(&app);
}

/// Speak a fixed sample using the provided (possibly unsaved) settings.
#[tauri::command]
pub fn test_voice(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let player = state.player.clone();
    // Run off the UI thread; network synthesis can take a moment.
    std::thread::spawn(move || {
        reader::speak_text(&app, &player, &settings, SAMPLE_TEXT);
    });
    Ok(())
}
