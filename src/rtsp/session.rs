use std::{io, str};

use async_std::net::TcpStream;
use maplit::hashmap;

use super::{
    request::Request,
    response::{Response, StatusCode},
};

pub struct Session {
    stream: TcpStream,
}

impl Session {
    pub async fn run(stream: TcpStream) -> io::Result<()> {
        let mut session = Self { stream };

        loop {
            let req = Request::parse(&mut session.stream).await?;
            println!("req {} {:?} {:?}", req.method, req.headers, str::from_utf8(&req.content).unwrap());

            let res = Response::new(StatusCode::Ok, hashmap! {"CSeq".into() => req.headers.get("CSeq").unwrap().into()});
            println!("res {} {:?}", res.status as u32, res.headers);

            res.write(&mut session.stream).await?;
        }
    }
}
