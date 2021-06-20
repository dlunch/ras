use std::{collections::HashMap, io, str};

use async_std::net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket};
use log::{debug, trace, warn};
use maplit::hashmap;

use super::{
    request::Request,
    response::{Response, StatusCode},
};

pub struct Session {
    id: u32,
    stream: TcpStream,
    rtp: Option<UdpSocket>,
    control: Option<UdpSocket>,
    timing: Option<UdpSocket>,
}

impl Session {
    pub async fn start(id: u32, stream: TcpStream) -> io::Result<()> {
        let mut session = Self {
            id,
            stream,
            rtp: None,
            control: None,
            timing: None,
        };

        session.run().await
    }

    async fn run(&mut self) -> io::Result<()> {
        loop {
            let req = Request::parse(&mut self.stream).await?;
            if req.is_none() {
                break;
            }
            let req = req.unwrap();

            trace!(
                "req {} {} {:?} {:?}",
                req.method,
                req.path,
                req.headers,
                str::from_utf8(&req.content).unwrap()
            );

            let res = self.handle_request(&req).await?;
            trace!("res {} {:?}", res.status as u32, res.headers);

            res.write(&mut self.stream).await?;
        }

        Ok(())
    }

    async fn handle_request(&mut self, request: &Request) -> io::Result<Response> {
        let cseq = request.headers.get("CSeq").unwrap();

        let (status, mut header) = match request.method.as_str() {
            "GET" => (StatusCode::NotFound, HashMap::new()),
            "POST" => (StatusCode::NotFound, HashMap::new()),
            "ANNOUNCE" => (StatusCode::Ok, HashMap::new()),
            "SETUP" => self.setup(request).await?,
            _ => {
                warn!("Unhandled method {}", request.method);

                (StatusCode::Ok, HashMap::new())
            }
        };

        header.insert("CSeq", cseq.into());

        Ok(Response::new(status, header))
    }

    async fn setup(&mut self, request: &Request) -> io::Result<(StatusCode, HashMap<&'static str, String>)> {
        let client_transport = request.headers.get("Transport").unwrap();

        debug!("client_transport: {:?}", client_transport);

        self.rtp = Some(UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?);
        self.control = Some(UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?);
        self.timing = Some(UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?);

        let transport = format!(
            "RTP/AVP/UDP;unicast;mode=record;server_port={};control_port={};timing_port={}",
            self.rtp.as_ref().unwrap().local_addr()?.port(),
            self.control.as_ref().unwrap().local_addr()?.port(),
            self.timing.as_ref().unwrap().local_addr()?.port()
        );

        let response_headers = hashmap! {
            "Session" => self.id.to_string(),
            "Transport" => transport
        };

        Ok((StatusCode::Ok, response_headers))
    }
}
