use std::sync::Arc;

use anyhow::Result;
use cfg_if::cfg_if;
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};

use super::{AudioFormat, AudioSink, AudioSinkSession};
use crate::util::convert_vec;

pub struct RodioAudioSink {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

impl RodioAudioSink {
    pub fn new() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();

        Self { _stream, stream_handle }
    }
}

impl AudioSink for RodioAudioSink {
    fn start(&self) -> Result<Arc<dyn AudioSinkSession>> {
        let sink = Sink::try_new(&self.stream_handle)?;

        Ok(Arc::new(RodioAudioSinkSession::new(sink)?))
    }
}

pub struct RodioAudioSinkSession {
    sink: Sink,
}

impl RodioAudioSinkSession {
    pub fn new(sink: Sink) -> Result<Self> {
        Ok(Self { sink })
    }
}

impl AudioSinkSession for RodioAudioSinkSession {
    fn write(&self, payload: &[u8], channels: u8, rate: u32, format: AudioFormat) -> Result<()> {
        let buffer = match format {
            AudioFormat::S16NE => SamplesBuffer::new(channels as u16, rate, unsafe { convert_vec(payload.to_vec()) }),
            AudioFormat::S16BE => {
                cfg_if! {
                    if #[cfg(target_endian = "big")] {
                        SamplesBuffer::new(channels as u16, rate, unsafe { convert_vec(payload.to_vec()) })
                    }
                    else if #[cfg(target_endian = "little")] {
                        let mut buf = vec![0; payload.len() / 2];
                        for i in 0..payload.len() / 2 {
                            buf[i] = i16::from_be_bytes([payload[i * 2], payload[i * 2 + 1]]);
                        }
                        SamplesBuffer::new(channels as u16, rate, buf)
                    }
                }
            }
        };

        self.sink.append(buffer);

        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        // airplay volume: -30.0 ~ 0.0, -144: mute, -20: default
        // it's in decibel, but i'm lazy to convert it correctly into linear scale..
        let volume = if volume == -144.0 { 0.0 } else { 1.0 + (volume / 30.0) };

        self.sink.set_volume(volume);
    }
}
