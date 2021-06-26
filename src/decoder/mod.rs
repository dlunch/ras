use alac::{Decoder as AlacDecoder, StreamInfo};

pub trait Decoder: Send {
    fn decode(&mut self, raw: &[u8]) -> Vec<i32>;
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
    fn decode(&mut self, raw: &[u8]) -> Vec<i32> {
        let mut out = vec![0; self.decoder.stream_info().max_samples_per_packet() as usize];
        self.decoder.decode_packet(raw, &mut out).unwrap();

        out
    }
}
