use std::collections::HashMap;

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
}
