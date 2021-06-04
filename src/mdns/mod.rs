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

use packet::{Name, Packet, ResourceRecord, ResourceRecordData};

pub struct Service {
    pub r#type: &'static str,
    pub name: &'static str,
    pub port: u16,
    pub txt: Vec<&'static str>,
}

pub async fn serve(service: &Service) -> io::Result<()> {
    let mdns_addr = Ipv4Addr::new(224, 0, 0, 251);
    let hostname = hostname::get()?.into_string().unwrap();
    debug!("hostname: {}", hostname);

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

        let response = handle_packet(&message.data, &service, &hostname);

        if let Some(response) = response {
            debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
            socket.send(&response, &message.interface)?;
        }
    }
}

fn handle_packet(data: &[u8], service: &Service, hostname: &str) -> Option<Vec<u8>> {
    let packet = Packet::parse(&data)?;
    if packet.header.is_query() {
        for question in &packet.questions {
            debug!("question {}", question.name);

            if question.name.equals(service.r#type) {
                let response = create_response(packet.header.id(), service, hostname);

                return Some(response.write());
            }
        }
    }

    None
}

fn create_response(id: u16, service: &Service, hostname: &str) -> Packet {
    let ip = Ipv4Addr::new(192, 168, 1, 1);

    // PTR answer
    let answer = ResourceRecord::new(service.r#type, 3600, ResourceRecordData::PTR(Name::new(service.name)));

    // SRV record
    let srv = ResourceRecord::new(
        service.name,
        3600,
        ResourceRecordData::SRV {
            priority: 0,
            weight: 0,
            port: service.port,
            target: Name::new(hostname),
        },
    );

    // TXT record
    let txt = ResourceRecord::new(
        service.name,
        3600,
        ResourceRecordData::TXT(service.txt.iter().map(|x| (*x).into()).collect()),
    );

    // A RECORD
    let a = ResourceRecord::new(hostname, 3600, ResourceRecordData::A(ip));

    Packet::new_response(id, Vec::new(), vec![answer], Vec::new(), vec![srv, txt, a])
}
