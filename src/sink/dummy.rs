use std::sync::Arc;

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
    fn start(&self) -> Result<Arc<dyn AudioSinkSession>> {
        Ok(Arc::new(DummyAudioSinkSession {}))
    }
}

pub struct DummyAudioSinkSession {}

impl AudioSinkSession for DummyAudioSinkSession {
    fn write(&self, payload: &[u8], _: u8, _: u32, _: AudioFormat) -> Result<()> {
        trace!("DummyAudioSink::write {:?}", payload);

        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        trace!("DummyAudioSink::set_volume {:?}", volume);
    }
}
