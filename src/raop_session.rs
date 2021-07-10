use std::{io, str, sync::Arc};

use anyhow::{anyhow, Result};
use async_std::{
    net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket},
    task,
};
use log::{debug, trace, warn};
use maplit::hashmap;
use rtp_rs::RtpReader;
use sdp::session_description::SessionDescription;

use super::{
    decoder::{AppleLoselessDecoder, Decoder, RawPCMDecoder},
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

            let res = self.handle_request(&req).await;
            trace!("res {} {:?}", res.status as u32, res.headers);

            res.write(&mut self.stream).await?;
        }
    }

    async fn handle_request(&mut self, request: &Request) -> Response {
        let cseq = request.headers.get("CSeq");

        let result = match request.method.as_str() {
            "ANNOUNCE" => self.handle_announce(request).await,
            "SETUP" => self.handle_setup(request).await,
            "RECORD" => Ok(Response::new(StatusCode::Ok)),
            "PAUSE" => Ok(Response::new(StatusCode::Ok)),
            "FLUSH" => Ok(Response::new(StatusCode::Ok)),
            "TEARDOWN" => Ok(Response::new(StatusCode::Ok)),
            "OPTIONS" => self.handle_options(request).await,
            "GET_PARAMETER" => Ok(Response::new(StatusCode::Ok)),
            "SET_PARAMETER" => Ok(Response::new(StatusCode::Ok)),
            "POST" => Ok(Response::new(StatusCode::NotFound)),
            "GET" => Ok(Response::new(StatusCode::NotFound)),
            _ => {
                warn!("Unhandled method {}", request.method);

                Ok(Response::new(StatusCode::MethodNotAllowed))
            }
        };

        if let Ok(mut response) = result {
            if let Some(cseq) = cseq {
                response.headers.insert("CSeq", cseq.into());
            }
            response.headers.insert("Server", "ras/0.1".into());

            response
        } else {
            Response::new(StatusCode::InternalServerError)
        }
    }

    async fn handle_options(&mut self, _: &Request) -> Result<Response> {
        Ok(Response::with_headers(
            StatusCode::Ok,
            hashmap! {
                "Public" => "ANNOUNCE, SETUP, RECORD, PAUSE, FLUSH, TEARDOWN, OPTIONS, GET_PARAMETER, SET_PARAMETER, POST, GET".into()
            },
        ))
    }

    async fn handle_announce(&mut self, request: &Request) -> Result<Response> {
        let response = (|| {
            let sdp = SessionDescription::unmarshal(&mut io::Cursor::new(&request.content)).ok()?;

            if sdp.media_descriptions.len() != 1 {
                return None;
            }

            // We can't use Codec structure because its fields are private as of sdp 0.2.1
            // let codec = sdp.get_codec_for_payload_type(96).ok()?;
            // debug!("codec: {:?}", codec);

            let media_description = &sdp.media_descriptions[0];
            let attribute_value = |attr: &str| media_description.attributes.iter().find(|&x| x.key == attr)?.value.as_ref();

            // 96 AppleLossless
            let mut rtpmap_split = attribute_value("rtpmap")?.split_whitespace();

            let (rtp_type, codec) = (rtpmap_split.next()?, rtpmap_split.next()?);
            self.rtp_type = Some(rtp_type.parse().ok()?);

            let codec_parameters = codec.split('/').collect::<Vec<_>>();

            debug!("codec: {:?}", codec);
            match codec_parameters[0] {
                "AppleLossless" => {
                    // 96 352 0 16 40 10 14 2 255 0 0 44100
                    let fmtp_attr = attribute_value("fmtp")?;
                    let fmtp = &fmtp_attr[fmtp_attr.find(char::is_whitespace)? + 1..];

                    debug!("fmtp: {:?}", fmtp);
                    self.decoder = Some(Box::new(AppleLoselessDecoder::new(fmtp).ok()?))
                }
                "L16" => {
                    let rate = codec_parameters[1].parse().ok()?;
                    let channels = codec_parameters[2].parse().ok()?;
                    self.decoder = Some(Box::new(RawPCMDecoder::new(AudioFormat::S16BE, channels, rate).ok()?))
                }
                unk => panic!("Unknown codec {:?}", unk),
            };

            Some(Response::new(StatusCode::Ok))
        })();

        if let Some(response) = response {
            Ok(response)
        } else {
            Ok(Response::new(StatusCode::BadRequest))
        }
    }

    async fn handle_setup(&mut self, request: &Request) -> Result<Response> {
        if let Some(client_transport) = request.headers.get("Transport") {
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

            let rtp_type = self.rtp_type.take().ok_or_else(|| anyhow!("Invalid request"))?;
            let decoder = self.decoder.take().ok_or_else(|| anyhow!("Invalid request"))?;
            let sink = self.sink.clone();
            task::spawn(async move { Self::rtp_loop(rtp, rtp_type, decoder, sink).await });

            Ok(Response::with_headers(StatusCode::Ok, response_headers))
        } else {
            Ok(Response::new(StatusCode::BadRequest))
        }
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
