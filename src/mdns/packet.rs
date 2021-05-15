#[derive(Clone, Debug)]
#[repr(C)]
pub struct U16be {
    raw: [u8; 2],
}

impl U16be {
    pub fn get(&self) -> u16 {
        u16::from_be_bytes(self.raw)
    }
}

#[derive(Clone, Debug)]
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
#[derive(Debug)]
pub struct Header {
    id: U16be,
    flags: U16be,
    qd_count: U16be,
    an_count: U16be,
    ns_count: U16be,
    ar_count: U16be,
}
