use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, serde::Serialize)]
pub struct MicDevice {
    pub name: String,
    pub is_default: bool,
}

pub fn list_microphones() -> Vec<MicDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut devices = Vec::new();
    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            if let Ok(name) = device.name() {
                devices.push(MicDevice {
                    is_default: name == default_name,
                    name,
                });
            }
        }
    }
    devices
}

/// Wrapper to make cpal::Stream usable across threads.
/// SAFETY: cpal::Stream on macOS (CoreAudio) is thread-safe in practice;
/// we only access it behind a Mutex to start/stop recording.
struct SendStream(#[allow(dead_code)] cpal::Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

pub struct AudioRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<SendStream>,
    source_sample_rate: u32,
    source_channels: u16,
    /// Set the moment any audio callback fires. Unlike `samples` (which gets
    /// drained/cleared by chunk flushes), this tracks whether the recording
    /// captured anything AT ALL, so the final stop can tell "silent the
    /// whole time" (a real error) apart from "already said via chunks,
    /// nothing left" (not an error).
    ever_captured: Arc<AtomicBool>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            source_sample_rate: 48000,
            source_channels: 1,
            ever_captured: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self, mic_name: &str) -> Result<(), String> {
        // Clear any leftover samples from previous recording
        self.samples.lock().unwrap().clear();
        self.ever_captured.store(false, Ordering::SeqCst);

        let host = cpal::default_host();

        let device = if mic_name == "default" {
            host.default_input_device()
                .ok_or("No default input device found")?
        } else {
            host.input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().map(|n| n == mic_name).unwrap_or(false))
                .ok_or(format!("Microphone '{}' not found", mic_name))?
        };

        // Use the device's default config instead of forcing 16kHz
        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        println!("[TypeIt] Mic config: {}Hz, {} channels", sample_rate, channels);

        self.source_sample_rate = sample_rate;
        self.source_channels = channels;

        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let samples = self.samples.clone();
        let ever_captured = self.ever_captured.clone();
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !data.is_empty() {
                        ever_captured.store(true, Ordering::SeqCst);
                    }
                    let mut buf = samples.lock().unwrap();
                    buf.extend_from_slice(data);
                },
                |err| {
                    eprintln!("[TypeIt] Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(SendStream(stream));
        println!("[TypeIt] Audio recording started");
        Ok(())
    }

    /// Converts a raw interleaved slice to mono, resampled 16kHz 16-bit PCM.
    fn to_pcm16(&self, raw: &[f32]) -> Vec<i16> {
        let mono: Vec<f32> = if self.source_channels > 1 {
            raw.chunks(self.source_channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                .collect()
        } else {
            raw.to_vec()
        };
        let resampled = resample(&mono, self.source_sample_rate, 16000);
        resampled
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect()
    }

    /// Grabs and clears whatever's accumulated so far, for a mid-recording
    /// streaming chunk. Returns None if there isn't at least ~0.25s of new
    /// audio yet, leaving it in the buffer so it accumulates for next tick.
    pub fn drain_chunk(&mut self) -> Option<Vec<i16>> {
        let mut samples = self.samples.lock().unwrap();
        if samples.len() < self.source_sample_rate as usize / 4 {
            return None;
        }
        let raw = std::mem::take(&mut *samples);
        drop(samples);
        Some(self.to_pcm16(&raw))
    }

    /// Grabs whatever's left in the buffer when recording stops.
    /// Ok(None) means the recording wasn't empty overall, there's just
    /// nothing new since the last chunk (not an error).
    pub fn stop_and_save(&mut self) -> Result<Option<Vec<i16>>, String> {
        self.stream = None; // Drop stops the stream
        println!("[TypeIt] Audio recording stopped");

        if !self.ever_captured.load(Ordering::SeqCst) {
            return Err("No audio captured".to_string());
        }
        let mut samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return Ok(None); // everything already flushed via chunks
        }
        let raw = std::mem::take(&mut *samples);
        drop(samples);
        println!("[TypeIt] Captured {} raw samples in final chunk", raw.len());
        Ok(Some(self.to_pcm16(&raw)))
    }
}

pub fn write_wav(pcm: &[i16], output_path: &PathBuf) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(output_path, spec).map_err(|e| e.to_string())?;
    for &sample in pcm {
        writer.write_sample(sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    println!("[TypeIt] WAV saved to {:?}", output_path);
    Ok(())
}

/// Simple linear interpolation resampler
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac
        } else {
            samples[idx.min(samples.len() - 1)] as f64
        };

        output.push(sample as f32);
    }

    output
}
