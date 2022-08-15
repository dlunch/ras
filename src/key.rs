use rsa::{pkcs1::DecodeRsaPrivateKey, RsaPrivateKey};

lazy_static::lazy_static! {
    pub static ref RTSP_KEY: RsaPrivateKey = RsaPrivateKey::from_pkcs1_pem(include_str!("rtsp.key")).unwrap();
}
