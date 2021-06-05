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

use packet::{Name, Packet, ResourceRecord, ResourceRecordData, ResourceType};

pub struct Service {
    pub r#type: &'static str,
    pub name: &'static str,
    pub port: u16,
    pub txt: Vec<&'static str>,
}

pub struct MdnsServer {
    services: Vec<Service>,
    hostname: String,
}

impl MdnsServer {
    pub fn new(services: Vec<Service>) -> io::Result<Self> {
        let hostname = hostname::get()?.into_string().unwrap();
        debug!("hostname: {}", hostname);

        Ok(Self { services, hostname })
    }

    pub async fn serve(&self) -> io::Result<()> {
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

            let response = self.handle_packet(&message.data);

            if let Some(response) = response {
                debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
                socket.send(&response, &message.interface)?;
            }
        }
    }

    fn handle_packet(&self, data: &[u8]) -> Option<Vec<u8>> {
        let packet = Packet::parse(&data)?;
        if packet.header.is_query() {
            for question in &packet.questions {
                debug!("question {}", question.name);

                for service in &self.services {
                    if question.r#type == ResourceType::PTR && question.name.equals(service.r#type) {
                        let (answers, additionals) = self.create_response(service);
                        let response = Packet::new_response(packet.header.id(), Vec::new(), answers, Vec::new(), additionals);

                        return Some(response.write());
                    }
                }
            }
        }

        None
    }

    fn create_response(&self, service: &Service) -> (Vec<ResourceRecord>, Vec<ResourceRecord>) {
        let ip = Ipv4Addr::new(192, 168, 1, 1);

        // PTR answer
        let answers = vec![ResourceRecord::new(
            service.r#type,
            3600,
            ResourceRecordData::PTR(Name::new(service.name)),
        )];

        // SRV record
        let mut additionals = vec![ResourceRecord::new(
            service.name,
            3600,
            ResourceRecordData::SRV {
                priority: 0,
                weight: 0,
                port: service.port,
                target: Name::new(&self.hostname),
            },
        )];

        // TXT record
        if !service.txt.is_empty() {
            additionals.push(ResourceRecord::new(
                service.name,
                3600,
                ResourceRecordData::TXT(service.txt.iter().map(|x| (*x).into()).collect()),
            ));
        }

        // A record
        additionals.push(ResourceRecord::new(&self.hostname, 3600, ResourceRecordData::A(ip)));

        (answers, additionals)
    }
}
