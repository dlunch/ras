use std::{
    convert::TryInto,
    fmt,
    mem::size_of,
    net::{Ipv4Addr, Ipv6Addr},
    str,
};

use bitflags::bitflags;

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

    fn is_end(&self) -> bool {
        self.cursor == self.buffer.len()
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

    fn parse(stream: &mut ReadStream) -> Self {
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
                let mut result = Name::parse(&mut new_stream);

                labels.append(&mut result.labels);

                break;
            } else {
                let label = stream.read(length as usize);
                labels.push(str::from_utf8(label).unwrap().into());
            }
        }

        Self { labels }
    }

    fn write(&self, stream: &mut WriteStream) {
        for label in &self.labels {
            stream.write_u8(label.len() as u8);
            stream.write(label.as_bytes());
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
    A = 1,
    PTR = 12,
    TXT = 16,
    AAAA = 28,
    SRV = 33,
    OPT = 41,
}

impl ResourceType {
    fn parse(raw: u16) -> Self {
        match raw {
            1 => Self::A,
            12 => Self::PTR,
            16 => Self::TXT,
            28 => Self::AAAA,
            33 => Self::SRV,
            41 => Self::OPT,
            x => panic!("Unknown resourcetype {}", x),
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
            Self::IN => stream.write_u16(1),
            Self::Unknown(x) => panic!("Cannot write unknown class {}", x),
        }
    }
}

pub struct Question {
    pub name: Name,
    r#type: ResourceType,
    class: Class,
}

impl Question {
    fn parse(mut stream: &mut ReadStream) -> Self {
        let name = Name::parse(&mut stream);

        let r#type = stream.read_u16();
        let class = stream.read_u16();

        // TODO
        let _unicast = class & 0x8000 != 0;

        Question {
            name,
            r#type: ResourceType::parse(r#type),
            class: Class::parse(class),
        }
    }

    fn write(&self, mut stream: &mut WriteStream) {
        self.name.write(&mut stream);

        stream.write_u16(self.r#type as u16);
        self.class.write(&mut stream);
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
    fn parse(r#type: ResourceType, mut stream: &mut ReadStream) -> Self {
        let length = stream.read_u16() as usize;

        match r#type {
            ResourceType::A => Self::A(Ipv4Addr::from(stream.read_u32())),
            ResourceType::AAAA => Self::AAAA(Ipv6Addr::from(stream.read_u128())),
            ResourceType::PTR => Self::PTR(Name::parse(&mut stream)),
            ResourceType::TXT => {
                let mut txt = Vec::new();
                let mut new_stream = ReadStream::new(&stream.buffer[stream.cursor..stream.cursor + length]);
                loop {
                    let length = new_stream.read_u8() as usize;
                    txt.push(str::from_utf8(new_stream.read(length)).unwrap().into());

                    if new_stream.is_end() {
                        break;
                    }
                }

                Self::TXT(txt)
            }
            ResourceType::SRV => Self::SRV {
                priority: stream.read_u16(),
                weight: stream.read_u16(),
                port: stream.read_u16(),
                target: Name::parse(&mut stream),
            },
            x => Self::Unknown {
                r#type: x,
                data: stream.read(length).into(),
            },
        }
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

    fn write(&self, mut stream: &mut WriteStream) {
        let mut new_stream = WriteStream::new(64);
        match self {
            Self::A(x) => {
                new_stream.write_u32((*x).into());
            }
            Self::AAAA(x) => {
                new_stream.write_u128((*x).into());
            }
            Self::PTR(x) => x.write(&mut stream),
            Self::TXT(x) => {
                for item in x {
                    new_stream.write_u16(item.len() as u16);
                    new_stream.write(item.as_bytes());
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
                target.write(&mut stream);
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

    fn parse(mut stream: &mut ReadStream) -> Self {
        let name = Name::parse(&mut stream);

        let r#type = ResourceType::parse(stream.read_u16());
        let class = Class::parse(stream.read_u16());
        let ttl = stream.read_u32();

        let data = ResourceRecordData::parse(r#type, &mut stream);

        ResourceRecord { name, class, ttl, data }
    }

    fn write(&self, mut stream: &mut WriteStream) {
        self.name.write(&mut stream);

        stream.write_u16(self.data.r#type() as u16);
        self.class.write(&mut stream);
        stream.write_u32(self.ttl as u32);

        self.data.write(&mut stream);
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

    pub fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() < size_of::<Header>() {
            return None;
        }

        let mut stream = ReadStream::new(raw);

        let header = stream.read_as::<Header>().clone();

        let questions = (0..header.qd_count.get()).map(|_| Question::parse(&mut stream)).collect::<Vec<_>>();
        let answers = (0..header.an_count.get()).map(|_| ResourceRecord::parse(&mut stream)).collect::<Vec<_>>();
        let nameservers = (0..header.ns_count.get()).map(|_| ResourceRecord::parse(&mut stream)).collect::<Vec<_>>();
        let additionals = (0..header.ar_count.get()).map(|_| ResourceRecord::parse(&mut stream)).collect::<Vec<_>>();

        Some(Self {
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
    fn parse_simple_query() {
        let query = b"\x06%\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01";
        let packet = Packet::parse(query).unwrap();

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
    }

    #[test]
    fn parse_simple_response() {
        let response =  b"\x06%\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01\x07example\x03com\x00\x00\x01\x00\x01\x00\x00\x04\xf8\x00\x04]\xb8\xd8\"";
        let packet = Packet::parse(response).unwrap();

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
    }

    #[test]
    fn parse_simple_response_with_name_pointer() {
        let response =  b"\x06%\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01\xc0\x0c\x00\x01\x00\x01\x00\x00\x04\xf8\x00\x04]\xb8\xd8\"";
        let packet = Packet::parse(response).unwrap();

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
    }

    #[test]
    fn parse_mdns_query() {
        let query = b"\x00\x00\x00\x00\x00\x05\x00\x00\x00\x00\x00\x00\x0f_companion-link\x04_tcp\x05local\x00\x00\x0c\x80\x01\x08_homekit\xc0\x1c\x00\x0c\x80\x01\x08_airplay\xc0\x1c\x00\x0c\x80\x01\x05_raop\xc0\x1c\x00\x0c\x80\x01\x0c_sleep-proxy\x04_udp\xc0!\x00\x0c\x80\x01";
        let packet = Packet::parse(query).unwrap();

        assert_eq!(packet.header.id.get(), 0);
        assert!(packet.header.is_query());
        assert_eq!(packet.header.qd_count.get(), 5);
        assert_eq!(packet.header.an_count.get(), 0);
        assert_eq!(packet.header.ns_count.get(), 0);
        assert_eq!(packet.header.ar_count.get(), 0);
    }
}
