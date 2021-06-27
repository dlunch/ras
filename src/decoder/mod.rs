use alac::{Decoder as AlacDecoder, StreamInfo};

pub trait Decoder: Send {
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
