use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use async_std::{net::UdpSocket, task::spawn_blocking};
use get_if_addrs::{get_if_addrs, Interface};
use log::debug;
use multicast_socket::{all_ipv4_interfaces, Message, MulticastOptions, MulticastSocket};

use super::packet::{Name, Packet, ResourceRecord, ResourceRecordData, ResourceType};
use super::Service;

pub struct Server {
    services: Vec<Service>,
    hostname: String,
    interfaces: Vec<Interface>,
}

impl Server {
    pub fn new(services: Vec<Service>) -> io::Result<Self> {
        let mut hostname = hostname::get()?.into_string().unwrap();
        if !hostname.ends_with(".local") {
            hostname = format!("{}.local", hostname);
        }
        debug!("hostname: {}", hostname);

        let interfaces = get_if_addrs()?;

        Ok(Self {
            services,
            hostname,
            interfaces,
        })
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

            if let Some((unicast_response, multicast_response)) = self.handle_packet(&message) {
                if let Some(unicast_response) = unicast_response {
                    let response = unicast_response.write();

                    debug!("sending response to {:?}, raw {:?}", message.origin_address, response);

                    // MulticastSocket doesn't exposes raw socket to us
                    let response_socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?;
                    response_socket.send_to(&response, message.origin_address).await?;
                }

                if let Some(multicast_response) = multicast_response {
                    let response = multicast_response.write();

                    debug!("sending response to {:?}, raw {:?}", message.origin_address, response);
                    socket.send(&response, &message.interface)?;
                }
            }
        }
    }

    fn handle_packet(&self, message: &Message) -> Option<(Option<Packet>, Option<Packet>)> {
        let packet = Packet::parse(&message.data)?;

        if packet.header.is_query() {
            let mut unicast_response = (Vec::new(), Vec::new());
            let mut multicast_response = (Vec::new(), Vec::new());

            for question in &packet.questions {
                debug!("question {}", question.name);

                for service in &self.services {
                    if question.r#type == ResourceType::PTR && question.name.equals(&service.r#type) {
                        let (mut answers, mut additionals) = self.create_response(service);

                        if question.unicast {
                            unicast_response.0.append(&mut answers);
                            unicast_response.1.append(&mut additionals);
                        } else {
                            multicast_response.0.append(&mut answers);
                            multicast_response.1.append(&mut additionals);
                        }
                    }
                }
            }

            let unicast_response = (!unicast_response.0.is_empty() || !unicast_response.1.is_empty())
                .then(|| Packet::new_response(packet.header.id(), Vec::new(), unicast_response.0, Vec::new(), unicast_response.1));
            let multicast_response = (!multicast_response.0.is_empty() || !multicast_response.1.is_empty())
                .then(|| Packet::new_response(packet.header.id(), Vec::new(), multicast_response.0, Vec::new(), multicast_response.1));

            return Some((unicast_response, multicast_response));
        }

        None
    }

    fn create_response(&self, service: &Service) -> (Vec<ResourceRecord>, Vec<ResourceRecord>) {
        let ip = Ipv4Addr::new(192, 168, 1, 1);

        // PTR answer
        let answers = vec![ResourceRecord::new(
            &service.r#type,
            3600,
            ResourceRecordData::PTR(Name::new(&service.name)),
        )];

        // SRV record
        let mut additionals = vec![ResourceRecord::new(
            &service.name,
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
            additionals.push(ResourceRecord::new(&service.name, 3600, ResourceRecordData::TXT(service.txt.clone())));
        }

        // A record
        additionals.push(ResourceRecord::new(&self.hostname, 3600, ResourceRecordData::A(ip)));

        (answers, additionals)
    }
}
