use std::{collections::HashMap, io, str, sync::Arc};

use anyhow::{anyhow, Result};
use futures::{select, SinkExt, StreamExt};
use log::{debug, trace, warn};
use mac_address::MacAddress;
use maplit::hashmap;
use sdp::SessionDescription;
use tokio::net::{TcpStream, UdpSocket};
use tokio_util::{codec::Framed, udp::UdpFramed};

use super::{
    cipher::{AppleChallenge, RsaAesCipher},
    decoder::{AppleLoselessDecoder, Decoder, RawPCMDecoder},
    rtp::{RtpCodec, RtpControlCodec, RtpControlPacket, RtpPacket},
    rtsp::{RtspCodec, RtspRequest, RtspResponse, RtspStatusCode},
    sink::{AudioFormat, AudioSink, AudioSinkSession},
};

struct StreamInfo {
    rtp_type: u8,
    decoder: Box<dyn Decoder>,
    cipher: Option<RsaAesCipher>,
    session: Box<dyn AudioSinkSession>,
}

pub struct RtspSession {
    id: u32,
    rtp_port: u16,
    control_port: u16,
    timing_port: u16,
    apple_challenge: AppleChallenge,
    sink: Arc<dyn AudioSink>,
    stream_info: Option<StreamInfo>,
}

impl RtspSession {
    pub async fn start(id: u32, rtsp: TcpStream, sink: Arc<dyn AudioSink>, mac_address: MacAddress) -> Result<()> {
        let rtp = UdpSocket::bind("0.0.0.0:0").await?;
        let control = UdpSocket::bind("0.0.0.0:0").await?;
        let timing = UdpSocket::bind("0.0.0.0:0").await?;

        let mut session = Self {
            id,
            rtp_port: rtp.local_addr()?.port(),
            control_port: control.local_addr()?.port(),
            timing_port: timing.local_addr()?.port(),
            apple_challenge: AppleChallenge::new(rtsp.local_addr()?.ip(), &mac_address.bytes()),
            sink,
            stream_info: None,
        };

        session.rtsp_loop(rtsp, rtp, control, timing).await
    }

    async fn rtsp_loop(&mut self, rtsp: TcpStream, rtp: UdpSocket, control: UdpSocket, timing: UdpSocket) -> Result<()> {
        let (mut rtsp_write, rtsp_read) = Framed::new(rtsp, RtspCodec {}).split();
        let mut rtp = UdpFramed::new(rtp, RtpCodec {}).fuse();
        let mut control = UdpFramed::new(control, RtpControlCodec {}).fuse();
        let mut timing = UdpFramed::new(timing, RtpCodec {}).fuse();

        let mut rtsp_read = rtsp_read.fuse();
        loop {
            select! {
                rtsp_packet = rtsp_read.next() => {
                    if rtsp_packet.is_none() {
                        // connection closed
                        return Ok(())
                    }
                    let req = rtsp_packet.unwrap()?;
                    trace!(
                        "req {} {} {:?} {:?}",
                        req.method,
                        req.path,
                        req.headers,
                        str::from_utf8(&req.content).unwrap_or("<Binary>")
                    );

                    let res = self.handle_rtsp(&req).await;
                    trace!("res {} {:?}", res.status as u32, res.headers);

                    rtsp_write.send(res).await?;
                }
                rtp_packet = rtp.next() => self.handle_rtp(rtp_packet.unwrap()?.0).await?,
                control_packet = control.next() => self.handle_control(control_packet.unwrap()?.0).await?,
                timing_packet = timing.next() => self.handle_timing(timing_packet.unwrap()?.0).await?,
            }
        }
    }

    async fn handle_control(&mut self, packet: RtpControlPacket) -> Result<()> {
        trace!(
            "control packet received {} {} {} {}",
            packet.timestamp,
            packet.current_time_seconds,
            packet.current_time_fraction,
            packet.next_timestamp
        );

        Ok(())
    }

    async fn handle_timing(&mut self, packet: RtpPacket) -> Result<()> {
        trace!("timing packet received {} {:?}", packet.payload_type, packet.payload);

        Ok(())
    }

    async fn handle_rtp(&mut self, packet: RtpPacket) -> Result<()> {
        let stream_info = self.stream_info.as_mut().ok_or_else(|| anyhow!("unexpected rtp packet"))?;
        if packet.payload_type != stream_info.rtp_type {
            return Err(anyhow!("Invalid rtp payload type"));
        }

        let payload = if let Some(cipher) = &stream_info.cipher {
            let decrypted = cipher.decrypt(&packet.payload)?;

            stream_info.decoder.decode(&decrypted)?
        } else {
            stream_info.decoder.decode(&packet.payload)?
        };

        stream_info.session.write(&payload)?;

        Ok(())
    }

