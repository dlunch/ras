use std::net::IpAddr;

use aes::{
    cipher::{BlockDecryptMut, KeyIvInit},
    Aes128, Block,
};
use anyhow::Result;
use cbc::Decryptor;
use rsa::{pkcs1::DecodeRsaPrivateKey, PaddingScheme, RsaPrivateKey};

lazy_static::lazy_static! {
    pub static ref KEY: RsaPrivateKey = RsaPrivateKey::from_pkcs1_pem(include_str!("rtsp.key")).unwrap();
}

pub struct AppleChallenge {
    ip_mac: Vec<u8>,
}

impl AppleChallenge {
    pub fn new(ip_addr: IpAddr, mac_address: &[u8]) -> Self {
        let mut ip_mac = Vec::with_capacity(14);

        match ip_addr {
            IpAddr::V4(ip) => ip_mac.extend_from_slice(&ip.octets()),
            IpAddr::V6(ip) => ip_mac.extend_from_slice(&ip.octets()),
        }
        ip_mac.extend_from_slice(mac_address);

        Self { ip_mac }
    }

    pub fn response(&self, challenge: &str) -> Result<String> {
        let mut challenge = base64::decode(challenge).unwrap();
        challenge.extend_from_slice(&self.ip_mac);

        let response = KEY.sign(PaddingScheme::new_pkcs1v15_sign_raw(), &challenge)?;

        Ok(base64::encode(response).replace('=', ""))
    }
}

pub struct RsaAesCipher {
    cipher: Decryptor<Aes128>,
}

impl RsaAesCipher {
    pub fn new(rsaaeskey: &[u8], aesiv: &[u8]) -> Result<Self> {
        let aeskey = KEY.decrypt(PaddingScheme::new_oaep::<sha1::Sha1>(), rsaaeskey)?;
        let cipher = Decryptor::<Aes128>::new_from_slices(&aeskey, aesiv).unwrap();

        Ok(Self { cipher })
    }

    pub fn decrypt(&self, raw: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = self.cipher.clone();

        let mut decrypted = raw.to_vec();
        decrypted.chunks_exact_mut(16).for_each(|x| {
            let block = Block::from_mut_slice(x);
            cipher.decrypt_block_mut(block);
        });

        Ok(decrypted)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn apple_challenge_test() -> Result<()> {
        let addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let mac_addr = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];

        let challenge = AppleChallenge::new(addr, &mac_addr);
        let response = challenge.response("test")?;

        assert_eq!(response, "O5TD24VQqAKIdTjPfoZzAJIrJo0Vc3gXzVAy18cWSLGN9ckUjjSWs5YCPkSmN3ExPCq2FTHtCYMW03p27K5zav97hETnJ7yLznE7cVc1RztWk0msX4MmSoN84Ei9hKDAALq/e68d6OWU+0sSX0cYcRLegkNLiCt2fNT9DnLV3PPNfBOh6bZ+PKIlqeTdAdzm73t6Lz5CBNbM7E7M/faE03XJiQHIjRylKoXRDRLwImuz8l8rWxjBjWhmcKoBbjmk1X1ohSeZWkx0ie9ySQJYyTk2PlrPFTTdA2DFrGNEIHvxPbQ94Sr5oF5lUjNaXMj2dLidRu8sQWWrhUqCkGd3JQ");

        Ok(())
    }

