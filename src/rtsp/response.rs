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
}

impl Response {
    pub fn new(status: StatusCode) -> Self {
        Self { status }
    }

    pub async fn write<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: io::Write + Unpin,
    {
        let status_line = format!(
            "RTSP/1.0 {} {}",
            self.status as usize,
            self.status.as_string()
        );

        stream.write(status_line.as_bytes()).await?;

        Ok(())
    }
}
