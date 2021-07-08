use alac::{Decoder as AlacDecoder, StreamInfo};
use anyhow::Result;

use crate::{sink::AudioFormat, util::convert_vec};

pub trait Decoder: Send {
    fn channels(&self) -> u8;
    fn rate(&self) -> u32;
    fn format(&self) -> AudioFormat;
    fn decode(&mut self, raw: &[u8]) -> Result<Vec<u8>>;
}

pub struct AppleLoselessDecoder {
    decoder: AlacDecoder,
}

impl AppleLoselessDecoder {
    pub fn new(fmtp: &str) -> Result<Self> {
        let stream_info = StreamInfo::from_sdp_format_parameters(fmtp)?;
        let decoder = AlacDecoder::new(stream_info);

        Ok(Self { decoder })
    }
}

impl Decoder for AppleLoselessDecoder {
    fn channels(&self) -> u8 {
        self.decoder.stream_info().channels()
    }

    fn rate(&self) -> u32 {
        self.decoder.stream_info().sample_rate()
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::S16NE
    }

    fn decode(&mut self, raw: &[u8]) -> Result<Vec<u8>> {
        let mut out = vec![0i16; self.decoder.stream_info().max_samples_per_packet() as usize];
        self.decoder.decode_packet(raw, &mut out, true)?;

        Ok(unsafe { convert_vec(out) })
    }
}

pub struct RawPCMDecoder {
    format: AudioFormat,
    channels: u8,
    rate: u32,
}

impl RawPCMDecoder {
    pub fn new(format: AudioFormat, channels: u8, rate: u32) -> Result<Self> {
        Ok(Self { format, channels, rate })
    }
}

impl Decoder for RawPCMDecoder {
    fn channels(&self) -> u8 {
        self.channels
    }

    fn rate(&self) -> u32 {
        self.rate
    }

    fn format(&self) -> AudioFormat {
        self.format
    }

    fn decode(&mut self, raw: &[u8]) -> Result<Vec<u8>> {
        Ok(raw.to_vec())
    }
}
