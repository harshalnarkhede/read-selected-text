//! Local system voice: no API key, works offline. Implemented by shelling out
//! to each platform's built-in speech tool so we avoid COM/threading pitfalls.
//!
//! * Windows: PowerShell + `System.Speech.Synthesis`
//! * macOS:   `say`
//! * Linux:   `espeak-ng` (or `espeak`)

use crate::settings::Settings;
#[cfg(unix)]
use std::io::Write;
use std::process::{Child, Command};
#[cfg(unix)]
use std::process::Stdio;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Speak `text` with the local voice, returning the spawned (already speaking)
/// child process so it can be stopped.
pub fn speak(settings: &Settings, text: &str) -> Result<Child, String> {
    let voice = settings.local_voice.trim();
    let rate = settings.local_rate;

    #[cfg(windows)]
    {
        speak_windows(text, voice, rate)
    }
    #[cfg(target_os = "macos")]
    {
        speak_macos(text, voice, rate)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        speak_linux(text, voice, rate)
    }
}

/// List available local voice names (best effort; empty on failure).
pub fn list_voices() -> Vec<String> {
    #[cfg(windows)]
    {
        list_windows()
    }
    #[cfg(target_os = "macos")]
    {
        list_macos()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        list_linux()
    }
}

// --- Windows -----------------------------------------------------------------

#[cfg(windows)]
fn unique_temp_txt() -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!("rst-speak-{}-{}.txt", std::process::id(), nanos));
    p
}

#[cfg(windows)]
fn speak_windows(text: &str, voice: &str, rate: i32) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;

    let path = unique_temp_txt();
    std::fs::write(&path, text.as_bytes()).map_err(|e| format!("temp file: {e}"))?;
    let path_str = path.to_string_lossy().replace('\'', "''");
    let rate = rate.clamp(-10, 10);
    let voice_line = if voice.is_empty() {
        String::new()
    } else {
        format!("try {{ $s.SelectVoice('{}') }} catch {{}};", voice.replace('\'', "''"))
    };

    let script = format!(
        "Add-Type -AssemblyName System.Speech; \
         $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $s.Rate = {rate}; {voice_line} \
         $t = [IO.File]::ReadAllText('{path_str}', [Text.Encoding]::UTF8); \
         $s.Speak($t); \
         Remove-Item -LiteralPath '{path_str}' -ErrorAction SilentlyContinue"
    );

    Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .spawn()
        .map_err(|e| format!("PowerShell speech failed: {e}"))
}

#[cfg(windows)]
fn list_windows() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    let script = "Add-Type -AssemblyName System.Speech; \
        (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() \
        | ForEach-Object { $_.VoiceInfo.Name }";
    let out = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

// --- macOS -------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn speak_macos(text: &str, voice: &str, rate: i32) -> Result<Child, String> {
    let mut cmd = Command::new("say");
    if !voice.is_empty() {
        cmd.args(["-v", voice]);
    }
    if rate != 0 {
        let wpm = (180 + rate * 12).clamp(80, 400);
        cmd.args(["-r", &wpm.to_string()]);
    }
    // Read the text from stdin to avoid any argument-quoting issues.
    cmd.arg("-f").arg("-");
    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("`say` failed: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    Ok(child)
}

#[cfg(target_os = "macos")]
fn list_macos() -> Vec<String> {
    let out = Command::new("say").args(["-v", "?"]).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                // Format: "Alex                en_US    # comment"
                let name = line.split("  ").next()?.trim();
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// --- Linux -------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
fn speak_linux(text: &str, voice: &str, rate: i32) -> Result<Child, String> {
    for exe in ["espeak-ng", "espeak"] {
        let mut cmd = Command::new(exe);
        if !voice.is_empty() {
            cmd.args(["-v", voice]);
        }
        if rate != 0 {
            let wpm = (175 + rate * 15).clamp(80, 450);
            cmd.args(["-s", &wpm.to_string()]);
        }
        cmd.arg("--stdin");
        match cmd.stdin(Stdio::piped()).spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                return Ok(child);
            }
            Err(_) => continue,
        }
    }
    Err("No local speech engine found. Install `espeak-ng` (e.g. `sudo apt install espeak-ng`).".into())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn list_linux() -> Vec<String> {
    for exe in ["espeak-ng", "espeak"] {
        if let Ok(o) = Command::new(exe).arg("--voices").output() {
            let voices: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .skip(1) // header row
                .filter_map(|line| line.split_whitespace().nth(3).map(|s| s.to_string()))
                .collect();
            if !voices.is_empty() {
                return voices;
            }
        }
    }
    Vec::new()
}
