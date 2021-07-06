mod decoder;
mod mdns;
mod raop_session;
mod rtsp;
mod sink;
mod util;

use std::{future::Future, sync::Arc};

use anyhow::Result;
use async_std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    stream::StreamExt,
    task::spawn,
};
use clap::{App, Arg};
use futures::future::try_join_all;
use log::debug;
use mac_address::get_mac_address;

#[async_std::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let matches = App::new("ras")
        .arg(Arg::with_name("server_name").long("server_name").default_value("ras"))
        .arg(Arg::with_name("audio_sink").long("audio_sink").default_value("rodio").possible_values(&[
            "rodio",
            #[cfg(all(unix, not(target_os = "macos")))]
            "pulseaudio",
            "dummy",
        ]))
        .get_matches();

    let server_name = matches.value_of("server_name").unwrap().to_owned();
    let audio_sink = matches.value_of("audio_sink").unwrap();

    debug!("{:?}", matches);

    let mac_address = get_mac_address()?.unwrap().to_string();
    debug!("Mac address: {}", mac_address);

    let audio_sink: Arc<Box<dyn sink::AudioSink>> = Arc::new(sink::create(audio_sink));

    let raop_join_handle = spawn(async move {
        serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 7000, |id, stream| {
            raop_session::RaopSession::start(id, stream, audio_sink.clone())
        })
        .await
    });

    let mdns_join_handle = spawn(async move {
        let service = mdns::Service::new(
            "_raop._tcp",
            &format!("{}@{}", mac_address.replace(":", ""), server_name),
            7000,
            vec![
                "sf=0x4",
                "fv=76400.10",
                "am=ras",
                "vs=105.1",
                "tp=UDP",
                "vn=65537",
                "md=0,1,2",
                "ss=16",
                "sr=44100",
                "da=true",
                "sv=false",
                "et=0,1",
                "ek=1",
                "cn=0,1",
                "ch=2",
                "txtvers=1",
                "pw=false",
            ],
        );
        let server = mdns::Server::new(vec![service])?;
        server.serve().await
    });

    try_join_all([raop_join_handle, mdns_join_handle]).await?;

    Ok(())
}

pub async fn serve<F>(ip: IpAddr, port: u16, handler: impl Fn(u32, TcpStream) -> F) -> Result<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
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
