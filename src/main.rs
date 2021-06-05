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
        rtsp::serve(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5000).await.unwrap();
    });

    let mdns_join_handle = spawn(async {
        let service = mdns::Service {
            r#type: "_raop._tcp.local",
            name: "tcp",
            port: 7000,
            txt: vec![
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
        };

        let server = mdns::MdnsServer::new(vec![service]).unwrap();
        server.serve().await.unwrap();
    });

    join!(rtsp_join_handle, mdns_join_handle);

    Ok(())
}
