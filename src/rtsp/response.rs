use std::collections::HashMap;

#[derive(Clone, Copy)]
pub enum RtspStatusCode {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
    MethodNotAllowed = 405,
    InternalServerError = 500,
}

impl RtspStatusCode {
    pub fn as_string(&self) -> &'static str {
        match self {
            RtspStatusCode::Ok => "OK",
            RtspStatusCode::BadRequest => "Bad Request",
            RtspStatusCode::NotFound => "Not Found",
            RtspStatusCode::MethodNotAllowed => "Method Not Allowed",
            RtspStatusCode::InternalServerError => "Internal Server Error",
        }
    }
}

pub struct RtspResponse {
    pub status: RtspStatusCode,
    pub headers: HashMap<&'static str, String>,
}

impl RtspResponse {
    pub fn new(status: RtspStatusCode) -> Self {
        Self {
            status,
            headers: HashMap::new(),
        }
    }

    pub fn with_headers(status: RtspStatusCode, headers: HashMap<&'static str, String>) -> Self {
        Self { status, headers }
    }
}
