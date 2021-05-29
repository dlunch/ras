mod packet;

use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

use async_std::task::spawn_blocking;
use log::debug;
use multicast_socket::MulticastSocket;

use packet::Packet;

pub async fn serve(service_type: &str, service_name: &str, service_port: u16, txt: &[&'static str]) -> io::Result<()> {
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let socket = Arc::new(MulticastSocket::all_interfaces(SocketAddrV4::new(mdns_addr, 5353))?);

    loop {
        let socket2 = socket.clone();
        let message = spawn_blocking(move || socket2.receive()).await?;
        debug!("receive from {}, raw {:?}", message.origin_address, message.data);

        let packet = Packet::parse(&message.data);
        if packet.header.is_query() {
            for question in &packet.questions {
                debug!("question {}", question.name);

                if question.name.equals(service_type) {
                    // TODO
                }
            }
        }

        let response = vec![0, 0];

        debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
        socket.send(&response, &message.interface)?;
    }
}
