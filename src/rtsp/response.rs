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
    status: StatusCode,
    headers: HashMap<String, String>,
}

impl Response {
    pub fn new(status: StatusCode, headers: HashMap<String, String>) -> Self {
        Self { status, headers }
    }

    pub async fn write<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: io::Write + Unpin,
    {
        let status_line = format!(
            "RTSP/1.0 {} {}\r\n",
            self.status as usize,
            self.status.as_string()
        );

        stream.write(status_line.as_bytes()).await?;

        for (key, value) in &self.headers {
            let header_line = format!("{}: {}\r\n", key, value);

            stream.write(header_line.as_bytes()).await?;
        }

        stream.write("\r\n".as_bytes());

        Ok(())
    }
}
