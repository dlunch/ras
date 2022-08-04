use std::net::IpAddr;

use anyhow::Result;
use rsa::PaddingScheme;

use super::key::RAOP_KEY;

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

        let response = RAOP_KEY.sign(PaddingScheme::new_pkcs1v15_sign(None), &challenge)?;

        Ok(base64::encode(response).replace('=', ""))
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
}
