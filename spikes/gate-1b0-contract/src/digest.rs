//! JSON digest helper.

use sha2::{Digest, Sha256};

pub fn json_digest(value: &impl serde::Serialize) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
