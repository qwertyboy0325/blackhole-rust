//! Determinism harness helpers.

use sha2::{Digest, Sha256};

pub fn endpoint_bits(values: &[f64]) -> String {
    let mut h = Sha256::new();
    for v in values {
        h.update(v.to_bits().to_le_bytes());
    }
    hex::encode(h.finalize())
}

pub fn signature_bits(values: &[f64]) -> String {
    endpoint_bits(values)
}

pub fn signature_join(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
    }
    hex::encode(h.finalize())
}

pub struct RepeatSummary {
    pub signatures: Vec<String>,
    pub endpoint_bits: Vec<String>,
    pub accepted_steps: Vec<u32>,
    pub json_digests: Vec<String>,
    pub deterministic: bool,
}

pub fn repeat_in_process<F>(runs: u32, mut f: F) -> RepeatSummary
where
    F: FnMut() -> (Vec<f64>, u32, String),
{
    let mut signatures = Vec::new();
    let mut endpoint_bits = Vec::new();
    let mut accepted_steps = Vec::new();
    let mut json_digests = Vec::new();
    for _ in 0..runs {
        let (endpoint, acc, digest) = f();
        let bits = crate::determinism::endpoint_bits(&endpoint);
        signatures.push(bits.clone());
        endpoint_bits.push(bits);
        accepted_steps.push(acc);
        json_digests.push(digest);
    }
    let deterministic = signatures.windows(2).all(|w| w[0] == w[1])
        && accepted_steps.windows(2).all(|w| w[0] == w[1]);
    RepeatSummary {
        signatures,
        endpoint_bits,
        accepted_steps,
        json_digests,
        deterministic,
    }
}

pub fn repeat_in_process_sig<F>(runs: u32, mut f: F) -> RepeatSummary
where
    F: FnMut() -> (String, u32, String),
{
    let mut signatures = Vec::new();
    let mut accepted_steps = Vec::new();
    let mut json_digests = Vec::new();
    for _ in 0..runs {
        let (sig, acc, digest) = f();
        signatures.push(sig);
        accepted_steps.push(acc);
        json_digests.push(digest);
    }
    let deterministic = signatures.windows(2).all(|w| w[0] == w[1])
        && accepted_steps.windows(2).all(|w| w[0] == w[1]);
    RepeatSummary {
        signatures: signatures.clone(),
        endpoint_bits: signatures,
        accepted_steps,
        json_digests,
        deterministic,
    }
}
