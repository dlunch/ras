mod dummy;
#[cfg(all(unix, not(target_os = "macos")))]
mod pulseaudio;
mod rodio;

use std::sync::Arc;

use anyhow::Result;

#[derive(Copy, Clone)]
pub enum AudioFormat {
    S16BE,
    S16NE,
}

pub trait AudioSink: Send + Sync {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Result<Box<dyn AudioSinkSession>>;
}

pub trait AudioSinkSession: Send + Sync {
    fn write(&self, payload: &[u8]) -> Result<()>;
}

pub fn create(sink: &str) -> Arc<dyn AudioSink> {
    match sink {
        #[cfg(all(unix, not(target_os = "macos")))]
        "pulseaudio" => Arc::new(pulseaudio::PulseAudioSink::new()),
        "dummy" => Arc::new(dummy::DummyAudioSink::new()),
        "rodio" => Arc::new(rodio::RodioAudioSink::new()),
        _ => panic!("Unknown sink"),
    }
}
