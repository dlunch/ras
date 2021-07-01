mod dummy;
mod rodio;

pub enum AudioFormat {
    S16NE,
}

pub trait AudioSink: Send + Sync {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Box<dyn AudioSinkSession>;
}

pub trait AudioSinkSession: Send + Sync {
    fn write(&self, payload: &[u8]);
}

cfg_if::cfg_if! {
    if #[cfg(all(unix, not(target_os = "macos")))] {
        mod pulseaudio;
    }
}

pub fn create(sink: &str) -> Box<dyn AudioSink> {
    match sink {
        #[cfg(all(unix, not(target_os = "macos")))]
        "pulseaudio" => Box::new(pulseaudio::PulseAudioSink::new()),
        "dummy" => Box::new(dummy::DummyAudioSink::new()),
        "rodio" => Box::new(rodio::RodioAudioSink::new()),
        _ => panic!("Unknown sink"),
    }
}
