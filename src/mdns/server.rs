use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Result};
use async_std::{net::UdpSocket, task::spawn_blocking};
use cidr_utils::cidr::Ipv4Cidr;
use log::{debug, trace};
use multicast_socket::{all_ipv4_interfaces, Message, MulticastOptions, MulticastSocket};

use super::packet::{Name, Packet, ResourceRecord, ResourceRecordData, ResourceType};
use super::Service;

pub struct Server {
    services: Vec<Service>,
    hostname: String,
    // consider only ipv4 for now
    prefixes: Vec<(Ipv4Addr, Ipv4Cidr)>,
}

impl Server {
    pub fn new(services: Vec<Service>) -> Result<Self> {
        let mut hostname = hostname::get()?.into_string().unwrap();
        if !hostname.ends_with(".local") {
            hostname = format!("{}.local", hostname);
        }
        debug!("hostname: {}", hostname);

        #[cfg(windows)]
        let prefixes = ipconfig::get_adapters()?
            .into_iter()
            .flat_map(|adapter| {
                adapter
                    .prefixes()
                    .iter()
                    .filter_map(|prefix| {
                        use std::net::IpAddr;

                        // returns ipv4 {adapter_address}/{mask} cidr
                        if let IpAddr::V4(prefix_addr) = prefix.0 {
                            let cidr = Ipv4Cidr::from_prefix_and_bits(prefix_addr, prefix.1 as u8).ok()?;
                            for address in adapter.ip_addresses() {
                                if let IpAddr::V4(adapter_addr) = address {
                                    if cidr.contains(adapter_addr) && prefix.1 != 32 && prefix.1 != 0 {
                                        return Some((*adapter_addr, Ipv4Cidr::from_prefix_and_bits(prefix_addr, prefix.1 as u8).ok()?));
                                    }
                                }
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        #[cfg(unix)]
        let prefixes = get_if_addrs::get_if_addrs()?
            .into_iter()
            .filter_map(|if_addr| {
                use get_if_addrs::IfAddr;
                if let IfAddr::V4(addr) = if_addr.addr {
                    if addr.netmask != Ipv4Addr::new(0, 0, 0, 0) && addr.netmask != Ipv4Addr::new(255, 255, 255, 255) {
                        return Some((addr.ip, Ipv4Cidr::from_prefix_and_mask(addr.ip, addr.netmask).ok()?));
                    }
                }

                None
            })
            .collect::<Vec<_>>();

        for prefix in &prefixes {
            debug!("ip {:?}/{}", prefix.0, prefix.1.get_bits());
        }

        Ok(Self {
            services,
            hostname,
            prefixes,
        })
    }

    pub async fn serve(&self) -> Result<()> {
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
                    // TimedOut: windows, Other: other
                    if x.kind() == io::ErrorKind::TimedOut || x.kind() == io::ErrorKind::Other {
                        continue;
                    }
                }
                return result;
            })
            .await?;
            trace!("receive from {}, raw {:?}", message.origin_address, message.data);

            if let Some((unicast_response, multicast_response)) = self.handle_packet(&message) {
                if let Some(unicast_response) = unicast_response {
                    let response = unicast_response.write();

                    trace!("sending response to {:?}, raw {:?}", message.origin_address, response);

                    // MulticastSocket doesn't exposes raw socket to us
                    let response_socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?;
                    response_socket.send_to(&response, message.origin_address).await?;
                }

                if let Some(multicast_response) = multicast_response {
                    let response = multicast_response.write();

                    trace!("sending response to {:?}, raw {:?}", message.origin_address, response);
                    socket.send(&response, &message.interface)?;
                }
            }
        }
    }

    fn handle_packet(&self, message: &Message) -> Option<(Option<Packet>, Option<Packet>)> {
        let packet = Packet::parse(&message.data).ok()?;

        if packet.header.is_query() {
            let mut unicast_response = (Vec::new(), Vec::new());
            let mut multicast_response = (Vec::new(), Vec::new());

            for question in &packet.questions {
                for service in &self.services {
                    if question.r#type == ResourceType::PTR && question.name.equals(&service.r#type) {
                        let (mut answers, mut additionals) = self.create_response(service, message.origin_address.ip()).ok()?;

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

    fn create_response(&self, service: &Service, remote_addr: &Ipv4Addr) -> Result<(Vec<ResourceRecord>, Vec<ResourceRecord>)> {
        debug!("Creating response for {}", service.name);

        let ip = self.find_local_ip(remote_addr).ok_or_else(|| anyhow!("Can't find local ip address"))?;

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

        Ok((answers, additionals))
    }

    fn find_local_ip(&self, remote_addr: &Ipv4Addr) -> Option<Ipv4Addr> {
        for prefix in &self.prefixes {
            if prefix.1.contains(remote_addr) {
                debug!("remote_addr: {:?}, interface ip: {:?}/{}", remote_addr, prefix.0, prefix.1.get_bits());
                return Some(prefix.0);
            }
        }

        None
    }
}
