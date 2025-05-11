mod cipher;
mod decoder;
mod rtp;
mod rtsp;
mod rtsp_session;
mod sink;
mod util;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use log::{debug, error};
use mac_address::get_mac_address;
use tokio::{
    net::TcpListener,
    task::{spawn, spawn_local},
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

    let local_set = tokio::task::LocalSet::new();

    let args = Args::parse();

    debug!("{:?}", args);

    let mac_address = get_mac_address()?.unwrap();
    debug!("Mac address: {}", mac_address);

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

    let audio_sink = sink::create(&args.audio_sink);
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), args.port);

    let listener = TcpListener::bind(addr).await?;
    let mut incoming = TcpListenerStream::new(listener);

    local_set
        .run_until(async move {
            let mut id = 1;
            while let Some(stream) = incoming.next().await {
                let stream = stream?;

                let audio_session = audio_sink.start()?;
                spawn_local(async move {
                    let result = rtsp_session::RtspSession::start(id, stream, audio_session, mac_address).await;

                    if let Err(err) = result {
                        error!("{:?}", err);
                    }
                });

                id += 1;
            }

            mdns_join_handle.await??;

            anyhow::Ok(())
        })
        .await?;

    Ok(())
}
