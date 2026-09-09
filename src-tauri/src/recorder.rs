use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{self, AudioRecorder};
use crate::cleanup::{cleanup_partial, cleanup_text};
use crate::paste::paste_text;
use crate::settings::Settings;
use crate::transcribe_groq;
use crate::transcribe_local;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum RecordingState {
    Ready,
    Recording,
    Transcribing,
}

fn update_overlay(app: &AppHandle, state: &RecordingState) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let class = match state {
            RecordingState::Ready => "mic",
            RecordingState::Recording => "mic recording",
            RecordingState::Transcribing => "mic transcribing",
        };
        let js = format!("document.getElementById('mic').className = '{}';", class);
        let _ = overlay.eval(&js);
    }
}

pub struct Recorder {
    state: Arc<Mutex<RecordingState>>,
    audio_recorder: Arc<Mutex<AudioRecorder>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::Ready)),
            audio_recorder: Arc::new(Mutex::new(AudioRecorder::new())),
        }
    }

    pub fn get_state(&self) -> RecordingState {
        self.state.lock().unwrap().clone()
    }

    pub fn start_recording(&self, app: &AppHandle, mic_name: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if *state != RecordingState::Ready {
            return Err("Already recording or transcribing".to_string());
        }

        let mut recorder = self.audio_recorder.lock().unwrap();
        recorder.start(mic_name)?;

        *state = RecordingState::Recording;
        let _ = app.emit("recording-state", RecordingState::Recording);
        update_overlay(app, &RecordingState::Recording);
        Ok(())
    }

    pub async fn stop_and_transcribe(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<String, String> {
        // Stop recording
        {
            let mut state = self.state.lock().unwrap();
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
            *state = RecordingState::Transcribing;
            let _ = app.emit("recording-state", RecordingState::Transcribing);
            update_overlay(app, &RecordingState::Transcribing);
        }

        let result = self.save_and_transcribe(app, settings, app_dir).await;

        // Always return to Ready, whether transcription succeeded or failed,
        // otherwise a failure here leaves the app stuck showing "Transcribing".
        {
            let mut state = self.state.lock().unwrap();
            *state = RecordingState::Ready;
            let _ = app.emit("recording-state", RecordingState::Ready);
            update_overlay(app, &RecordingState::Ready);
        }

        result
    }

    async fn save_and_transcribe(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<String, String> {
        let pcm = {
            let mut recorder = self.audio_recorder.lock().unwrap();
            recorder.stop_and_save()?
        };
        // None means everything was already said via mid-recording chunks.
        let Some(pcm) = pcm else {
            return Ok(String::new());
        };

        let temp_path = app_dir.join("temp_recording.wav");
        audio::write_wav(&pcm, &temp_path)?;

        let raw_text = transcribe(app, settings, app_dir, &temp_path).await?;
        let _ = std::fs::remove_file(&temp_path);

        let cleaned = cleanup_text(&raw_text);
        if !cleaned.is_empty() {
            paste_text(&cleaned)?;
        }

        Ok(cleaned)
    }

    /// Runs periodically while recording, so speech gets typed out in
    /// chunks instead of waiting for the whole recording to finish. Each
    /// chunk is treated as a standalone fragment: lightly cleaned (no
    /// forced capitalization/punctuation, since it's mid-sentence) and
    /// pasted immediately, with no correction of earlier chunks.
    pub async fn flush_chunk(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<(), String> {
        if self.get_state() != RecordingState::Recording {
            return Ok(());
        }

        let pcm = {
            let mut recorder = self.audio_recorder.lock().unwrap();
            recorder.drain_chunk()
        };
        let Some(pcm) = pcm else {
            return Ok(());
        };

        let chunk_path = app_dir.join("chunk_recording.wav");
        audio::write_wav(&pcm, &chunk_path)?;

        let raw_text = transcribe(app, settings, app_dir, &chunk_path).await;
        let _ = std::fs::remove_file(&chunk_path);
        let raw_text = raw_text?;

        let cleaned = cleanup_partial(&raw_text);
        if !cleaned.is_empty() {
            paste_text(&format!("{} ", cleaned))?;
        }

        Ok(())
    }
}

async fn transcribe(
    app: &AppHandle,
    settings: &Settings,
    app_dir: &PathBuf,
    audio_path: &PathBuf,
) -> Result<String, String> {
    match settings.engine.as_str() {
        "local" => {
            let model_path = app_dir.join(transcribe_local::model_filename(&settings.whisper_model));
            transcribe_local::transcribe_local(app, &model_path, audio_path, &settings.language).await
        }
        "cloud" => transcribe_groq::transcribe_groq(&settings.groq_api_key, audio_path).await,
        _ => Err(format!("Unknown engine: {}", settings.engine)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_ready() {
        let recorder = Recorder::new();
        assert_eq!(recorder.get_state(), RecordingState::Ready);
    }
}
