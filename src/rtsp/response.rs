use std::collections::HashMap;

use async_std::io::{self, prelude::WriteExt};

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum StatusCode {
    Ok = 200,
    NotFound = 404,
}

impl StatusCode {
    pub fn as_string(&self) -> &'static str {
        match self {
            StatusCode::Ok => "OK",
            StatusCode::NotFound => "Not Found",
        }
    }
}

pub struct Response {
    pub status: StatusCode,
    pub headers: HashMap<&'static str, String>,
}

impl Response {
    pub fn new(status: StatusCode, headers: HashMap<&'static str, String>) -> Self {
        Self { status, headers }
    }

    pub async fn write<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: io::Write + Unpin,
    {
        let mut result = Vec::with_capacity(256);
        result.extend(format!("RTSP/1.0 {} {}\r\n", self.status as usize, self.status.as_string()).as_bytes());

        for (key, value) in &self.headers {
            let header_line = format!("{}: {}\r\n", key, value);

            result.extend(header_line.as_bytes());
        }
        result.extend("\r\n".as_bytes());

        stream.write(&result).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::hashmap;
    use std::str;

    #[async_std::test]
    async fn test_simple_response() {
        let response = Response::new(StatusCode::Ok, hashmap! { "Test" => "Test".into() });

        let mut buf = Vec::new();
        response.write(&mut buf).await.unwrap();

        let response_text = str::from_utf8(&buf).unwrap();
        assert_eq!(response_text, "RTSP/1.0 200 OK\r\nTest: Test\r\n\r\n");
    }
}
