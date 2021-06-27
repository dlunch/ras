use log::trace;

#[allow(dead_code)]
pub enum AudioFormat {
    S16NE,
    S32NE,
}

pub trait AudioSink: Send + Sync {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Box<dyn AudioSinkSession>;
}

pub trait AudioSinkSession: Send + Sync {
    fn write(&self, payload: &[u8]);
}

pub struct DummyAudioSink {}

#[allow(dead_code)]
impl DummyAudioSink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioSink for DummyAudioSink {
    fn start(&self, _: u8, _: u32, _: AudioFormat) -> Box<dyn AudioSinkSession> {
        Box::new(DummyAudioSinkSession {})
    }
}

pub struct DummyAudioSinkSession {}

impl AudioSinkSession for DummyAudioSinkSession {
    fn write(&self, payload: &[u8]) {
        trace!("DummyAudioSink::write {:?}", payload);
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(unix, not(target_os = "macos")))] {
        mod pulseaudio;
        pub use pulseaudio::PulseAudioSink;
    }
}

pub fn create_default_audio_sink() -> Box<dyn AudioSink> {
    cfg_if::cfg_if! {
        if #[cfg(all(unix, not(target_os = "macos")))] {
            Box::new(PulseAudioSink::new())
        }
        else {
            Box::new(DummyAudioSink::new())
        }
    }
}
