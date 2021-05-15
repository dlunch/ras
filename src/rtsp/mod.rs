use async_std::io::{self, prelude::BufReadExt, BufReader};
use async_std::net::{IpAddr, SocketAddr, TcpListener};
use async_std::stream::StreamExt;

pub async fn serve(ip: IpAddr, port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let reader = BufReader::new(stream);

        let mut lines = reader.lines();

        while let Some(line) = lines.next().await {
            println!("{}", line?);
        }
    }

    Ok(())
}
