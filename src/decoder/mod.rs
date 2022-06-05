use std::{mem::size_of, slice};

use anyhow::Result;
use symphonia::{
    core::{
        audio::RawSampleBuffer,
        codecs::{CodecParameters, Decoder as SymphoniaDecoder, DecoderOptions, CODEC_TYPE_ALAC},
        formats::Packet,
        sample::SampleFormat,
    },
    default::codecs::AlacDecoder,
};

use crate::sink::AudioFormat;

pub trait Decoder: Send {
    fn channels(&self) -> u8;
    fn rate(&self) -> u32;
    fn format(&self) -> AudioFormat;
    fn decode(&mut self, raw: &[u8]) -> Result<Vec<u8>>;
}

pub struct AppleLoselessDecoder {
    decoder: AlacDecoder,
    channels: u8,
    sample_rate: u32,
}

#[repr(C)]
#[repr(packed)]
struct MagicCookie {
    frame_length: [u8; 4],
    compatible_version: u8,
    bit_depth: u8,
    pb: u8,
    mb: u8,
    kb: u8,
    num_channels: u8,
    max_run: [u8; 2],
    max_frame_bytes: [u8; 4],
    avg_bit_rate: [u8; 4],
    sample_rate: [u8; 4],
}

impl AppleLoselessDecoder {
    pub fn new(fmtp: &str) -> Result<Self> {
        let magic_cookie = Self::fmtp_to_magic_cookie(fmtp)?;
        let magic_cookie_data: [u8; 24] =
            (unsafe { slice::from_raw_parts(&magic_cookie as *const MagicCookie as *const u8, size_of::<MagicCookie>()) }).try_into()?;

        let decoder = AlacDecoder::try_new(
            CodecParameters::new()
                .for_codec(CODEC_TYPE_ALAC)
                .with_sample_format(SampleFormat::S16)
                .with_extra_data(Box::new(magic_cookie_data)),
            &DecoderOptions::default(),
        )?;

        Ok(Self {
            decoder,
            channels: magic_cookie.num_channels,
            sample_rate: u32::from_be_bytes(magic_cookie.sample_rate),
        })
    }

    fn fmtp_to_magic_cookie(fmtp: &str) -> Result<MagicCookie> {
        // symphonia doesn't supports fmtp parsing, so here converts it to alac magic cookie manually
        let fmtp_params = fmtp.split(' ').collect::<Vec<_>>();
        Ok(MagicCookie {
            frame_length: fmtp_params[0].parse::<u32>()?.to_be_bytes(),
            compatible_version: fmtp_params[1].parse()?,
            bit_depth: fmtp_params[2].parse()?,
            pb: fmtp_params[3].parse()?,
            mb: fmtp_params[4].parse()?,
            kb: fmtp_params[5].parse()?,
            num_channels: fmtp_params[6].parse()?,
            max_run: fmtp_params[7].parse::<u16>()?.to_be_bytes(),
            max_frame_bytes: fmtp_params[8].parse::<u32>()?.to_be_bytes(),
            avg_bit_rate: fmtp_params[9].parse::<u32>()?.to_be_bytes(),
            sample_rate: fmtp_params[10].parse::<u32>()?.to_be_bytes(),
        })
    }
}

impl Decoder for AppleLoselessDecoder {
    fn channels(&self) -> u8 {
        self.channels
    }

    fn rate(&self) -> u32 {
        self.sample_rate
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::S16NE
    }

    fn decode(&mut self, raw: &[u8]) -> Result<Vec<u8>> {
        let packet = Packet::new_from_slice(0, 0, 0, raw);
        let decoded = self.decoder.decode(&packet)?;

        let spec = *decoded.spec();
        let duration = decoded.capacity() as u64;
        let mut sample_buffer = RawSampleBuffer::<i16>::new(duration, spec);
        sample_buffer.copy_interleaved_ref(decoded);

        Ok(sample_buffer.as_bytes().to_vec())
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
