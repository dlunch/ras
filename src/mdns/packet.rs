use core::{convert::TryInto, fmt, mem::size_of, str};

use bitflags::bitflags;

struct ReadStream<'a> {
    buffer: &'a [u8],
    cursor: usize,
}

impl<'a> ReadStream<'a> {
    fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, cursor: 0 }
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

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct U32be {
    raw: [u8; 4],
}

impl U32be {
    pub fn new(value: u32) -> Self {
        Self { raw: value.to_be_bytes() }
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
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

                let mut new_stream = ReadStream::new(&stream.buffer[offset..]);
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

    fn write(&self, buf: &mut [u8]) -> usize {
        let mut cursor = 0;

        for label in &self.labels {
            buf[cursor] = label.len() as u8;
            cursor += 1;

            buf[cursor..cursor + label.len()].copy_from_slice(label.as_bytes());
            cursor += label.len();
        }

        buf[cursor] = 0;
        cursor += 1;

        cursor
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
    SRV = 33,
}

impl ResourceType {
    fn parse(raw: u16) -> Self {
        match raw {
            1 => Self::A,
            12 => Self::PTR,
            16 => Self::TXT,
            33 => Self::SRV,
            unknown => panic!("Unknown resourcetype {}", unknown),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Class {
    IN = 1,
}

impl Class {
    fn parse(raw: u16) -> Self {
        match raw & 0x7fff {
            1 => Self::IN,
            unknown => panic!("Unknown class {}", unknown),
        }
    }
}

pub struct Question {
    pub name: Name,
    r#type: ResourceType,
    unicast: bool,
    class: Class,
}

impl Question {
    fn parse(mut stream: &mut ReadStream) -> Self {
        let name = Name::parse(&mut stream);

        let r#type = stream.read_u16();
        let class = stream.read_u16();

        let unicast = class & 0x8000 != 0;

        Question {
            name,
            r#type: ResourceType::parse(r#type),
            unicast,
            class: Class::parse(class),
        }
    }

    fn write(&self, buf: &mut [u8]) -> usize {
        let mut cursor = self.name.write(buf);

        buf[cursor..cursor + 2].copy_from_slice(U16be::new(self.r#type as u16).raw());
        cursor += 2;

        buf[cursor..cursor + 2].copy_from_slice(U16be::new(self.class as u16).raw());
        cursor += 2;

        cursor
    }
}

pub struct ResourceRecord {
    name: Name,
    r#type: ResourceType,
    class: Class,
    ttl: u32,
    data: Vec<u8>,
}

impl ResourceRecord {
    fn parse(mut stream: &mut ReadStream) -> Self {
        let name = Name::parse(&mut stream);

        let r#type = stream.read_u16();
        let class = stream.read_u16();
        let ttl = stream.read_u32();
        let rd_len = stream.read_u16();

        let data = stream.read(rd_len as usize);

        ResourceRecord {
            name,
            r#type: ResourceType::parse(r#type),
            class: Class::parse(class),
            ttl,
            data: data.into(),
        }
    }

    pub fn write(&self, buf: &mut [u8]) -> usize {
        let mut cursor = self.name.write(buf);

        buf[cursor..cursor + 2].copy_from_slice(U16be::new(self.r#type as u16).raw());
        cursor += 2;

        buf[cursor..cursor + 2].copy_from_slice(U16be::new(self.class as u16).raw());
        cursor += 2;

        buf[cursor..cursor + 4].copy_from_slice(U32be::new(self.ttl).raw());
        cursor += 4;

        buf[cursor..cursor + 2].copy_from_slice(U16be::new(self.data.len() as u16).raw());
        cursor += 2;

        buf[cursor..cursor + self.data.len()].copy_from_slice(&self.data);
        cursor += self.data.len();

        cursor
    }
}

pub struct Packet {
    pub header: Header,
    pub questions: Vec<Question>,
    pub answers: Vec<ResourceRecord>,
    pub nameservers: Vec<ResourceRecord>,
}

impl Packet {
    pub fn new_response(id: u16) -> Self {
        let header = Header {
            id: U16be::new(id),
            flags: HeaderFlags::RESPONSE,
            qd_count: U16be::new(0),
            an_count: U16be::new(0),
            ns_count: U16be::new(0),
            ar_count: U16be::new(0),
        };

        Self {
            header,
            questions: Vec::new(),
            answers: Vec::new(),
            nameservers: Vec::new(),
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

        Some(Self {
            header,
            questions,
            answers,
            nameservers,
        })
    }

    pub fn write(&self, buf: &mut [u8]) -> usize {
        let mut cursor = 0;

        buf[0..size_of::<Header>()].copy_from_slice(cast_bytes(&self.header));
        cursor += size_of::<Header>();

        for question in &self.questions {
            cursor += question.write(&mut buf[cursor..]);
        }

        for answer in &self.answers {
            cursor += answer.write(&mut buf[cursor..]);
        }

        cursor
    }
}

pub fn cast_bytes<T>(data: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((data as *const T) as *const u8, size_of::<T>()) }
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

        let mut buf = vec![0; 512];
        let len = packet.write(&mut buf);

        assert_eq!(len, query.len());
        assert_eq!(&buf[..len], query);
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
        assert!(packet.answers[0].r#type == ResourceType::A);
        assert!(packet.answers[0].class == Class::IN);

        let mut buf = vec![0; 512];
        let len = packet.write(&mut buf);

        assert_eq!(len, response.len());
        assert_eq!(&buf[..len], response);
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
        assert!(packet.answers[0].r#type == ResourceType::A);
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
