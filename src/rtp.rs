use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};
use rtp_rs::RtpReader;
use tokio_util::codec::Decoder;

pub struct RtpPacket {
    pub payload_type: u8,
    pub payload: Vec<u8>,
}

pub struct Codec {}

impl Decoder for Codec {
    type Item = RtpPacket;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (packet, length) = {
            let reader = RtpReader::new(src);
            if let Err(e) = reader {
                return match e {
                    rtp_rs::RtpReaderError::BufferTooShort(_) => Ok(None),
                    rtp_rs::RtpReaderError::UnsupportedVersion(_) => Err(anyhow!("{:?}", e)),
                    rtp_rs::RtpReaderError::HeadersTruncated { .. } => Ok(None),
                    rtp_rs::RtpReaderError::PaddingLengthInvalid(_) => Ok(None),
                };
            }

            let reader = reader.unwrap();
            Ok::<_, Self::Error>((
                RtpPacket {
                    payload_type: reader.payload_type(),
                    payload: reader.payload().to_vec(),
                },
                reader.payload_offset() + reader.payload().len(),
            ))
        }?;

        src.advance(length);

        Ok(Some(packet))
    }
}
