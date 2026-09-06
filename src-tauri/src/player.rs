//! Playback of both cloud TTS audio (MP3 bytes via rodio) and the local
//! system voice (via a subprocess), with a single `stop()` that covers both.

use std::io::Cursor;
use std::process::Child;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use rodio::{Decoder, OutputStream, Sink};

enum AudioCmd {
    Play(Vec<u8>),
    Stop,
}

/// Owns the audio output thread and any running local-voice process.
pub struct Player {
    audio_tx: Sender<AudioCmd>,
    local_child: Arc<Mutex<Option<Child>>>,
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCmd>();

        // Dedicated audio thread: the rodio OutputStream is not Send, so it must
        // live entirely on one thread. Commands arrive over the channel.
        std::thread::spawn(move || {
            let stream = OutputStream::try_default();
            let (_stream, handle) = match stream {
                Ok(s) => s,
                Err(e) => {
                    log::error!("audio output unavailable: {e}");
                    return;
                }
            };
            let mut current: Option<Sink> = None;
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCmd::Play(bytes) => {
                        if let Some(s) = current.take() {
                            s.stop();
                        }
                        match Sink::try_new(&handle) {
                            Ok(sink) => match Decoder::new(Cursor::new(bytes)) {
                                Ok(source) => {
                                    sink.append(source);
                                    current = Some(sink);
                                }
                                Err(e) => log::error!("decode audio: {e}"),
                            },
                            Err(e) => log::error!("create sink: {e}"),
                        }
                    }
                    AudioCmd::Stop => {
                        if let Some(s) = current.take() {
                            s.stop();
                        }
                    }
                }
            }
        });

        Player {
            audio_tx: tx,
            local_child: Arc::new(Mutex::new(None)),
        }
    }

    /// Play MP3 (or other rodio-decodable) bytes from a cloud provider.
    pub fn play_bytes(&self, bytes: Vec<u8>) {
        self.stop_local();
        let _ = self.audio_tx.send(AudioCmd::Play(bytes));
    }

    /// Register the child process spawned for local system-voice playback so it
    /// can be stopped later. Also stops any cloud audio.
    pub fn set_local_child(&self, child: Child) {
        let _ = self.audio_tx.send(AudioCmd::Stop);
        if let Ok(mut guard) = self.local_child.lock() {
            if let Some(mut old) = guard.take() {
                let _ = old.kill();
            }
            *guard = Some(child);
        }
    }

    fn stop_local(&self) {
        if let Ok(mut guard) = self.local_child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }
    }

    /// Stop everything currently playing.
    pub fn stop(&self) {
        let _ = self.audio_tx.send(AudioCmd::Stop);
        self.stop_local();
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
