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
    struct HeaderFlags: u16 {
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

pub enum ResourceType {
    A = 1,
    PTR = 12,
    TXT = 16,
    SRV = 33,
}

pub enum Class {
    IN = 1,
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

        Self {
            header: header.clone(),
            questions: Vec::new(),
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
    }
}
