mod packet;

use async_std::io;
use async_std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use log::trace;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use packet::Packet;

pub async fn serve(service_type: &str, service_name: &str, service_port: u16, txt: &[&'static str]) -> io::Result<()> {
    let any = Ipv4Addr::new(0, 0, 0, 0);
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let raw_socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    raw_socket.set_reuse_address(true)?;
    raw_socket.bind(&SockAddr::from(SocketAddr::new(IpAddr::V4(any), 5353)))?;
    raw_socket.join_multicast_v4(&mdns_addr, &any)?;

    let socket = UdpSocket::from(std::net::UdpSocket::from(raw_socket.try_clone()?));

    loop {
        let mut buf = [0; 2048];
        let (_, remote_addr) = socket.recv_from(&mut buf).await?;

        let packet = Packet::parse(&buf);

        if packet.header.is_query() {
            for question in &packet.questions {
                trace!("question {}", question.name);

                if question.name.equals(service_type) {
                    // TODO
                }
            }
        }

        // set multicast response interface
        if let IpAddr::V4(x) = remote_addr.ip() {
            raw_socket.set_multicast_if_v4(&x)?;
        }
    }

    Ok(())
}
