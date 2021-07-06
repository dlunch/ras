use std::{
    sync::mpsc::{sync_channel, SyncSender},
    thread::spawn,
};

use anyhow::{Error, Result};
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

use super::{AudioFormat, AudioSink, AudioSinkSession};
use crate::util::convert_vec;

pub struct RodioAudioSink {}

impl RodioAudioSink {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioSink for RodioAudioSink {
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Result<Box<dyn AudioSinkSession>> {
        Ok(Box::new(RodioAudioSinkSession::new(channels, rate, format)?))
    }
}

pub struct RodioAudioSinkSession {
    channels: u16,
    rate: u32,
    format: AudioFormat,
    sender: SyncSender<SamplesBuffer<i16>>,
}

impl RodioAudioSinkSession {
    pub fn new(channels: u8, rate: u32, format: AudioFormat) -> Result<Self> {
        // rodio::OutputStream is not Sync, so we have to wrap them on thread
        let (sender, receiver) = sync_channel(20);

        spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default()?;
            let sink = Sink::try_new(&stream_handle)?;

            while let Ok(buffer) = receiver.recv() {
                sink.append(buffer);
            }

            Ok::<_, Error>(())
        });

        Ok(Self {
            channels: channels as u16,
            rate,
            format,
            sender,
        })
    }
}

impl AudioSinkSession for RodioAudioSinkSession {
    fn write(&self, payload: &[u8]) -> Result<()> {
        let buffer = match self.format {
            AudioFormat::S16NE => SamplesBuffer::new(self.channels, self.rate, unsafe { convert_vec(payload.to_vec()) }),
        };

        self.sender.send(buffer)?;

        Ok(())
    }
}
