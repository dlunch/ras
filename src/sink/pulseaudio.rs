use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

use super::{AudioFormat, AudioSink, AudioSinkSession};

pub struct PulseAudioSink {}

impl PulseAudioSink {
    pub fn new() -> Self {
        Self {}
    }

    fn convert_format(format: AudioFormat) -> Format {
        match format {
            AudioFormat::S32NE => Format::S32NE,
            AudioFormat::S16NE => Format::S16NE,
        }
    }
}

impl AudioSink for PulseAudioSink {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Box<dyn AudioSinkSession> {
        let spec = Spec {
            format: Self::convert_format(format),
            channels,
            rate,
        };

        let simple = Simple::new(None, "RAS", Direction::Playback, None, "Music", &spec, None, None).unwrap();

        Box::new(PulseAudioSinkSession { simple })
    }
}

pub struct PulseAudioSinkSession {
    simple: Simple,
}

impl AudioSinkSession for PulseAudioSinkSession {
    fn write(&self, payload: &[u8]) {
        self.simple.write(payload).unwrap()
    }
}
