use anyhow::Result;
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
            AudioFormat::S16NE => Format::S16NE,
            AudioFormat::S16BE => Format::S16be,
        }
    }
}

impl AudioSink for PulseAudioSink {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Result<Box<dyn AudioSinkSession>> {
        let spec = Spec {
            format: Self::convert_format(format),
            channels,
            rate,
        };

        let simple = Simple::new(None, "RAS", Direction::Playback, None, "Music", &spec, None, None)?;

        Ok(Box::new(PulseAudioSinkSession { simple }))
    }
}

pub struct PulseAudioSinkSession {
    simple: Simple,
}

impl AudioSinkSession for PulseAudioSinkSession {
    fn write(&self, payload: &[u8]) -> Result<()> {
        self.simple.write(payload)?;

        Ok(())
    }
}
