use std::{
    sync::mpsc::{sync_channel, SyncSender},
    thread::spawn,
};

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
    fn start(&self, channels: u8, rate: u32, format: AudioFormat) -> Box<dyn AudioSinkSession> {
        Box::new(RodioAudioSinkSession::new(channels, rate, format))
    }
}

pub struct RodioAudioSinkSession {
    channels: u16,
    rate: u32,
    format: AudioFormat,
    sender: SyncSender<SamplesBuffer<u16>>,
}

impl RodioAudioSinkSession {
    pub fn new(channels: u8, rate: u32, format: AudioFormat) -> Self {
        // rodio::OutputStream is not Sync, so we have to wrap them on thread
        let (sender, receiver) = sync_channel(20);

        spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();

            loop {
                let buffer = receiver.recv().unwrap();

                sink.append(buffer);
            }
        });

        Self {
            channels: channels as u16,
            rate,
            format,
            sender,
        }
    }
}

impl AudioSinkSession for RodioAudioSinkSession {
    fn write(&self, payload: &[u8]) {
        let buffer = match self.format {
            AudioFormat::S16NE => SamplesBuffer::new(self.channels, self.rate, unsafe { convert_vec(payload.to_vec()) }),
        };

        self.sender.send(buffer).unwrap();
    }
}
