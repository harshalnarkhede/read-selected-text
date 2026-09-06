//! Piper: free, offline, neural local voice. The engine binary and voice model
//! are fetched on first use (see [`crate::download`]). Synthesis shells out to
//! the piper executable and returns WAV bytes for the player.

use std::io::Write;
use std::process::{Command, Stdio};

use tauri::AppHandle;

use crate::download;
use crate::settings::Settings;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn synthesize(app: &AppHandle, settings: &Settings, text: &str) -> Result<Vec<u8>, String> {
    let exe = download::ensure_engine(app)?;
    let model = download::ensure_voice(app, &settings.piper_voice)?;

    let engine_dir = exe
        .parent()
        .ok_or("engine directory not found")?
        .to_path_buf();

    // Output to a unique temp WAV file.
    let out = std::env::temp_dir().join(format!(
        "rst-piper-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    // Piper uses length_scale as the inverse of speed (higher = slower).
    let speed = settings.speed.clamp(0.5, 2.0);
    let length_scale = (1.0 / speed).to_string();

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&engine_dir);
    cmd.args([
        "--model",
        model.to_string_lossy().as_ref(),
        "--output_file",
        out.to_string_lossy().as_ref(),
        "--length_scale",
        &length_scale,
    ]);

    // Help the dynamic loader find the bundled onnxruntime next to the binary.
    #[cfg(all(unix, not(target_os = "macos")))]
    cmd.env("LD_LIBRARY_PATH", &engine_dir);
    #[cfg(target_os = "macos")]
    cmd.env("DYLD_LIBRARY_PATH", &engine_dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start Piper: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("write to Piper: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Piper failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Piper error: {}", err.trim()));
    }

    let bytes = std::fs::read(&out).map_err(|e| format!("read Piper output: {e}"))?;
    let _ = std::fs::remove_file(&out);
    if bytes.is_empty() {
        return Err("Piper produced no audio.".into());
    }
    Ok(bytes)
}
