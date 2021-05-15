mod mdns;
mod rtsp;

use std::error::Error;

use async_std::net::{IpAddr, Ipv4Addr};
use async_std::task::spawn;
use futures::join;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let rtsp_join_handle = spawn(async {
        rtsp::serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5000)
            .await
            .unwrap();
    });

    let mdns_join_handle = spawn(async {
        mdns::serve("_raop._tcp").await.unwrap();
    });

    join!(rtsp_join_handle, mdns_join_handle);

    Ok(())
}
