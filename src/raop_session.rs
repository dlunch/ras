use std::{collections::HashMap, str, sync::Arc};

use anyhow::Result;
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
    sink::AudioSink,
};

pub struct RaopSession {
    id: u32,
    stream: TcpStream,
    rtp_type: Option<u8>,
    decoder: Option<Box<dyn Decoder>>,
    sink: Arc<Box<dyn AudioSink>>,
}

impl RaopSession {
    pub async fn start(id: u32, stream: TcpStream, sink: Arc<Box<dyn AudioSink>>) -> Result<()> {
        let mut session = Self {
            id,
            stream,
            rtp_type: None,
            decoder: None,
            sink,
        };

        session.rtsp_loop().await
    }

    async fn rtsp_loop(&mut self) -> Result<()> {
        loop {
            let req = Request::parse(&mut self.stream).await?;
            trace!("req {} {} {:?} {:?}", req.method, req.path, req.headers, str::from_utf8(&req.content)?);

            let res = self.handle_request(&req).await?;
            trace!("res {} {:?}", res.status as u32, res.headers);

            res.write(&mut self.stream).await?;
        }
    }

    async fn handle_request(&mut self, request: &Request) -> Result<Response> {
        let cseq = request.headers.get("CSeq").unwrap();

        let (status, mut header) = match request.method.as_str() {
            "ANNOUNCE" => self.handle_announce(request).await?,
            "SETUP" => self.handle_setup(request).await?,
            "RECORD" => (StatusCode::Ok, HashMap::new()),
            "PAUSE" => (StatusCode::Ok, HashMap::new()),
            "FLUSH" => (StatusCode::Ok, HashMap::new()),
            "TEARDOWN" => (StatusCode::Ok, HashMap::new()),
            "OPTIONS" => self.handle_options(request).await?,
            "GET_PARAMETER" => (StatusCode::Ok, HashMap::new()),
            "SET_PARAMETER" => (StatusCode::Ok, HashMap::new()),
            "POST" => (StatusCode::NotFound, HashMap::new()),
            "GET" => (StatusCode::NotFound, HashMap::new()),
            _ => {
                warn!("Unhandled method {}", request.method);

                (StatusCode::MethodNotAllowed, HashMap::new())
            }
        };

        header.insert("CSeq", cseq.into());
        header.insert("Server", "ras/0.1".into());

        Ok(Response::new(status, header))
    }

    async fn handle_options(&mut self, _: &Request) -> Result<(StatusCode, HashMap<&'static str, String>)> {
        Ok((
            StatusCode::Ok,
            hashmap! {
                "Public" => "ANNOUNCE, SETUP, RECORD, PAUSE, FLUSH, TEARDOWN, OPTIONS, GET_PARAMETER, SET_PARAMETER, POST, GET".into()
            },
        ))
    }

    async fn handle_announce(&mut self, request: &Request) -> Result<(StatusCode, HashMap<&'static str, String>)> {
        let mut codec = None;
        let mut fmtp = None;
        for line in str::from_utf8(&request.content)?.lines() {
            if line.starts_with("a=rtpmap") {
                // a=rtpmap:96 AppleLossless

                let content = &line["a=rtpmap".len() + 1..];
                let mut split = content.split(' ');

                self.rtp_type = Some(split.next().unwrap().parse()?);
                codec = Some(split.next().unwrap());
            } else if line.starts_with("a=fmtp") {
                // a=fmtp:96 352 0 16 40 10 14 2 255 0 0 44100
                fmtp = Some(&line[line.find(' ').unwrap() + 1..]);
            }
        }

        debug!("codec: {:?}, fmtp: {:?}", codec, fmtp);

        match codec.unwrap() {
            "AppleLossless" => self.decoder = Some(Box::new(AppleLoselessDecoder::new(fmtp.unwrap())?)),
            unk => panic!("Unknown codec {:?}", unk),
        }

        Ok((StatusCode::Ok, HashMap::new()))
    }

    async fn handle_setup(&mut self, request: &Request) -> Result<(StatusCode, HashMap<&'static str, String>)> {
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
        task::spawn(async move { Self::rtp_loop(rtp, rtp_type, decoder, sink).await });

        Ok((StatusCode::Ok, response_headers))
    }

    async fn rtp_loop(socket: UdpSocket, rtp_type: u8, mut decoder: Box<dyn Decoder>, sink: Arc<Box<dyn AudioSink>>) -> Result<()> {
        let session = sink.start(decoder.channels(), decoder.rate(), decoder.format())?;

        loop {
            let mut buf = [0; 2048];
            let len = socket.recv(&mut buf).await?;

            let rtp = RtpReader::new(&buf[..len]).map_err(|x| anyhow::Error::msg(format!("Can't read rtp packet {:?}", x)))?;

            if rtp.payload_type() == rtp_type {
                let decoded_content = decoder.decode(rtp.payload())?;
                session.write(&decoded_content)?;
            }
        }
    }
}
