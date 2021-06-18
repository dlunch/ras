use std::{collections::HashMap, hash::Hash, io, str};

use async_std::net::TcpStream;
use log::warn;
use maplit::hashmap;

use super::{
    request::Request,
    response::{Response, StatusCode},
};

pub struct Session {
    stream: TcpStream,
}

impl Session {
    pub async fn start(stream: TcpStream) -> io::Result<()> {
        let mut session = Self { stream };

        session.run().await
    }

    async fn run(&mut self) -> io::Result<()> {
        loop {
            let req = Request::parse(&mut self.stream).await?;
            if req.is_none() {
                break;
            }
            let req = req.unwrap();

            println!("req {} {:?} {:?}", req.method, req.headers, str::from_utf8(&req.content).unwrap());

            let res = self.handle_request(&req);
            println!("res {} {:?}", res.status as u32, res.headers);

            res.write(&mut self.stream).await?;
        }

        Ok(())
    }

    fn handle_request(&self, request: &Request) -> Response {
        let cseq = request.headers.get("CSeq").unwrap();

        let (status, mut header) = match request.method.as_str() {
            "ANNOUNCE" => (StatusCode::Ok, HashMap::new()),
            "SETUP" => self.setup(request),
            _ => {
                warn!("Unhandled method {}", request.method);

                (StatusCode::Ok, HashMap::new())
            }
        };

        header.insert("CSeq".into(), cseq.into());

        Response::new(status, header)
    }

    fn setup(&self, request: &Request) -> (StatusCode, HashMap<String, String>) {
        // let transport = request.headers.get("Transport").unwrap();

        (StatusCode::Ok, HashMap::new())
    }
}
