use ed25519_dalek::{VerifyingKey};
use blake3::hash;

#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn from_public_key(pk: &VerifyingKey) -> Self {
        let address: [u8;20] = hash(pk.as_bytes()).as_bytes()[0..20].try_into().unwrap();

        Self(address)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.extend_from_slice(&self.0);
        bytes
    }
}
