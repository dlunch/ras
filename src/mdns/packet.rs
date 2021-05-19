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

#[repr(C)]
#[derive(Clone)]
pub struct Header {
    id: U16be,
    flags: U16be,
    qd_count: U16be,
    an_count: U16be,
    ns_count: U16be,
    ar_count: U16be,
}

pub struct Name {
    labels: Vec<String>,
}

pub struct Question {
    qname: Name,
    qtype: U16be,
    qclass: U16be,
}

pub struct ResourceRecord {
    rrname: Name,
    rrtype: U16be,
    rrclass: U16be,
    ttl: U32be,
    rdlength: U16be,
    rdata: Vec<u8>,
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
