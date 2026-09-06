//! ElevenLabs text-to-speech (`/v1/text-to-speech/{voice_id}`).

use crate::settings::{self, Provider, Settings};
use serde_json::json;
use std::time::Duration;

pub fn synthesize(settings: &Settings, text: &str) -> Result<Vec<u8>, String> {
    let key = settings::get_api_key(Provider::Elevenlabs)
        .filter(|k| !k.trim().is_empty())
        .ok_or("No ElevenLabs API key set. Add one in settings.")?;

    let voice_id = settings.elevenlabs_voice_id.trim();
    if voice_id.is_empty() {
        return Err("No ElevenLabs voice ID set. Add one in settings.".into());
    }

    let url = format!(
        "https://api.elevenlabs.io/v1/text-to-speech/{voice_id}?output_format=mp3_44100_128"
    );

    // ElevenLabs accepts a speed in voice_settings (roughly 0.7 - 1.2).
    let speed = settings.speed.clamp(0.7, 1.2);

    let body = json!({
        "text": text,
        "model_id": settings.elevenlabs_model,
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75,
            "speed": speed,
        }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .header("xi-api-key", key)
        .header("accept", "audio/mpeg")
        .json(&body)
        .send()
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("ElevenLabs error {status}: {}", truncate(&text, 300)));
    }

    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
