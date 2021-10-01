use std::{
    convert::TryInto,
    fmt,
    mem::size_of,
    net::{Ipv4Addr, Ipv6Addr},
    str,
};

use anyhow::{anyhow, Result};
use bitflags::bitflags;
use log::trace;

struct ReadStream<'a> {
    buffer: &'a [u8],
    cursor: usize,
}

impl<'a> ReadStream<'a> {
    fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    fn with_cursor(buffer: &'a [u8], cursor: usize) -> Self {
        Self { buffer, cursor }
    }

    fn read(&mut self, length: usize) -> &[u8] {
        let result = &self.buffer[self.cursor..self.cursor + length];
        self.cursor += length;

        result
    }

    fn read_as<T>(&mut self) -> &T {
        unsafe { &*(self.read(size_of::<T>()).as_ptr() as *const T) }
    }

    fn read_u8(&mut self) -> u8 {
        let result = u8::from_be_bytes(self.buffer[self.cursor..self.cursor + size_of::<u8>()].try_into().unwrap());
        self.cursor += size_of::<u8>();

        result
    }

    fn read_u16(&mut self) -> u16 {
        let result = u16::from_be_bytes(self.buffer[self.cursor..self.cursor + size_of::<u16>()].try_into().unwrap());
        self.cursor += size_of::<u16>();

        result
    }

    fn read_u32(&mut self) -> u32 {
        let result = u32::from_be_bytes(self.buffer[self.cursor..self.cursor + size_of::<u32>()].try_into().unwrap());
        self.cursor += size_of::<u32>();

        result
    }

    fn read_u128(&mut self) -> u128 {
        let result = u128::from_be_bytes(self.buffer[self.cursor..self.cursor + size_of::<u128>()].try_into().unwrap());
        self.cursor += size_of::<u128>();

        result
    }
}

struct WriteStream {
    buffer: Vec<u8>,
}

