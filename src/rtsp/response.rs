use std::collections::HashMap;

use tokio::io::{self, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Copy)]
pub enum StatusCode {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
    MethodNotAllowed = 405,
    InternalServerError = 500,
}

impl StatusCode {
    pub fn as_string(&self) -> &'static str {
        match self {
            StatusCode::Ok => "OK",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::InternalServerError => "Internal Server Error",
        }
    }
}

pub struct Response {
    pub status: StatusCode,
    pub headers: HashMap<&'static str, String>,
}

impl Response {
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: HashMap::new(),
        }
    }

    pub fn with_headers(status: StatusCode, headers: HashMap<&'static str, String>) -> Self {
        Self { status, headers }
    }

    pub async fn write<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
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
    use anyhow::Result;
    use maplit::hashmap;
    use std::str;

    #[tokio::test]
    async fn test_simple_response() -> Result<()> {
        let response = Response::with_headers(StatusCode::Ok, hashmap! { "Test" => "Test".into() });

        let mut buf = Vec::new();
        response.write(&mut buf).await?;

        let response_text = str::from_utf8(&buf)?;
        assert_eq!(response_text, "RTSP/1.0 200 OK\r\nTest: Test\r\n\r\n");

        Ok(())
    }
}
