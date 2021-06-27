mod decoder;
mod mdns;
mod raop_session;
mod rtsp;
mod sink;

use std::{future::Future, sync::Arc};

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

    let audio_sink: Arc<Box<dyn sink::AudioSink>> = Arc::new(sink::create_default_audio_sink());

    let raop_join_handle = spawn(async move {
        serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 7000, |id, stream| {
            raop_session::RaopSession::start(id, stream, audio_sink.clone())
        })
        .await
        .unwrap();
    });

    let mdns_join_handle = spawn(async {
        let service = mdns::Service::new(
            "_raop._tcp",
            "test@test",
            7000,
            vec![
                "txtvers=1",
                "ch=2",
                "cn=0,1,2,3",
                "et=0",
                "md=0,1,2",
                "pw=false",
                "sr=44100",
                "ss=16",
                "tp=UDP",
                "vs=220.68",
                "am=AppleTV3,2",
            ],
        );
        let server = mdns::Server::new(vec![service]).unwrap();
        server.serve().await.unwrap();
    });

    join!(raop_join_handle, mdns_join_handle);
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