    #[tokio::test]
    async fn cipher_test() -> Result<()> {
        let key = vec![
            158, 76, 35, 216, 157, 106, 76, 58, 199, 141, 158, 138, 173, 201, 45, 156, 254, 206, 205, 119, 12, 55, 233, 178, 51, 236, 155, 75, 162,
            27, 120, 221, 71, 24, 230, 21, 162, 21, 2, 212, 94, 244, 12, 136, 89, 230, 99, 140, 66, 9, 217, 120, 191, 121, 122, 26, 95, 20, 221, 222,
            47, 41, 160, 210, 116, 42, 186, 89, 57, 249, 107, 124, 117, 255, 2, 125, 190, 122, 206, 149, 179, 96, 106, 9, 195, 38, 225, 209, 28, 243,
            24, 26, 25, 228, 12, 108, 96, 205, 140, 84, 234, 143, 198, 15, 15, 144, 177, 233, 153, 45, 163, 21, 26, 131, 236, 251, 98, 36, 48, 6,
            234, 7, 194, 181, 197, 13, 114, 74, 42, 111, 223, 60, 47, 87, 224, 27, 212, 95, 215, 122, 222, 90, 140, 82, 156, 212, 29, 81, 5, 253, 77,
            210, 168, 102, 231, 14, 248, 8, 54, 20, 13, 2, 153, 78, 170, 229, 150, 182, 177, 214, 160, 176, 75, 190, 194, 166, 166, 29, 9, 136, 11,
            126, 132, 113, 21, 46, 143, 171, 193, 178, 220, 249, 158, 191, 235, 119, 251, 125, 147, 164, 137, 86, 17, 56, 84, 188, 218, 206, 224,
            205, 76, 94, 81, 125, 179, 197, 90, 125, 169, 109, 174, 123, 198, 183, 151, 189, 20, 89, 98, 132, 83, 72, 47, 175, 190, 29, 62, 252, 127,
            181, 249, 146, 17, 225, 96, 146, 25, 119, 227, 233, 147, 127, 187, 136, 34, 79,
        ];
        let iv = vec![185, 103, 26, 130, 51, 239, 107, 111, 155, 57, 8, 107, 138, 170, 168, 207];

        let cipher = RsaAesCipher::new(&key, &iv)?;

        let raw = vec![
            155, 34, 3, 99, 252, 176, 190, 92, 160, 127, 189, 240, 217, 146, 246, 27, 183, 181, 224, 15, 151, 211, 28, 90, 6, 242, 154, 94, 155, 184,
            129, 146,
        ];
        let decrypted = cipher.decrypt(&raw)?;
        assert_eq!(
            decrypted,
            vec![32, 0, 0, 4, 0, 19, 8, 9, 129, 248, 193, 255, 128, 0, 0, 19, 8, 9, 129, 248, 193, 255, 128, 0, 0, 255, 128, 175, 191, 224, 43, 252]
        );

        let raw = vec![
            155, 34, 3, 99, 252, 176, 190, 92, 160, 127, 189, 240, 217, 146, 246, 27, 210, 70, 124, 62, 143, 209, 113, 154, 188, 101, 182, 68, 46,
            70, 98, 250, 105, 230, 62, 52, 69, 106, 122, 204, 163, 217, 239, 251, 98, 156, 134, 83, 236, 149, 252, 163, 233, 128, 226, 135, 48, 50,
            59, 28, 30, 112, 88, 126, 128, 122, 139, 112, 234, 6, 221, 66, 176, 72, 164, 154, 102, 184, 215, 0, 159, 171, 126, 109, 23, 84, 211, 137,
            200, 231, 153, 43, 197, 52, 11, 72, 47, 158, 119, 83, 223, 154, 101, 16, 159, 63, 245, 0, 200, 195, 247, 160, 116, 234, 31, 152, 10, 230,
            177, 223, 216, 232, 102, 3, 16, 155, 146, 12, 68, 79, 170, 45, 135, 61, 135, 205, 214, 200, 209, 4, 204, 114, 243, 73, 212, 154, 159,
            230, 121, 173, 121, 206, 168, 97, 215, 30, 74, 241, 37, 90, 229, 166, 240, 0, 26, 132, 69, 90, 39, 10, 11, 249, 191, 105, 9, 249, 173,
            96, 234, 48, 87, 76, 231, 205, 54, 0, 129, 246, 235, 228, 58, 163, 235, 171, 116, 248, 77, 172, 94, 121, 135, 53, 107, 164, 88, 164, 210,
            39, 184, 100, 18, 129, 170, 194, 176, 87, 27, 225, 214, 1, 199, 67, 202, 3, 245, 29, 153, 191, 195, 116, 21, 77, 176, 250, 168, 248, 149,
            42, 180, 37, 223, 58, 34, 91, 80, 30, 248,
        ];

        let decrypted = cipher.decrypt(&raw)?;

        assert_eq!(
            decrypted,
            vec![
                32, 0, 0, 4, 0, 19, 8, 9, 129, 248, 193, 255, 128, 0, 0, 19, 8, 9, 129, 248, 193, 255, 128, 0, 0, 237, 140, 35, 186, 107, 207, 248,
                0, 38, 189, 207, 153, 245, 61, 44, 144, 134, 0, 10, 148, 170, 140, 16, 0, 0, 0, 55, 111, 28, 204, 136, 85, 1, 138, 162, 110, 44, 74,
                144, 38, 226, 196, 169, 11, 32, 2, 165, 123, 74, 146, 164, 169, 42, 64, 192, 0, 52, 117, 16, 192, 106, 109, 107, 171, 155, 53, 178,
                246, 70, 196, 14, 60, 209, 73, 12, 85, 1, 170, 82, 170, 55, 10, 87, 85, 10, 87, 77, 197, 145, 214, 158, 71, 90, 120, 144, 192, 0, 28,
                160, 78, 247, 21, 23, 142, 33, 128, 56, 234, 76, 179, 171, 148, 149, 32, 91, 87, 79, 188, 119, 30, 237, 229, 173, 145, 177, 0, 0, 3,
                118, 168, 134, 3, 74, 156, 84, 237, 181, 54, 174, 165, 56, 220, 50, 213, 85, 182, 149, 56, 169, 219, 113, 139, 46, 109, 36, 197, 74,
                60, 136, 96, 178, 230, 210, 64, 44, 185, 179, 91, 53, 209, 167, 87, 149, 105, 226, 186, 168, 101, 167, 145, 2, 165, 41, 170, 81, 228,
                84, 94, 53, 172, 119, 72, 84, 226, 203, 217, 204, 168, 220, 42, 0, 169, 74, 0, 1, 82, 189, 145, 184, 192, 0, 7, 30, 248, 189, 223,
                58, 34, 91, 80, 30, 248
            ]
        );

        Ok(())
    }
}
