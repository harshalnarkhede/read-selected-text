//! OpenAI text-to-speech (`/v1/audio/speech`).

use crate::settings::{self, Provider, Settings};
use serde_json::json;
use std::time::Duration;

const ENDPOINT: &str = "https://api.openai.com/v1/audio/speech";

pub fn synthesize(settings: &Settings, text: &str) -> Result<Vec<u8>, String> {
    let key = settings::get_api_key(Provider::Openai)
        .filter(|k| !k.trim().is_empty())
        .ok_or("No OpenAI API key set. Add one in settings.")?;

    let speed = settings.speed.clamp(0.25, 4.0);

    let body = json!({
        "model": settings.openai_model,
        "voice": settings.openai_voice,
        "input": text,
        "response_format": "mp3",
        "speed": speed,
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(ENDPOINT)
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("OpenAI error {status}: {}", truncate(&text, 300)));
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
