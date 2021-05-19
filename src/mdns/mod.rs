mod packet;

use async_std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use async_std::io;

use packet::Packet;


pub async fn serve(service_type: &str, service_name: &str, service_port: u16, txt: &[&'static str]) -> io::Result<()> {
    let any = Ipv4Addr::new(0, 0, 0, 0);
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(any), 5353)).await?;
    socket.join_multicast_v4(mdns_addr, any)?;

    loop {
        let mut buf = [0; 2048];
        socket.recv_from(&mut buf).await?;

        let packet = Packet::parse(&buf);
    }

    Ok(())
}
