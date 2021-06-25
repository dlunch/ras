mod audio_session;
mod mdns;
mod rtsp;

use std::error::Error;

use async_std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    stream::StreamExt,
    task::spawn,
};
use futures::join;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let audio_join_handle = spawn(async {
        serve_audio(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 7000).await.unwrap();
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

    Ok(())
}

pub async fn serve_audio(ip: IpAddr, port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;

    let mut incoming = listener.incoming();

    let mut id = 1;
    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        spawn(async move { audio_session::AudioSession::start(id, stream).await });
        id += 1;
    }

    Ok(())
}
