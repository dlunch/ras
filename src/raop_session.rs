use std::{collections::HashMap, io, str, sync::Arc};

use async_std::{
    net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket},
    task,
};
use log::{debug, trace, warn};
use maplit::hashmap;
use rtp_rs::RtpReader;

use super::{
    decoder::{AppleLoselessDecoder, Decoder},
    rtsp::{Request, Response, StatusCode},
    sink::{AudioFormat, AudioSink},
};

pub struct RaopSession {
    id: u32,
    stream: TcpStream,
    rtp_type: Option<u8>,
    decoder: Option<Box<dyn Decoder>>,
    sink: Arc<Box<dyn AudioSink>>,
}

impl RaopSession {
    pub async fn start(id: u32, stream: TcpStream, sink: Arc<Box<dyn AudioSink>>) -> io::Result<()> {
        let mut session = Self {
            id,
            stream,
            rtp_type: None,
            decoder: None,
            sink,
        };

        session.rtsp_loop().await
    }

    async fn rtsp_loop(&mut self) -> io::Result<()> {
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
            "ANNOUNCE" => self.announce(request).await?,
            "RECORD" => (StatusCode::Ok, HashMap::new()),
            "SETUP" => self.setup(request).await?,
            _ => {
                warn!("Unhandled method {}", request.method);

                (StatusCode::Ok, HashMap::new())
            }
        };

        header.insert("CSeq", cseq.into());

        Ok(Response::new(status, header))
    }

    async fn announce(&mut self, request: &Request) -> io::Result<(StatusCode, HashMap<&'static str, String>)> {
        let mut codec = None;
        let mut fmtp = None;
        for line in str::from_utf8(&request.content).unwrap().lines() {
            if line.starts_with("a=rtpmap") {
                // a=rtpmap:96 AppleLossless

                let content = &line["a=rtpmap".len() + 1..];
                let mut split = content.split(' ');

                self.rtp_type = Some(split.next().unwrap().parse().unwrap());
                codec = Some(split.next().unwrap());
            } else if line.starts_with("a=fmtp") {
                // a=fmtp:96 352 0 16 40 10 14 2 255 0 0 44100
                fmtp = Some(&line[line.find(' ').unwrap() + 1..]);
            }
        }

        debug!("codec: {:?}, fmtp: {:?}", codec, fmtp);

        match codec.unwrap() {
            "AppleLossless" => self.decoder = Some(Box::new(AppleLoselessDecoder::new(fmtp.unwrap()))),
            unk => panic!("Unknown codec {:?}", unk),
        }

        Ok((StatusCode::Ok, HashMap::new()))
    }

    async fn setup(&mut self, request: &Request) -> io::Result<(StatusCode, HashMap<&'static str, String>)> {
        let client_transport = request.headers.get("Transport").unwrap();

        debug!("client_transport: {:?}", client_transport);

        let rtp = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?;
        let control = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?;
        let timing = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).await?;

        let transport = format!(
            "RTP/AVP/UDP;unicast;mode=record;server_port={};control_port={};timing_port={}",
            rtp.local_addr()?.port(),
            control.local_addr()?.port(),
            timing.local_addr()?.port()
        );

        let response_headers = hashmap! {
            "Session" => self.id.to_string(),
            "Transport" => transport
        };

        let rtp_type = self.rtp_type.take().unwrap();
        let decoder = self.decoder.take().unwrap();
        let sink = self.sink.clone();
        task::spawn(async move { Self::rtp_loop(rtp, rtp_type, decoder, sink).await.unwrap() });

        Ok((StatusCode::Ok, response_headers))
    }

    async fn rtp_loop(socket: UdpSocket, rtp_type: u8, mut decoder: Box<dyn Decoder>, sink: Arc<Box<dyn AudioSink>>) -> io::Result<()> {
        let session = sink.start(decoder.channels(), decoder.rate(), AudioFormat::S32NE);

        loop {
            let mut buf = [0; 2048];
            let len = socket.recv(&mut buf).await?;

            let rtp = RtpReader::new(&buf[..len]).map_err(|x| io::Error::new(io::ErrorKind::Other, format!("{:?}", x)))?;

            if rtp.payload_type() == rtp_type {
                let decoded_content = decoder.decode(rtp.payload());
                session.write(&decoded_content);
            }
        }
    }
}
