use std::collections::HashMap;

use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub content: Vec<u8>,
}

impl Request {
    pub async fn parse<S>(stream: S) -> Result<Self>
    where
        S: AsyncRead + Unpin,
    {
        let mut reader = BufReader::new(stream);

        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;

            line = line.trim_end().to_string();

            if line.is_empty() {
                break;
            }

            lines.push(line);
        }

        if lines.len() < 2 {
            return Err(anyhow!("Buffer too short"));
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
            let length = length.parse::<usize>()?;

            let mut content = vec![0; length];
            reader.read_exact(&mut content).await?;

            content
        } else {
            Vec::new()
        };

        Ok(Self {
            method,
            path,
            headers,
            content,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_simple_request() -> Result<()> {
        let data = "GET /info RTSP/1.0\r\nX-Apple-ProtocolVersion: 1\r\nCSeq: 0\r\n\r\n";

        let req = Request::parse(data.as_bytes()).await?;

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/info");

        Ok(())
    }
}
