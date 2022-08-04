use rsa::{pkcs1::DecodeRsaPrivateKey, RsaPrivateKey};

lazy_static::lazy_static! {
    pub static ref RAOP_KEY: RsaPrivateKey = RsaPrivateKey::from_pkcs1_pem(include_str!("raop.key")).unwrap();
}
