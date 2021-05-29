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

pub struct Service {
    pub r#type: &'static str,
    pub name: &'static str,
    pub port: u16,
    pub txt: Vec<&'static str>,
}

pub async fn serve(service: &Service) -> io::Result<()> {
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let socket = Arc::new(MulticastSocket::all_interfaces(SocketAddrV4::new(mdns_addr, 5353))?);

    loop {
        let socket2 = socket.clone();
        let message = spawn_blocking(move || socket2.receive()).await?;
        debug!("receive from {}, raw {:?}", message.origin_address, message.data);

        let response = handle_packet(&message.data, &service);

        if let Some(response) = response {
            debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
            socket.send(&response, &message.interface)?;
        }
    }
}

fn handle_packet(data: &[u8], service: &Service) -> Option<Vec<u8>> {
    let packet = Packet::parse(&data);
    if packet.header.is_query() {
        for question in &packet.questions {
            debug!("question {}", question.name);

            if question.name.equals(service.r#type) {
                // TODO
            }
        }
    }

    Some(vec![0, 0])
}
