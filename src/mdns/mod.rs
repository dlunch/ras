mod packet;

use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use async_std::task::spawn_blocking;
use log::debug;
use multicast_socket::{all_ipv4_interfaces, MulticastOptions, MulticastSocket};

use packet::Packet;

pub struct Service {
    pub r#type: &'static str,
    pub name: &'static str,
    pub port: u16,
    pub txt: Vec<&'static str>,
}

pub async fn serve(service: &Service) -> io::Result<()> {
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);

    let interfaces = all_ipv4_interfaces()?;
    let socket = Arc::new(MulticastSocket::with_options(
        SocketAddrV4::new(mdns_addr, 5353),
        interfaces,
        MulticastOptions {
            read_timeout: Duration::from_secs(60), // MulticastSocket doesn't let us to use infinite timeout here
            loopback: false,
            buffer_size: 2048,
            bind_address: Ipv4Addr::UNSPECIFIED,
        },
    )?);

    loop {
        let socket2 = socket.clone();
        let message = spawn_blocking(move || loop {
            let result = socket2.receive();
            if let Err(x) = &result {
                if x.kind() == io::ErrorKind::TimedOut {
                    continue;
                }
            }
            return result;
        })
        .await?;
        debug!("receive from {}, raw {:?}", message.origin_address, message.data);

        let response = handle_packet(&message.data, &service);

        if let Some(response) = response {
            debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
            socket.send(&response, &message.interface)?;
        }
    }
}

fn handle_packet(data: &[u8], service: &Service) -> Option<Vec<u8>> {
    let packet = Packet::parse(&data)?;
    if packet.header.is_query() {
        for question in &packet.questions {
            debug!("question {}", question.name);

            if question.name.equals(service.r#type) {
                let response = create_response(packet.header.id(), service);

                let mut buf = vec![0; 2048];
                let len = response.write(&mut buf);
                buf.resize(len, 0);

                return Some(buf);
            }
        }
    }

    None
}

fn create_response(id: u16, service: &Service) -> Packet {
    // WIP

    Packet::new_response(id)
}