impl WriteStream {
    fn new(buffer_capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(buffer_capacity),
        }
    }

    fn write(&mut self, data: &[u8]) {
        self.buffer.extend(data);
    }

    fn write_from<T>(&mut self, data: &T) {
        self.write(unsafe { std::slice::from_raw_parts((data as *const T) as *const u8, size_of::<T>()) })
    }

    fn write_u8(&mut self, data: u8) {
        self.write(&data.to_be_bytes())
    }

    fn write_u16(&mut self, data: u16) {
        self.write(&data.to_be_bytes())
    }

    fn write_u32(&mut self, data: u32) {
        self.write(&data.to_be_bytes())
    }

    fn write_u128(&mut self, data: u128) {
        self.write(&data.to_be_bytes())
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct U16be {
    raw: [u8; 2],
}

impl U16be {
    pub fn new(value: u16) -> Self {
        Self { raw: value.to_be_bytes() }
    }

    pub fn get(&self) -> u16 {
        u16::from_be_bytes(self.raw)
    }
}

bitflags! {
    struct HeaderFlags: u16 { // in big endian form
        const RESPONSE = 0b0000_0000_1000_0000;
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct Header {
    id: U16be,
    flags: HeaderFlags,
    qd_count: U16be,
    an_count: U16be,
    ns_count: U16be,
    ar_count: U16be,
}

impl Header {
    pub fn is_query(&self) -> bool {
        !self.flags.contains(HeaderFlags::RESPONSE)
    }

    pub fn id(&self) -> u16 {
        self.id.get()
    }
}

pub struct Name {
    labels: Vec<String>,
}

impl Name {
    pub fn new(name: &str) -> Self {
        Self {
            labels: name.split('.').map(|x| x.into()).collect(),
        }
    }

    fn parse(stream: &mut ReadStream) -> Result<Self> {
        let mut labels = Vec::new();
        loop {
            let length = stream.read_u8() as usize;
            if length == 0 {
                break;
            }
            if length & 192 == 192 {
                let offset_byte = stream.read_u8() as usize;
                let offset = (length << 8 | offset_byte) & !49152;

                let mut new_stream = ReadStream::with_cursor(stream.buffer, offset);
                let mut result = Name::parse(&mut new_stream)?;

                labels.append(&mut result.labels);

                break;
            } else {
                let label = stream.read(length as usize);
                labels.push(str::from_utf8(label)?.into());
            }
        }

        Ok(Self { labels })
    }

    fn write(&self, stream: &mut WriteStream) {
        for label in &self.labels {
            let bytes = label.as_bytes();
            stream.write_u8(bytes.len() as u8);
            stream.write(bytes);
        }

        stream.write_u8(0);
    }

    pub fn equals(&self, other: &str) -> bool {
        let split = other.split('.').collect::<Vec<_>>();

        self.labels == split
    }
}

impl fmt::Display for Name {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(&self.labels.join("."))
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ResourceType {
    A,
    PTR,
    TXT,
    AAAA,
    SRV,
    Unknown(u16),
}

impl ResourceType {
    fn parse(raw: u16) -> Self {
        match raw {
            1 => Self::A,
            12 => Self::PTR,
            16 => Self::TXT,
            28 => Self::AAAA,
            33 => Self::SRV,
            x => {
                trace!("Unknown resourcetype {}", x);

                Self::Unknown(x)
            }
        }
    }

    fn write(&self, stream: &mut WriteStream) {
        match self {
            Self::A => stream.write_u16(1),
            Self::PTR => stream.write_u16(12),
            Self::TXT => stream.write_u16(16),
            Self::AAAA => stream.write_u16(28),
            Self::SRV => stream.write_u16(33),
            Self::Unknown(x) => panic!("Cannot write unknown resourcetype {}", x),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Class {
    IN,
    Unknown(u16),
}

impl Class {
    fn parse(raw: u16) -> Self {
        match raw & 0x7fff {
            1 => Self::IN,
            x => Self::Unknown(x),
        }
    }

    fn write(&self, stream: &mut WriteStream) {
        match self {
            Self::IN => stream.write_u16(0x8001), // with cache flush bit
            Self::Unknown(x) => panic!("Cannot write unknown class {}", x),
        }
    }
}

pub struct Question {
    pub name: Name,
    pub r#type: ResourceType,
    class: Class,
    pub unicast: bool,
}

impl Question {
    fn parse(stream: &mut ReadStream) -> Result<Self> {
        let name = Name::parse(stream)?;

        let r#type = stream.read_u16();
        let class = stream.read_u16();

        let unicast = class & 0x8000 != 0;

        Ok(Question {
            name,
            r#type: ResourceType::parse(r#type),
            class: Class::parse(class),
            unicast,
        })
    }

    fn write(&self, stream: &mut WriteStream) {
        self.name.write(stream);

        self.r#type.write(stream);
        self.class.write(stream);
    }
}

#[allow(clippy::upper_case_acronyms)]
pub enum ResourceRecordData {
    A(Ipv4Addr),
    AAAA(Ipv6Addr),
    PTR(Name),
    TXT(Vec<String>),
    SRV { priority: u16, weight: u16, port: u16, target: Name },
    Unknown { r#type: ResourceType, data: Vec<u8> },
}

impl ResourceRecordData {
    fn parse(r#type: ResourceType, stream: &mut ReadStream) -> Result<Self> {
        let length = stream.read_u16() as usize;

        Ok(match r#type {
            ResourceType::A => Self::A(Ipv4Addr::from(stream.read_u32())),
            ResourceType::AAAA => Self::AAAA(Ipv6Addr::from(stream.read_u128())),
            ResourceType::PTR => Self::PTR(Name::parse(stream)?),
            ResourceType::TXT => {
                let mut txt = Vec::new();
                let end = stream.cursor + length;
                loop {
                    let length = stream.read_u8() as usize;
                    txt.push(str::from_utf8(stream.read(length))?.into());

                    if stream.cursor == end {
                        break;
                    }
                }

                Self::TXT(txt)
            }
            ResourceType::SRV => Self::SRV {
                priority: stream.read_u16(),
                weight: stream.read_u16(),
                port: stream.read_u16(),
                target: Name::parse(stream)?,
            },
            x => Self::Unknown {
                r#type: x,
                data: stream.read(length).into(),
            },
        })
    }

    fn r#type(&self) -> ResourceType {
        match self {
            Self::A(_) => ResourceType::A,
            Self::AAAA(_) => ResourceType::AAAA,
            Self::PTR(_) => ResourceType::PTR,
            Self::TXT(_) => ResourceType::TXT,
            Self::SRV { .. } => ResourceType::SRV,
            Self::Unknown { r#type, .. } => *r#type,
        }
    }

    fn write(&self, stream: &mut WriteStream) {
        let mut new_stream = WriteStream::new(64);
        match self {
            Self::A(x) => {
                new_stream.write_u32((*x).into());
            }
            Self::AAAA(x) => {
                new_stream.write_u128((*x).into());
            }
            Self::PTR(x) => x.write(&mut new_stream),
            Self::TXT(x) => {
                for item in x {
                    let bytes = item.as_bytes();

                    new_stream.write_u8(bytes.len() as u8);
                    new_stream.write(bytes);
                }
            }
            Self::SRV {
                priority,
                weight,
                port,
                target,
            } => {
                new_stream.write_u16(*priority);
                new_stream.write_u16(*weight);
                new_stream.write_u16(*port);
                target.write(&mut new_stream);
            }
            Self::Unknown { data, .. } => new_stream.write(data),
        }

        stream.write_u16(new_stream.buffer.len() as u16);
        stream.write(&new_stream.buffer);
    }
}

pub struct ResourceRecord {
    name: Name,
    class: Class,
    ttl: u32,
    data: ResourceRecordData,
}

impl ResourceRecord {
    pub fn new(name: &str, ttl: u32, data: ResourceRecordData) -> Self {
        Self {
            name: Name::new(name),
            class: Class::IN,
            ttl,
            data,
        }
    }

    fn parse(stream: &mut ReadStream) -> Result<Self> {
        let name = Name::parse(stream)?;

        let r#type = ResourceType::parse(stream.read_u16());
        let class = Class::parse(stream.read_u16());
        let ttl = stream.read_u32();

        let data = ResourceRecordData::parse(r#type, stream)?;

        Ok(ResourceRecord { name, class, ttl, data })
    }

    fn write(&self, stream: &mut WriteStream) {
        self.name.write(stream);

        self.data.r#type().write(stream);
        self.class.write(stream);
        stream.write_u32(self.ttl as u32);

        self.data.write(stream);
    }
}

pub struct Packet {
    pub header: Header,
    pub questions: Vec<Question>,
    pub answers: Vec<ResourceRecord>,
    pub nameservers: Vec<ResourceRecord>,
    pub additionals: Vec<ResourceRecord>,
}

impl Packet {
    pub fn new_response(
        id: u16,
        questions: Vec<Question>,
        answers: Vec<ResourceRecord>,
        nameservers: Vec<ResourceRecord>,
        additionals: Vec<ResourceRecord>,
    ) -> Self {
        let header = Header {
            id: U16be::new(id),
            flags: HeaderFlags::RESPONSE,
            qd_count: U16be::new(questions.len() as u16),
            an_count: U16be::new(answers.len() as u16),
            ns_count: U16be::new(nameservers.len() as u16),
            ar_count: U16be::new(additionals.len() as u16),
        };

        Self {
            header,
            questions,
            answers,
            nameservers,
            additionals,
        }
    }

    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < size_of::<Header>() {
            return Err(anyhow!("Buffer too small"));
        }

        let mut stream = ReadStream::new(raw);

        let header = stream.read_as::<Header>().clone();

        let questions = (0..header.qd_count.get()).map(|_| Question::parse(&mut stream)).collect::<Result<_>>()?;
        let answers = (0..header.an_count.get())
            .map(|_| ResourceRecord::parse(&mut stream))
            .collect::<Result<_>>()?;
        let nameservers = (0..header.ns_count.get())
            .map(|_| ResourceRecord::parse(&mut stream))
            .collect::<Result<_>>()?;
        let additionals = (0..header.ar_count.get())
            .map(|_| ResourceRecord::parse(&mut stream))
            .collect::<Result<_>>()?;

        Ok(Self {
            header,
            questions,
            answers,
            nameservers,
            additionals,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut stream = WriteStream::new(2048);

        stream.write_from(&self.header);

        self.questions.iter().for_each(|x| x.write(&mut stream));
        self.answers.iter().for_each(|x| x.write(&mut stream));
        self.nameservers.iter().for_each(|x| x.write(&mut stream));
        self.additionals.iter().for_each(|x| x.write(&mut stream));

        stream.buffer
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // some tests are copied from https://github.com/librespot-org/libmdns/blob/master/src/dns_parser/parser.rs
    #[test]
    fn parse_simple_query() -> Result<()> {
        let query = b"\x06%\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x80\x01";
        let packet = Packet::parse(query)?;

        assert_eq!(packet.header.id.get(), 1573);
        assert!(packet.header.is_query());
        assert_eq!(packet.header.qd_count.get(), 1);
        assert_eq!(packet.header.an_count.get(), 0);
        assert_eq!(packet.header.ns_count.get(), 0);
        assert_eq!(packet.header.ar_count.get(), 0);

        assert_eq!(packet.questions.len(), 1);
        assert_eq!(packet.questions[0].name.labels.len(), 2);
        assert_eq!(packet.questions[0].name.labels[0], "example");
        assert_eq!(packet.questions[0].name.labels[1], "com");
        assert!(packet.questions[0].r#type == ResourceType::A);
        assert!(packet.questions[0].class == Class::IN);

        let new_packet = packet.write();

        assert_eq!(new_packet.len(), query.len());
        assert_eq!(&new_packet, query);

        Ok(())
    }

    #[test]
    fn parse_simple_response() -> Result<()> {
        let response =  b"\x06%\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x80\x01\x07example\x03com\x00\x00\x01\x80\x01\x00\x00\x04\xf8\x00\x04]\xb8\xd8\"";
        let packet = Packet::parse(response)?;

        assert_eq!(packet.header.id.get(), 1573);
        assert!(!packet.header.is_query());
        assert_eq!(packet.header.qd_count.get(), 1);
        assert_eq!(packet.header.an_count.get(), 1);
        assert_eq!(packet.header.ns_count.get(), 0);
        assert_eq!(packet.header.ar_count.get(), 0);

        assert_eq!(packet.questions.len(), 1);
        assert_eq!(packet.questions[0].name.labels.len(), 2);
        assert_eq!(packet.questions[0].name.labels[0], "example");
        assert_eq!(packet.questions[0].name.labels[1], "com");
        assert!(packet.questions[0].r#type == ResourceType::A);
        assert!(packet.questions[0].class == Class::IN);

        assert_eq!(packet.answers.len(), 1);
        assert_eq!(packet.answers[0].name.labels.len(), 2);
        assert_eq!(packet.answers[0].name.labels[0], "example");
        assert_eq!(packet.answers[0].name.labels[1], "com");
        assert!(matches!(packet.answers[0].data, ResourceRecordData::A(_)));
        assert!(packet.answers[0].class == Class::IN);

        let new_packet = packet.write();

        assert_eq!(new_packet.len(), response.len());
        assert_eq!(&new_packet, response);

        Ok(())
    }

    #[test]
    fn parse_simple_response_with_name_pointer() -> Result<()> {
        let response =  b"\x06%\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01\xc0\x0c\x00\x01\x00\x01\x00\x00\x04\xf8\x00\x04]\xb8\xd8\"";
        let packet = Packet::parse(response)?;

        assert_eq!(packet.header.id.get(), 1573);
        assert!(!packet.header.is_query());
        assert_eq!(packet.header.qd_count.get(), 1);
        assert_eq!(packet.header.an_count.get(), 1);
        assert_eq!(packet.header.ns_count.get(), 0);
        assert_eq!(packet.header.ar_count.get(), 0);

        assert_eq!(packet.questions.len(), 1);
        assert_eq!(packet.questions[0].name.labels.len(), 2);
        assert_eq!(packet.questions[0].name.labels[0], "example");
        assert_eq!(packet.questions[0].name.labels[1], "com");
        assert!(packet.questions[0].r#type == ResourceType::A);
        assert!(packet.questions[0].class == Class::IN);

        assert_eq!(packet.answers.len(), 1);
        assert_eq!(packet.answers[0].name.labels.len(), 2);
        assert_eq!(packet.answers[0].name.labels[0], "example");
        assert_eq!(packet.answers[0].name.labels[1], "com");
        assert!(matches!(packet.answers[0].data, ResourceRecordData::A(_)));
        assert!(packet.answers[0].class == Class::IN);

        Ok(())
    }

    #[test]
    fn parse_mdns_query() -> Result<()> {
        let query = b"\x00\x00\x00\x00\x00\x05\x00\x00\x00\x00\x00\x00\x0f_companion-link\x04_tcp\x05local\x00\x00\x0c\x80\x01\x08_homekit\xc0\x1c\x00\x0c\x80\x01\x08_airplay\xc0\x1c\x00\x0c\x80\x01\x05_raop\xc0\x1c\x00\x0c\x80\x01\x0c_sleep-proxy\x04_udp\xc0!\x00\x0c\x80\x01";
        let packet = Packet::parse(query)?;

        assert_eq!(packet.header.id.get(), 0);
        assert!(packet.header.is_query());
        assert_eq!(packet.header.qd_count.get(), 5);
        assert_eq!(packet.header.an_count.get(), 0);
        assert_eq!(packet.header.ns_count.get(), 0);
        assert_eq!(packet.header.ar_count.get(), 0);

        Ok(())
    }

    #[test]
    fn write_and_parse() -> Result<()> {
        let hostname = "hostname.local";
        let ip = Ipv4Addr::new(192, 168, 1, 1);

        // PTR answer
        let answer = ResourceRecord::new("_raop._tcp.local", 3600, ResourceRecordData::PTR(Name::new("test")));

        // SRV record
        let srv = ResourceRecord::new(
            "test",
            3600,
            ResourceRecordData::SRV {
                priority: 0,
                weight: 0,
                port: 1234,
                target: Name::new(hostname),
            },
        );

        // TXT record
        let txt = ResourceRecord::new("test", 3600, ResourceRecordData::TXT(vec!["test".into(), "test1".into()]));

        // A RECORD
        let a = ResourceRecord::new(hostname, 3600, ResourceRecordData::A(ip));

        let packet = Packet::new_response(1234, Vec::new(), vec![answer], Vec::new(), vec![srv, txt, a]);

        let packet2 = Packet::parse(&packet.write())?;

        assert_eq!(packet.header.id.get(), packet2.header.id.get());
        assert_eq!(packet.header.qd_count.get(), packet2.header.qd_count.get());
        assert_eq!(packet.header.an_count.get(), packet2.header.an_count.get());
        assert_eq!(packet.header.ns_count.get(), packet2.header.ns_count.get());
        assert_eq!(packet.header.ar_count.get(), packet2.header.ar_count.get());

        Ok(())
    }
}
