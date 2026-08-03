//! Determinism harness helpers.

use sha2::{Digest, Sha256};

pub fn endpoint_bits(values: &[f64]) -> String {
    let mut h = Sha256::new();
    for v in values {
        h.update(v.to_bits().to_le_bytes());
    }
    hex::encode(h.finalize())
}

pub struct RepeatSummary {
    pub endpoint_bits: Vec<String>,
    pub accepted_steps: Vec<u32>,
    pub json_digests: Vec<String>,
    pub deterministic: bool,
}

pub fn repeat_in_process<F>(runs: u32, mut f: F) -> RepeatSummary
where
    F: FnMut() -> (Vec<f64>, u32, String),
{
    let mut endpoint_bits = Vec::new();
    let mut accepted_steps = Vec::new();
    let mut json_digests = Vec::new();
    for _ in 0..runs {
        let (endpoint, acc, digest) = f();
        endpoint_bits.push(crate::determinism::endpoint_bits(&endpoint));
        accepted_steps.push(acc);
        json_digests.push(digest);
    }
    let deterministic = endpoint_bits.windows(2).all(|w| w[0] == w[1])
        && accepted_steps.windows(2).all(|w| w[0] == w[1]);
    RepeatSummary {
        endpoint_bits,
        accepted_steps,
        json_digests,
        deterministic,
    }
}
