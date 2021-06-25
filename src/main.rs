mod audio_session;
mod mdns;
mod rtsp;

use std::future::Future;

use async_std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    stream::StreamExt,
    task::spawn,
};
use futures::join;

#[async_std::main]
async fn main() {
    pretty_env_logger::init();

    let audio_join_handle = spawn(async {
        serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 7000, audio_session::AudioSession::start)
            .await
            .unwrap();
    });

    let mdns_join_handle = spawn(async {
        let service = mdns::Service::new(
            "_raop._tcp",
            "test@test",
            7000,
            vec![
                "am=AppleTV3,2",
                "cn=0,1,2,3",
                "da=true",
                "et=0,3,5",
                "md=0,1,2",
                "sf=0x4",
                "tp=UDP",
                "vn=65537",
                "vs=220.68",
                "vv=2",
            ],
        );
        let server = mdns::Server::new(vec![service]).unwrap();
        server.serve().await.unwrap();
    });

    join!(audio_join_handle, mdns_join_handle);
}

pub async fn serve<F>(ip: IpAddr, port: u16, handler: impl Fn(u32, TcpStream) -> F) -> io::Result<()>
where
    F: Future<Output = io::Result<()>> + Send + 'static,
{
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;
    let mut incoming = listener.incoming();

    let mut id = 1;
    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        spawn(handler(id, stream));
        id += 1;
    }

    Ok(())
}
