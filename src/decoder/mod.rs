use alac::{Decoder as AlacDecoder, StreamInfo};

use crate::sink::AudioFormat;

pub trait Decoder: Send {
    fn channels(&self) -> u8;
    fn rate(&self) -> u32;
    fn format(&self) -> AudioFormat;
    fn decode(&mut self, raw: &[u8]) -> Vec<u8>;
}

pub struct AppleLoselessDecoder {
    decoder: AlacDecoder,
}

impl AppleLoselessDecoder {
    pub fn new(fmtp: &str) -> Self {
        let stream_info = StreamInfo::from_sdp_format_parameters(fmtp).unwrap();
        let decoder = AlacDecoder::new(stream_info);

        Self { decoder }
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
        AudioFormat::S32NE
    }

    fn decode(&mut self, raw: &[u8]) -> Vec<u8> {
        let mut out = vec![0; self.decoder.stream_info().max_samples_per_packet() as usize];
        self.decoder.decode_packet(raw, &mut out, true).unwrap();

        unsafe {
            let ratio = std::mem::size_of::<u32>() / std::mem::size_of::<u8>();

            let length = out.len() * ratio;
            let capacity = out.capacity() * ratio;
            let ptr = out.as_mut_ptr() as *mut u8;

            std::mem::forget(out);

            Vec::from_raw_parts(ptr, length, capacity)
        }
    }
}
