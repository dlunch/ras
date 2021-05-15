mod request;

use async_std::io;
use async_std::net::{IpAddr, SocketAddr, TcpListener};
use async_std::stream::StreamExt;

use request::Request;

pub async fn serve(ip: IpAddr, port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let mut stream = stream?;

        let req = Request::parse(&mut stream).await?;

        println!("{} {:?} {:?}", req.method, req.headers, req.content);
    }

    Ok(())
}
