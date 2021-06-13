use std::collections::HashMap;

use async_std::io::{self, prelude::BufReadExt, BufReader, ReadExt};
use async_std::stream::StreamExt;

pub struct Request {
    pub(super) method: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) content: Vec<u8>,
}

impl Request {
    pub async fn parse<S>(stream: S) -> io::Result<Self>
    where
        S: io::Read + Unpin,
    {
        let mut reader = BufReader::new(stream);
        let mut lines = reader.by_ref().lines();

        let method_line = lines.next().await.unwrap()?;

        let split = method_line.split(' ').collect::<Vec<_>>();
        let (method, _, _) = (split[0].into(), split[1], split[2]);

        let mut headers = HashMap::new();
        loop {
            let header_line = lines.next().await.unwrap()?;
            if header_line.is_empty() {
                break;
            }

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

        Ok(Self { method, headers, content })
    }
}
