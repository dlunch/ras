mod request;
mod response;

use std::collections::HashMap;
use std::str;

use anyhow::Result;
use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub use request::RtspRequest;
pub use response::{RtspResponse, RtspStatusCode};

pub struct RtspCodec {}

impl Decoder for RtspCodec {
    type Item = RtspRequest;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let header_end = src.windows(4).position(|window| window == b"\r\n\r\n");
        if header_end.is_none() {
            return Ok(None); //partial
        }
        let header_end = header_end.unwrap() + 4;
        let header = str::from_utf8(&src[..header_end - 4])?;

        let lines = header.split("\r\n").collect::<Vec<_>>();
        let (method, path, _) = {
            let split = lines[0].split(' ').collect::<Vec<_>>();

            (split[0].into(), split[1].into(), split[2])
        };

        let mut headers = HashMap::new();
        for header_line in lines.into_iter().skip(1) {
            let split = header_line.split(':').collect::<Vec<_>>();

            let (key, value) = (split[0].trim().to_owned(), split[1].trim().to_owned());

            headers.insert(key, value);
        }

        // TODO header casing
        let content = if let Some(length) = headers.get("Content-Length") {
            let length = length.parse::<usize>()?;

            if src.len() < header_end + length {
                return Ok(None); //partial
            }

            src[header_end..header_end + length].to_vec()
        } else {
            Vec::new()
        };

        src.advance(header_end + content.len());

        Ok(Some(RtspRequest {
            method,
            path,
            headers,
            content,
        }))
    }
}

impl Encoder<RtspResponse> for RtspCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: RtspResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend(format!("RTSP/1.0 {} {}\r\n", item.status as usize, item.status.as_string()).as_bytes());

        for (key, value) in &item.headers {
            let header_line = format!("{}: {}\r\n", key, value);

            dst.extend(header_line.as_bytes());
        }
        dst.extend("\r\n".as_bytes());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use maplit::hashmap;
    use std::str;

    #[tokio::test]
    async fn test_simple_request() -> Result<()> {
        let data = "GET /info RTSP/1.0\r\nX-Apple-ProtocolVersion: 1\r\nCSeq: 0\r\n\r\n";

        let mut codec = RtspCodec {};
        let mut bytes = BytesMut::from(data);

        let req = codec.decode(&mut bytes)?.unwrap();

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/info");
        assert_eq!(bytes.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_simple_response() -> Result<()> {
        let response = RtspResponse::with_headers(RtspStatusCode::Ok, hashmap! { "Test" => "Test".into() });

        let mut codec = RtspCodec {};
        let mut bytes = BytesMut::new();

        codec.encode(response, &mut bytes)?;

        let response_text = str::from_utf8(&bytes)?;
        assert_eq!(response_text, "RTSP/1.0 200 OK\r\nTest: Test\r\n\r\n");

        Ok(())
    }
}
