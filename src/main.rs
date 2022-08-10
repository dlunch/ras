mod apple_challenge;
mod decoder;
mod key;
mod raop_session;
mod rtp;
mod rtsp;
mod sink;
mod util;

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use anyhow::Result;
use clap::Parser;
use futures::{future::try_join_all, FutureExt, StreamExt};
use log::{debug, error};
use mac_address::get_mac_address;
use tokio::{
    net::{TcpListener, TcpStream},
    task::spawn,
};
use tokio_stream::wrappers::TcpListenerStream;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long, default_value = "ras")]
    server_name: String,
    #[clap(long, default_value = "rodio")]
    audio_sink: String,
    #[clap(long, default_value_t = 7000)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    debug!("{:?}", args);

    let mac_address = get_mac_address()?.unwrap();
    debug!("Mac address: {}", mac_address);

    let audio_sink = sink::create(&args.audio_sink);

    let raop_join_handle = spawn(async move {
        let result = serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), args.port, |id, stream| {
            raop_session::RaopSession::start(id, stream, audio_sink.clone(), mac_address).map(|x| {
                if let Err(err) = x {
                    error!("{:?}", err);
                }
            })
        })
        .await;

        if let Err(e) = &result {
            error!("{}", e);
        }

        result
    });

    let mdns_join_handle = spawn(async move {
        let service = simple_mdns::Service::new(
            "_raop._tcp",
            &format!("{}@{}", mac_address.to_string().replace(':', ""), args.server_name),
            args.port,
            vec![
                "txtvers=1", // always 1
                "md=0,1,2",  // metadata type
                "ss=16",     // sample size
                "sr=44100",  // sample rate
                "ch=2",      // channels
                "et=0,1",    // encryption type
                "cn=0,1",    // codec type
                "pw=false",  // has password?
                "tp=UDP",    // transport protocol
                "vn=65537",  // required, unknown
            ],
        );
        let server = simple_mdns::Server::new(vec![service]).unwrap();
        server.serve().await
    });

    try_join_all([raop_join_handle, mdns_join_handle]).await?;

    Ok(())
}

pub async fn serve<F>(ip: IpAddr, port: u16, handler: impl Fn(u32, TcpStream) -> F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(SocketAddr::new(ip, port)).await?;
    let mut incoming = TcpListenerStream::new(listener);

    let mut id = 1;
    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        spawn(handler(id, stream));
        id += 1;
    }

    Ok(())
}
