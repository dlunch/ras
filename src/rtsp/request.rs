use std::collections::HashMap;

pub struct RtspRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub content: Vec<u8>,
}
