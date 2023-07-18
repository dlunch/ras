mod dummy;
mod rodio;

use std::rc::Rc;

use anyhow::Result;

#[derive(Copy, Clone)]
pub enum AudioFormat {
    S16BE,
    S16NE,
}

pub trait AudioSink {
    fn start(&self) -> Result<Rc<dyn AudioSinkSession>>;
}

pub trait AudioSinkSession: Send + Sync {
    fn write(&self, payload: &[u8], channels: u8, rate: u32, format: AudioFormat) -> Result<()>;
    fn set_volume(&self, volume: f32);
}

pub fn create(sink: &str) -> Rc<dyn AudioSink> {
    match sink {
        "dummy" => Rc::new(dummy::DummyAudioSink::new()),
        "rodio" => Rc::new(rodio::RodioAudioSink::new()),
        _ => panic!("Unknown sink"),
    }
}
