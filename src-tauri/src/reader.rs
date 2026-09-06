//! Capturing the current selection and reading it aloud.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::player::Player;
use crate::settings::Settings;
use crate::tts::{self, Speech};

/// Shared application state, managed by Tauri and reachable from hotkey handlers.
pub struct AppState {
    pub player: Arc<Player>,
    pub settings: Arc<Mutex<Settings>>,
}

/// Simulate the platform copy shortcut so the focused app copies its selection.
fn simulate_copy() -> Result<(), String> {
    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| e.to_string())?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Capture the currently-selected text by copying it to the clipboard.
///
/// The clipboard is cleared first so an empty result reliably means
/// "nothing was selected", and the previous contents are restored afterwards
/// when `restore` is set.
fn capture_selection(restore: bool) -> Result<String, String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("clipboard: {e}"))?;
    let previous = clipboard.get_text().ok();

    // Sentinel so we can detect whether the copy produced anything.
    let _ = clipboard.set_text("");
    // Small delay so the emptied clipboard settles before we send copy.
    std::thread::sleep(Duration::from_millis(30));

    simulate_copy()?;

    // Give the foreground app time to service the copy command.
    std::thread::sleep(Duration::from_millis(180));

    let text = clipboard.get_text().unwrap_or_default();

    if restore {
        if let Some(prev) = previous {
            let _ = clipboard.set_text(prev);
        }
    }

    Ok(text)
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

/// Speak the given text using the provided settings, routing to the player.
pub fn speak_text(app: &AppHandle, player: &Player, settings: &Settings, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        notify(app, "Read Selected Text", "No text was selected.");
        return;
    }
    match tts::speak(settings, text) {
        Ok(Speech::Audio(bytes)) => player.play_bytes(bytes),
        Ok(Speech::Local(child)) => player.set_local_child(child),
        Err(e) => notify(app, "Could not read text", &e),
    }
}

/// Capture the selection and read it. Runs the slow work on a worker thread.
pub fn trigger_read(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state: tauri::State<AppState> = app.state();
        let settings = {
            match state.settings.lock() {
                Ok(g) => g.clone(),
                Err(_) => Settings::default(),
            }
        };
        let player = state.player.clone();

        match capture_selection(settings.restore_clipboard) {
            Ok(text) => speak_text(&app, &player, &settings, &text),
            Err(e) => notify(&app, "Could not capture selection", &e),
        }
    });
}

/// Stop any current playback.
pub fn trigger_stop(app: &AppHandle) {
    let state: tauri::State<AppState> = app.state();
    state.player.stop();
}
