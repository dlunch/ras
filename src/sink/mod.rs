use log::trace;

pub trait AudioSink: Send + Sync {
    fn write(&self, payload: &[i32]);
}

pub struct DummyAudioSink {}

impl DummyAudioSink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioSink for DummyAudioSink {
    fn write(&self, payload: &[i32]) {
        trace!("DummyAudioSink::write {:?}", payload);
    }
}
