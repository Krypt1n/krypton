use ed25519_dalek::{VerifyingKey, SigningKey};
use blake3::hash;
use rand::rngs::OsRng;
use serde::Deserialize;
use std::fmt;
use base64::prelude::*;

#[derive(Clone, Debug, PartialEq, Hash, Eq, Deserialize)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn from_public_key(pk: &VerifyingKey) -> Self {
        let address: [u8; 20] = hash(pk.as_bytes()).as_bytes()[0..20].try_into().unwrap();

        Self(address)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        bytes.extend_from_slice(&self.0);
        bytes
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let address = BASE64_STANDARD_NO_PAD.encode(self.0);
        write!(f, "{}", address)
    }
}

pub fn get_user_keypair() -> (SigningKey, VerifyingKey) {
    let mut rng = OsRng;
    let secret_key = ed25519_dalek::SigningKey::generate(&mut rng);
    let public_key = secret_key.verifying_key();
    (secret_key, public_key)
} 
