use std::collections::HashMap;

use async_std::io::{self, prelude::BufReadExt, BufReader, ReadExt};
use futures::{future, stream::TryStreamExt};

pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub content: Vec<u8>,
}

impl Request {
    pub async fn parse<S>(stream: S) -> io::Result<Option<Self>>
    where
        S: io::Read + Unpin,
    {
        let mut reader = BufReader::new(stream);
        let lines = reader
            .by_ref()
            .lines()
            .try_take_while(|x| future::ready(Ok(!x.is_empty())))
            .try_collect::<Vec<_>>()
            .await?;

        if lines.len() < 2 {
            return Ok(None);
        }

        let split = lines[0].split(' ').collect::<Vec<_>>();
        let (method, path, _) = (split[0].into(), split[1].into(), split[2]);

        let mut headers = HashMap::new();
        for header_line in lines.into_iter().skip(1) {
            let split = header_line.split(':').collect::<Vec<_>>();

            let (key, value) = (split[0].trim().to_owned(), split[1].trim().to_owned());

            headers.insert(key, value);
        }

        // TODO header casing
        let content = if let Some(length) = headers.get("Content-Length") {
            let length = length.parse::<usize>().unwrap();

            let mut content = vec![0; length];
            reader.read_exact(&mut content).await?;

            content
        } else {
            Vec::new()
        };

        Ok(Some(Self {
            method,
            path,
            headers,
            content,
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[async_std::test]
    async fn test_simple_request() -> io::Result<()> {
        let data = "GET /info RTSP/1.0\r\nX-Apple-ProtocolVersion: 1\r\nCSeq: 0\r\n\r\n";

        let req = Request::parse(data.as_bytes()).await?;
        assert!(req.is_some());

        let req = req.unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/info");

        Ok(())
    }
}
