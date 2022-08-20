use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};
use rtp_rs::RtpReader;
use tokio_util::codec::Decoder;

pub struct RtpPacket {
    pub payload_type: u8,
    pub payload: Vec<u8>,
}

pub struct RtpControlPacket {
    pub timestamp: u32,
    pub current_time_seconds: u32,
    pub current_time_fraction: u32,
    pub next_timestamp: u32,
}

pub struct RtpCodec {}

impl Decoder for RtpCodec {
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

pub struct RtpControlCodec {}

impl Decoder for RtpControlCodec {
    type Item = RtpControlPacket;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // control packet looks likes regular rtp packet, but it comes without ssrc part.

        #[repr(C)]
        struct RawRtpControlPacket {
            rtp_header: [u8; 4],
            rtp_timestamp: [u8; 4],
            ntp_time_seconds: [u8; 4],
            ntp_time_fraction: [u8; 4],
            next_timestamp: [u8; 4],
        }

        if src.len() < core::mem::size_of::<RawRtpControlPacket>() {
            return Ok(None);
        }
        let data = unsafe { &*(src.as_ptr() as *const RawRtpControlPacket) };
        src.advance(core::mem::size_of::<RawRtpControlPacket>());

        Ok(Some(RtpControlPacket {
            timestamp: u32::from_be_bytes(data.rtp_timestamp),
            current_time_seconds: u32::from_be_bytes(data.ntp_time_seconds),
            current_time_fraction: u32::from_be_bytes(data.ntp_time_fraction),
            next_timestamp: u32::from_be_bytes(data.next_timestamp),
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_control() -> Result<()> {
        let data = vec![
            0x90u8, 0xd4, 0x00, 0x04, 0x76, 0xc4, 0x5c, 0x94, 0x83, 0xac, 0xce, 0x14, 0x57, 0xfc, 0x53, 0x13, 0x76, 0xc5, 0x8a, 0x0b,
        ];

        let mut codec = RtpControlCodec {};
        let mut bytes = BytesMut::from(data.as_slice());

        let req = codec.decode(&mut bytes)?.unwrap();

        assert_eq!(req.timestamp, 1992580244);
        assert_eq!(req.current_time_seconds, 2209140244);
        assert_eq!(req.current_time_fraction, 1476154131);
        assert_eq!(req.next_timestamp, 1992657419);

        Ok(())
    }
}
