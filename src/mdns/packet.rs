use std::{mem::size_of, str};

use bitflags::bitflags;

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

    pub fn get(&self) -> u32 {
        u32::from_be_bytes(self.raw)
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

pub struct Name {
    labels: Vec<String>,
}

impl Name {
    pub fn parse(raw: &[u8], original: &[u8]) -> (usize, Self) {
        let mut cursor = 0;

        let mut labels = Vec::new();
        loop {
            let length = raw[cursor] as usize;
            if length == 0 {
                break;
            }
            if length & 192 == 192 {
                let offset = cast::<U16be>(&raw[cursor..cursor + 2]);
                let offset = (offset.get() & !49152) as usize;
                let result = Name::parse(&original[offset..], original);

                return (cursor + 2, result.1);
            } else {
                let label = &raw[cursor + 1..cursor + 1 + length];
                labels.push(str::from_utf8(label).unwrap().into());
            }

            cursor += length + 1;
        }

        (cursor + 1, Self { labels })
    }

    pub fn write(&self, buf: &mut [u8]) -> usize {
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
    pub fn parse(raw: &U16be) -> Self {
        match raw.get() {
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
    pub fn parse(raw: &U16be) -> Self {
        match raw.get() {
            1 => Self::IN,
            unknown => panic!("Unknown class {}", unknown),
        }
    }
}

pub struct Question {
    name: Name,
    r#type: ResourceType,
    class: Class,
}

impl Question {
    pub fn parse(raw: &[u8], original: &[u8]) -> (usize, Self) {
        let mut cursor = 0;
        let (name_len, name) = Name::parse(raw, original);
        cursor += name_len;

        let r#type = cast::<U16be>(&raw[cursor..cursor + 2]);
        let class = cast::<U16be>(&raw[cursor + 2..cursor + 4]);
        cursor += 4;

        (
            cursor,
            Question {
                name,
                r#type: ResourceType::parse(r#type),
                class: Class::parse(class),
            },
        )
    }

    pub fn write(&self, buf: &mut [u8]) -> usize {
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
    pub fn parse(raw: &[u8], original: &[u8]) -> (usize, Self) {
        let mut cursor = 0;
        let (name_len, name) = Name::parse(raw, original);
        cursor += name_len;

        let r#type = cast::<U16be>(&raw[cursor..cursor + 2]);
        let class = cast::<U16be>(&raw[cursor + 2..cursor + 4]);
        cursor += 4;

        let ttl = cast::<U32be>(&raw[cursor..cursor + 4]);
        cursor += 4;

        let rd_len = cast::<U16be>(&raw[cursor..cursor + 2]);
        cursor += 2;

        let data = Vec::from(&raw[cursor..cursor + rd_len.get() as usize]);
        cursor += rd_len.get() as usize;

        (
            cursor,
            ResourceRecord {
                name,
                r#type: ResourceType::parse(r#type),
                class: Class::parse(class),
                ttl: ttl.get(),
                data,
            },
        )
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
    header: Header,
    questions: Vec<Question>,
    answers: Vec<ResourceRecord>,
    nameservers: Vec<ResourceRecord>,
    additional: Vec<ResourceRecord>,
}

impl Packet {
    pub fn parse(raw: &[u8]) -> Self {
        let header = cast::<Header>(&raw);

        let mut cursor = size_of::<Header>();

        let mut questions = Vec::new();
        for _ in 0..header.qd_count.get() {
            let (len, question) = Question::parse(&raw[cursor..], raw);
            cursor += len;

            questions.push(question);
        }

        let mut answers = Vec::new();
        for _ in 0..header.an_count.get() {
            let (len, answer) = ResourceRecord::parse(&raw[cursor..], raw);
            cursor += len;

            answers.push(answer);
        }

        if cursor != raw.len() {
            panic!("Some bytes left, raw: {:?}", raw)
        }

        Self {
            header: header.clone(),
            questions,
            answers,
            nameservers: Vec::new(),
            additional: Vec::new(),
        }
    }

    pub fn write(&self, mut buf: &mut [u8]) -> usize {
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

pub fn cast<T>(data: &[u8]) -> &T {
    unsafe { &*(data.as_ptr() as *const T) }
}

pub fn cast_bytes<T>(data: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((data as *const T) as *const u8, size_of::<T>()) }
}

#[cfg(test)]
mod test {
    use super::*;

    // tests are copied from https://github.com/librespot-org/libmdns/blob/master/src/dns_parser/parser.rs
    #[test]
    fn parse_simple_query() {
        let query = b"\x06%\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x07example\x03com\x00\x00\x01\x00\x01";
        let packet = Packet::parse(query);

        assert_eq!(packet.header.id.get(), 1573);
        assert!(!packet.header.flags.contains(HeaderFlags::RESPONSE));
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
        let packet = Packet::parse(response);

        assert_eq!(packet.header.id.get(), 1573);
        assert!(packet.header.flags.contains(HeaderFlags::RESPONSE));
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
        let packet = Packet::parse(response);

        assert_eq!(packet.header.id.get(), 1573);
        assert!(packet.header.flags.contains(HeaderFlags::RESPONSE));
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
}
