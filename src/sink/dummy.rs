use log::trace;

use super::{AudioFormat, AudioSink, AudioSinkSession};

pub struct DummyAudioSink {}

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
