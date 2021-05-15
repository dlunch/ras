mod request;
mod response;

use std::str;

use async_std::io;
use async_std::net::{IpAddr, SocketAddr, TcpListener};
use async_std::stream::StreamExt;
use maplit::hashmap;

use request::Request;
use response::{Response, StatusCode};

pub async fn serve(ip: IpAddr, port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let mut stream = stream?;

        loop {
            let req = Request::parse(&mut stream).await?;

            println!(
                "req {} {:?} {:?}",
                req.method,
                req.headers,
                str::from_utf8(&req.content).unwrap()
            );

            let res = Response::new(
                StatusCode::Ok,
                hashmap! {"CSeq".into() => req.headers.get("CSeq").unwrap().into()},
            );

            res.write(&mut stream).await?;
        }
    }

    Ok(())
}