    async fn handle_rtsp(&mut self, request: &RtspRequest) -> RtspResponse {
        let cseq = request.headers.get("CSeq");
        let apple_challenge = request.headers.get("Apple-Challenge");

        let result = match request.method.as_str() {
            "ANNOUNCE" => self.handle_announce(request).await,
            "SETUP" => self.handle_setup(request).await,
            "RECORD" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "PAUSE" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "FLUSH" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "TEARDOWN" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "OPTIONS" => self.handle_options(request).await,
            "GET_PARAMETER" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "SET_PARAMETER" => Ok(RtspResponse::new(RtspStatusCode::Ok)),
            "POST" => Ok(RtspResponse::new(RtspStatusCode::NotFound)),
            "GET" => Ok(RtspResponse::new(RtspStatusCode::NotFound)),
            _ => {
                warn!("Unhandled method {}", request.method);

                Ok(RtspResponse::new(RtspStatusCode::MethodNotAllowed))
            }
        };

        if let Ok(mut response) = result {
            if let Some(cseq) = cseq {
                response.headers.insert("CSeq", cseq.into());
            }
            if let Some(apple_challenge) = apple_challenge {
                response
                    .headers
                    .insert("Apple-Response", self.apple_challenge.response(apple_challenge).unwrap());
            }
            response.headers.insert("Server", "ras/0.1".into());

            response
        } else {
            RtspResponse::new(RtspStatusCode::InternalServerError)
        }
    }

    async fn handle_options(&mut self, _: &RtspRequest) -> Result<RtspResponse> {
        Ok(RtspResponse::with_headers(
            RtspStatusCode::Ok,
            hashmap! {
                "Public" => "ANNOUNCE, SETUP, RECORD, PAUSE, FLUSH, TEARDOWN, OPTIONS, GET_PARAMETER, SET_PARAMETER, POST, GET".into()
            },
        ))
    }

    async fn handle_announce(&mut self, request: &RtspRequest) -> Result<RtspResponse> {
        let response = (|| {
            let sdp = SessionDescription::unmarshal(&mut io::Cursor::new(&request.content)).ok()?;

            if sdp.media_descriptions.len() != 1 {
                return None;
            }

            let codec = sdp.get_codec_for_payload_type(96).ok()?;
            let media_description = &sdp.media_descriptions[0];

            debug!("codec: {:?}", codec);
            let decoder: Box<dyn Decoder> = match codec.name.as_str() {
                "AppleLossless" => {
                    // we can't use codec.fmtp here because
                    // https://github.com/webrtc-rs/sdp/blob/v0.5.0/src/util/mod.rs#L148 doesn't work if fmtp has whitespaces
                    let fmtp = media_description.attribute("fmtp")??.split_once(' ')?.1;

                    debug!("fmtp: {:?}", fmtp);
                    Box::new(AppleLoselessDecoder::new(fmtp).ok()?)
                }
                "L16" => {
                    let channels = codec.encoding_parameters.parse().ok()?;
                    Box::new(RawPCMDecoder::new(AudioFormat::S16BE, channels, codec.clock_rate).ok()?)
                }
                unk => panic!("Unknown codec {:?}", unk),
            };

            let rsaaeskey = media_description.attribute("rsaaeskey");
            let aesiv = media_description.attribute("aesiv");

            let cipher = if let Some(Some(rsaaeskey)) = rsaaeskey {
                if let Some(Some(aesiv)) = aesiv {
                    let rsaaeskey = base64::decode(rsaaeskey).ok()?;
                    let aesiv = base64::decode(aesiv).ok()?;

                    debug!("key: {:?}, iv: {:?}", rsaaeskey, aesiv);

                    Some(RsaAesCipher::new(&rsaaeskey, &aesiv).ok()?)
                } else {
                    None
                }
            } else {
                None
            };

            let session = self.sink.start(decoder.channels(), decoder.rate(), decoder.format()).unwrap();
            self.stream_info = Some(StreamInfo {
                rtp_type: codec.payload_type,
                decoder,
                cipher,
                session,
            });

            Some(RtspResponse::new(RtspStatusCode::Ok))
        })();

        if let Some(response) = response {
            Ok(response)
        } else {
            Ok(RtspResponse::new(RtspStatusCode::BadRequest))
        }
    }

    async fn handle_setup(&mut self, request: &RtspRequest) -> Result<RtspResponse> {
        if let Some(client_transport) = request.headers.get("Transport") {
            debug!("client_transport: {:?}", client_transport);

            let transports = client_transport
                .split(';')
                .map(|x| x.split('=').collect::<Vec<_>>())
                .map(|x| (x[0], x[1]))
                .collect::<HashMap<_, _>>();

            let client_control_port = transports.get("control_port").unwrap();
            let client_timing_port = transports.get("timing_port").unwrap();

            debug!("client_control_port: {}", client_control_port);
            debug!("client_timing_port: {}", client_timing_port);

            let transport = format!(
                "RTP/AVP/UDP;unicast;mode=record;server_port={};control_port={};timing_port={}",
                self.rtp_port, self.control_port, self.timing_port
            );

            let response_headers = hashmap! {
                "Session" => self.id.to_string(),
                "Transport" => transport
            };

            Ok(RtspResponse::with_headers(RtspStatusCode::Ok, response_headers))
        } else {
            Ok(RtspResponse::new(RtspStatusCode::BadRequest))
        }
    }
}
