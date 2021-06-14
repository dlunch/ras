mod request;
mod response;
mod session;

use async_std::{
    io,
    net::{IpAddr, SocketAddr, TcpListener},
    stream::StreamExt,
    task,
};

pub async fn serve(ip: IpAddr, port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        task::spawn(async move { session::Session::start(stream).await });
    }

    Ok(())
}
