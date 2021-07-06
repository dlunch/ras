use anyhow::Result;
use log::trace;

use super::{AudioFormat, AudioSink, AudioSinkSession};

pub struct DummyAudioSink {}

impl DummyAudioSink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioSink for DummyAudioSink {
    fn start(&self, _: u8, _: u32, _: AudioFormat) -> Result<Box<dyn AudioSinkSession>> {
        Ok(Box::new(DummyAudioSinkSession {}))
    }
}

pub struct DummyAudioSinkSession {}

impl AudioSinkSession for DummyAudioSinkSession {
    fn write(&self, payload: &[u8]) -> Result<()> {
        trace!("DummyAudioSink::write {:?}", payload);

        Ok(())
    }
}
