use std::str;

use bitflags::bitflags;

#[derive(Clone)]
#[repr(C)]
pub struct U16be {
    raw: [u8; 2],
}

impl U16be {
    pub fn get(&self) -> u16 {
        u16::from_be_bytes(self.raw)
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct U32be {
    raw: [u8; 4],
}

impl U32be {
    pub fn get(&self) -> u32 {
        u32::from_be_bytes(self.raw)
    }
}

bitflags! {
    struct HeaderFlags: u16 { // in big endian form
        const QUERY = 0b0000_0000_0000_0001;
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
    pub fn parse(data: &[u8]) -> (usize, Self) {
        let mut offset = 0;

        let mut labels = Vec::new();
        loop {
            // TODO pointer
            let length = data[offset] as usize;
            if length == 0 {
                break;
            }

            let label = &data[offset + 1..offset + 1 + length];
            labels.push(str::from_utf8(label).unwrap().into());

            offset += length + 1;
        }

        (offset + 1, Self { labels })
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Eq, PartialEq)]
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

#[derive(Eq, PartialEq)]
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

pub struct ResourceRecord {
    name: Name,
    r#type: ResourceType,
    class: Class,
    ttl: u32,
    data: Vec<u8>,
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

        let mut cursor = 12;

        let mut questions = Vec::new();
        for _ in 0..header.qd_count.get() {
            let (name_len, name) = Name::parse(&raw[cursor..]);
            cursor += name_len;

            let r#type = cast::<U16be>(&raw[cursor..cursor + 2]);
            let class = cast::<U16be>(&raw[cursor + 2..cursor + 4]);
            cursor += 4;

            let question = Question {
                name,
                r#type: ResourceType::parse(r#type),
                class: Class::parse(class),
            };

            questions.push(question);
        }

        Self {
            header: header.clone(),
            questions,
            answers: Vec::new(),
            nameservers: Vec::new(),
            additional: Vec::new(),
        }
    }
}

pub fn cast<T>(data: &[u8]) -> &T {
    unsafe { &*(data.as_ptr() as *const T) }
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
        assert!(packet.header.flags & HeaderFlags::QUERY == HeaderFlags::QUERY);
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
    }
}
